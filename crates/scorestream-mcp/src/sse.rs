//! Legacy MCP HTTP+SSE (2024-11-05), not Streamable HTTP.
//! A session owns exactly one response stream. Tokens never appear in URLs.
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Query, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{
        IntoResponse, Response, Sse,
        sse::{Event, KeepAlive},
    },
    routing::{get, post},
};
use futures_util::stream;
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::HashMap,
    convert::Infallible,
    fmt,
    future::IntoFuture,
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub type Handler = Arc<dyn Fn(Value) -> Option<Value> + Send + Sync>;
const MAX_SESSIONS: usize = 32;
const QUEUE_SIZE: usize = 32;
const MAX_BODY: usize = 1024 * 1024;

#[derive(Clone)]
pub struct SseConfig {
    pub bind: IpAddr,
    pub port: u16,
    /// Optional on loopback; required on other interfaces. Keep out of logs.
    pub token: Option<String>,
}
impl Default for SseConfig {
    fn default() -> Self {
        Self {
            bind: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 3210,
            token: None,
        }
    }
}
impl fmt::Debug for SseConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SseConfig")
            .field("bind", &self.bind)
            .field("port", &self.port)
            .field("token", &self.token.as_ref().map(|_| "[redacted]"))
            .finish()
    }
}
impl SseConfig {
    pub fn validate(&self) -> io::Result<()> {
        if self
            .token
            .as_ref()
            .is_some_and(|s| s.is_empty() || !s.bytes().all(|b| (0x21..=0x7e).contains(&b)))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "The token must contain nonempty visible ASCII without spaces",
            ));
        }
        if !self.bind.is_loopback() && self.token.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "A Bearer token is required for a non-loopback bind address",
            ));
        }
        Ok(())
    }
}

