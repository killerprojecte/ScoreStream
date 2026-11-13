use super::*;
use axum::{
    body::Body,
    http::{Method, Request},
};
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

fn service(token: Option<&str>) -> Arc<Service> {
    Service::new(
        SseConfig {
            token: token.map(str::to_owned),
            ..Default::default()
        },
        "127.0.0.1:3210".parse().unwrap(),
        Arc::new(crate::handle),
        CancellationToken::new(),
    )
}
fn request(method: Method, uri: &str, body: impl Into<Body>) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "127.0.0.1:3210")
        .header(header::ACCEPT, "text/event-stream")
        .header(header::CONTENT_TYPE, "application/json")
        .body(body.into())
        .unwrap()
}
async fn event(body: &mut Body) -> (String, String) {
    let frame = tokio::time::timeout(Duration::from_secs(1), body.frame())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .into_data()
        .unwrap();
    let text = String::from_utf8(frame.to_vec()).unwrap();
    let field = |name: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(name))
            .unwrap()
            .trim()
            .to_owned()
    };
    (field("event:"), field("data:"))
}
async fn session(app: &Router) -> (Body, String) {
    let response = app
        .clone()
        .oneshot(request(Method::GET, "/sse", Body::empty()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/event-stream"
    );
    let mut body = response.into_body();
    let (kind, endpoint) = event(&mut body).await;
    assert_eq!(kind, "endpoint");
    assert!(endpoint.starts_with("/messages?session_id="));
    (body, endpoint)
}
async fn send(app: &Router, endpoint: &str, value: &Value) -> StatusCode {
    app.clone()
        .oneshot(request(Method::POST, endpoint, value.to_string()))
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn legacy_sse_matches_stdio_and_isolates_sessions() {
    let state = service(None);
    let app = router(state.clone());
    let (mut a, endpoint_a) = session(&app).await;
    let (mut b, endpoint_b) = session(&app).await;
    assert_ne!(endpoint_a, endpoint_b);
    for (stream, endpoint, id) in [(&mut a, &endpoint_a, 1), (&mut b, &endpoint_b, 2)] {
        for request in [
            json!({"jsonrpc":"2.0","id":id,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":id,"method":"tools/list"}),
            json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"score_get_format","arguments":{}}}),
            json!({"jsonrpc":"2.0","id":id,"method":"resources/read","params":{"uri":"scorestream://spec/1.1/grammar"}}),
        ] {
            assert_eq!(send(&app, endpoint, &request).await, StatusCode::ACCEPTED);
            if let Some(expected) = crate::handle(request) {
                let (kind, data) = event(stream).await;
                assert_eq!(kind, "message");
                assert_eq!(serde_json::from_str::<Value>(&data).unwrap(), expected);
            }
        }
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(20), a.frame())
            .await
            .is_err()
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(20), b.frame())
            .await
            .is_err()
    );
    drop(a);
    assert_eq!(state.sessions.lock().unwrap().len(), 1);
    assert_eq!(
        send(
            &app,
            &endpoint_a,
            &json!({"jsonrpc":"2.0","id":4,"method":"ping"})
        )
        .await,
        StatusCode::NOT_FOUND
    );
    state.shutdown.cancel();
    assert!(b.frame().await.is_none());
    assert!(state.sessions.lock().unwrap().is_empty());
}
#[tokio::test]
async fn authentication_and_origin_apply_to_both_routes() {
    let state = service(Some("test-secret"));
    let app = router(state);
    for (method, path) in [
        (Method::GET, "/sse"),
        (
            Method::POST,
            "/messages?session_id=00000000-0000-4000-8000-000000000000",
        ),
    ] {
        for (origin, token, expected) in [
            (None, None, StatusCode::UNAUTHORIZED),
            (None, Some("Bearer wrong"), StatusCode::UNAUTHORIZED),
            (
                Some("http://evil.example:3210"),
                Some("Bearer test-secret"),
                StatusCode::FORBIDDEN,
            ),
            (
                Some("null"),
                Some("Bearer test-secret"),
                StatusCode::FORBIDDEN,
            ),
            (
                Some("http://127.0.0.1:3211"),
                Some("Bearer test-secret"),
                StatusCode::FORBIDDEN,
            ),
            (
                Some("http://localhost:3210/evil"),
                Some("Bearer test-secret"),
                StatusCode::FORBIDDEN,
            ),
        ] {
            let mut req = request(method.clone(), path, "{}");
            if let Some(origin) = origin {
                req.headers_mut()
                    .insert(header::ORIGIN, origin.parse().unwrap());
            }
            if let Some(token) = token {
                req.headers_mut()
                    .insert(header::AUTHORIZATION, token.parse().unwrap());
            }
            assert_eq!(app.clone().oneshot(req).await.unwrap().status(), expected);
        }
        for origin in [
            None,
            Some("http://localhost:3210"),
            Some("http://127.0.0.1:3210"),
        ] {
            let mut req = request(method.clone(), path, "{}");
            req.headers_mut()
                .insert(header::AUTHORIZATION, "Bearer test-secret".parse().unwrap());
            if let Some(origin) = origin {
                req.headers_mut()
                    .insert(header::ORIGIN, origin.parse().unwrap());
            }
            let response = app.clone().oneshot(req).await.unwrap();
            assert_eq!(
                response.status(),
                if method == Method::GET {
                    StatusCode::OK
                } else {
                    StatusCode::NOT_FOUND
                }
            );
        }
    }
    let mut request = request(Method::GET, "/sse", Body::empty());
    request
        .headers_mut()
        .insert(header::AUTHORIZATION, "Bearer test-secret".parse().unwrap());
    request
        .headers_mut()
        .insert(header::HOST, "evil.example:3210".parse().unwrap());
    assert_eq!(
        app.oneshot(request).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn busy_workspace_returns_immediately_and_stream_can_stop() {
    let workspace = Arc::new(Mutex::new(scorestream_app::Workspace::demo().unwrap()));
    let state = Service::new(
        SseConfig::default(),
        "127.0.0.1:3210".parse().unwrap(),
        crate::workspace_handler(workspace.clone()),
        CancellationToken::new(),
    );
    let app = router(state.clone());
    let (mut body, endpoint) = session(&app).await;
    // Simulate the desktop holding its authority while parsing an editor change.
    // Lock in another thread so the regression also works on multithread runtimes.
    let (locked_tx, locked_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let busy = std::thread::spawn(move || {
        let _guard = workspace.lock().unwrap();
        locked_tx.send(()).unwrap();
        let _ = release_rx.recv_timeout(Duration::from_secs(3));
    });
    locked_rx.recv().unwrap();
    let request = json!({"jsonrpc":"2.0","id":1,"method":"resources/read","params":{"uri":"scorestream://workspace/current"}});
    assert_eq!(send(&app, &endpoint, &request).await, StatusCode::ACCEPTED);
    assert_eq!(
        serde_json::from_str::<Value>(&event(&mut body).await.1).unwrap()["error"]["code"],
        -32001
    );
    state.shutdown.cancel();
    assert!(
        tokio::time::timeout(Duration::from_millis(100), body.frame())
            .await
            .unwrap()
            .is_none()
    );
    release_tx.send(()).unwrap();
    busy.join().unwrap();
}
#[tokio::test]
async fn malformed_unknown_bounded_and_disconnected_requests() {
    let state = service(None);
    let app = router(state.clone());
    let (mut body, endpoint) = session(&app).await;
    for (uri, data, status) in [
        (endpoint.as_str(), "{", StatusCode::BAD_REQUEST),
        ("/messages", "{}", StatusCode::BAD_REQUEST),
        ("/messages?session_id=bad", "{}", StatusCode::BAD_REQUEST),
        (
            "/messages?session_id=00000000-0000-4000-8000-000000000000",
            "{}",
            StatusCode::NOT_FOUND,
        ),
    ] {
        assert_eq!(
            app.clone()
                .oneshot(request(Method::POST, uri, data.to_string()))
                .await
                .unwrap()
                .status(),
            status
        );
    }
    assert_eq!(
        app.clone()
            .oneshot(request(Method::POST, &endpoint, "x".repeat(MAX_BODY + 1)))
            .await
            .unwrap()
            .status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    let ping = json!({"jsonrpc":"2.0","id":1,"method":"ping"});
    for _ in 0..QUEUE_SIZE {
        assert_eq!(send(&app, &endpoint, &ping).await, StatusCode::ACCEPTED);
    }
    assert_eq!(
        send(&app, &endpoint, &ping).await,
        StatusCode::TOO_MANY_REQUESTS
    );
    event(&mut body).await;
    assert_eq!(send(&app, &endpoint, &ping).await, StatusCode::ACCEPTED);
    drop(body);
    assert!(state.sessions.lock().unwrap().is_empty());
    let mut bodies = Vec::new();
    for _ in 0..MAX_SESSIONS {
        bodies.push(session(&app).await.0);
    }
    assert_eq!(
        app.clone()
            .oneshot(request(Method::GET, "/sse", Body::empty()))
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    drop(bodies);
    assert!(state.sessions.lock().unwrap().is_empty());
    assert_eq!(
        app.oneshot(request(Method::POST, "/sse", "{}"))
            .await
            .unwrap()
            .status(),
        StatusCode::METHOD_NOT_ALLOWED
    );
}
#[test]
fn configuration_rejects_unsafe_binds_and_redacts_secrets() {
    assert!(SseConfig::default().validate().is_ok());
    let mut config = SseConfig {
        bind: "0.0.0.0".parse().unwrap(),
        ..Default::default()
    };
    assert!(config.validate().is_err());
    config.token = Some("test-secret".into());
    assert!(config.validate().is_ok());
    assert!(!format!("{config:?}").contains("test-secret"));
    for token in ["", "a b", "a\nb", "é"] {
        config.token = Some(token.into());
        assert!(config.validate().is_err());
    }
    config.bind = "::1".parse().unwrap();
    config.token = None;
    assert!(config.validate().is_ok());
}