/// Owns the listener and its runtime. Safe to start outside an async runtime.
/// Stopping closes active SSE streams and releases the port before returning.
pub struct SseServer {
    address: SocketAddr,
    shutdown: CancellationToken,
    worker: Option<thread::JoinHandle<io::Result<()>>>,
}
impl SseServer {
    pub fn start(config: SseConfig, handler: Handler) -> io::Result<Self> {
        config.validate()?;
        let listener = std::net::TcpListener::bind(SocketAddr::new(config.bind, config.port))?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?;
        let shutdown = CancellationToken::new();
        let state = Service::new(config, address, handler, shutdown.clone());
        let stop = shutdown.clone();
        let worker = thread::Builder::new()
            .name("scorestream-mcp".into())
            .spawn(move || {
                runtime.block_on(async move {
                    let listener = tokio::net::TcpListener::from_std(listener)?;
                    let server = axum::serve(listener, router(state))
                        .with_graceful_shutdown(stop.clone().cancelled_owned())
                        .into_future();
                    // A stalled HTTP body must not keep the desktop shutdown waiting forever.
                    tokio::pin!(server);
                    tokio::select! {
                        result = &mut server => result,
                        _ = stop.cancelled() => {
                            match tokio::time::timeout(Duration::from_secs(2), &mut server).await {
                                Ok(result) => result,
                                Err(_) => Ok(()), // Drop outstanding connections with the runtime.
                            }
                        }
                    }
                })
            })?;
        Ok(Self {
            address,
            shutdown,
            worker: Some(worker),
        })
    }
    pub fn local_addr(&self) -> SocketAddr {
        self.address
    }
    pub fn is_running(&self) -> bool {
        self.worker.as_ref().is_some_and(|w| !w.is_finished()) && !self.shutdown.is_cancelled()
    }
    pub fn stop(&mut self) -> io::Result<()> {
        self.shutdown.cancel();
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| io::Error::other("MCP service thread failed"))??;
        }
        Ok(())
    }
}
impl Drop for SseServer {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

struct Service {
    config: SseConfig,
    address: SocketAddr,
    handler: Handler,
    sessions: Mutex<HashMap<Uuid, mpsc::Sender<Value>>>,
    shutdown: CancellationToken,
}
impl Service {
    fn new(
        config: SseConfig,
        address: SocketAddr,
        handler: Handler,
        shutdown: CancellationToken,
    ) -> Arc<Self> {
        Arc::new(Self {
            config,
            address,
            handler,
            sessions: Mutex::new(HashMap::new()),
            shutdown,
        })
    }
    fn allowed_authority(&self, value: &str) -> bool {
        // Never trust a request's Host to decide the allowed Origin.
        if value == self.address.to_string() {
            return true;
        }
        self.address.ip().is_loopback() && value == format!("localhost:{}", self.address.port())
    }
    fn authorize(&self, headers: &HeaderMap) -> Result<(), StatusCode> {
        // Reject duplicate Origin/Authorization fields, malformed origins, and null.
        if headers.get_all(header::ORIGIN).iter().count() > 1 {
            return Err(StatusCode::FORBIDDEN);
        }
        if let Some(origin) = headers.get(header::ORIGIN) {
            let valid = origin
                .to_str()
                .ok()
                .and_then(|s| s.strip_prefix("http://"))
                .is_some_and(|authority| self.allowed_authority(authority));
            if !valid {
                return Err(StatusCode::FORBIDDEN);
            }
        }
        if let Some(token) = &self.config.token {
            let value = headers
                .get(header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "));
            if headers.get_all(header::AUTHORIZATION).iter().count() != 1
                || !value.is_some_and(|value| constant_time_eq(value.as_bytes(), token.as_bytes()))
            {
                return Err(StatusCode::UNAUTHORIZED);
            }
        }
        Ok(())
    }
}
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b)
        .fold(0u8, |different, (a, b)| different | (a ^ b))
        == 0
}
fn router(state: Arc<Service>) -> Router {
    Router::new()
        .route("/sse", get(connect))
        .route("/messages", post(message))
        .layer(DefaultBodyLimit::max(MAX_BODY))
        .layer(middleware::from_fn_with_state(state.clone(), check_access))
        .with_state(state)
}
async fn check_access(
    State(state): State<Arc<Service>>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    if let Err(status) = state.authorize(request.headers()) {
        let mut response = status.into_response();
        if status == StatusCode::UNAUTHORIZED {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, "Bearer".parse().unwrap());
        }
        return response;
    }
    // Requests from non-browser MCP clients may omit Origin, but not Host.
    // For wildcard listeners allow literal interface IPs, never arbitrary DNS names.
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok());
    let valid_host = host.is_some_and(|host| {
        state.allowed_authority(host)
            || (state.address.ip().is_unspecified()
                && host
                    .parse::<SocketAddr>()
                    .is_ok_and(|addr| addr.port() == state.address.port()))
    });
    if !valid_host {
        return StatusCode::FORBIDDEN.into_response();
    }
    if state.shutdown.is_cancelled() {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    match tokio::time::timeout(Duration::from_secs(10), next.run(request)).await {
        Ok(response) => response,
        Err(_) => StatusCode::REQUEST_TIMEOUT.into_response(),
    }
}
struct SessionGuard {
    id: Uuid,
    service: Arc<Service>,
}
impl Drop for SessionGuard {
    fn drop(&mut self) {
        if let Ok(mut sessions) = self.service.sessions.lock() {
            sessions.remove(&self.id);
        }
    }
}
async fn connect(State(state): State<Arc<Service>>, headers: HeaderMap) -> Response {
    if !headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|s| {
            s.split(',')
                .any(|v| v.trim().split(';').next() == Some("text/event-stream"))
        })
    {
        return StatusCode::NOT_ACCEPTABLE.into_response();
    }
    let (tx, rx) = mpsc::channel(QUEUE_SIZE);
    let id = Uuid::new_v4();
    {
        let Ok(mut sessions) = state.sessions.lock() else {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        };
        if sessions.len() >= MAX_SESSIONS {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
        sessions.insert(id, tx);
    }
    let guard = SessionGuard {
        id,
        service: state.clone(),
    };
    let endpoint = format!("/messages?session_id={id}");
    let events = stream::unfold(
        (Some(endpoint), rx, guard),
        |(endpoint, mut rx, guard)| async move {
            if let Some(endpoint) = endpoint {
                return Some((
                    Ok::<_, Infallible>(Event::default().event("endpoint").data(endpoint)),
                    (None, rx, guard),
                ));
            }
            tokio::select! {
                _ = guard.service.shutdown.cancelled() => None,
                value = rx.recv() => value.map(|value| (
                    Ok(Event::default().event("message").data(value.to_string())), (None, rx, guard)
                )),
            }
        },
    );
    let mut response = Sse::new(events)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keepalive"),
        )
        .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert("x-accel-buffering", "no".parse().unwrap());
    response
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionQuery {
    session_id: Uuid,
}
async fn message(
    State(state): State<Arc<Service>>,
    Query(query): Query<SessionQuery>,
    Json(request): Json<Value>,
) -> StatusCode {
    let tx = match state.sessions.lock() {
        Ok(sessions) => match sessions.get(&query.session_id) {
            Some(tx) => tx.clone(),
            None => return StatusCode::NOT_FOUND,
        },
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
    };
    // Reserve capacity before executing the handler. Never process a request and
    // then discard its response because another request filled the queue.
    let permit = match tx.try_reserve() {
        Ok(permit) => permit,
        Err(mpsc::error::TrySendError::Full(_)) => return StatusCode::TOO_MANY_REQUESTS,
        Err(mpsc::error::TrySendError::Closed(_)) => return StatusCode::GONE,
    };
    if let Some(response) = (state.handler)(request) {
        permit.send(response);
    }
    StatusCode::ACCEPTED
}

#[cfg(test)]
mod tests;
