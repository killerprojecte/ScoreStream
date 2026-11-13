use scorestream_mcp::sse::{SseConfig, SseServer};
use std::{
    io::{self, BufRead, Write},
    sync::Arc,
};

const HELP: &str = "ScoreStream MCP\n\nDefault: newline-delimited JSON-RPC over stdio.\n\n  scorestream-mcp [--transport stdio|sse] [--bind IP] [--port PORT]\n                  [--token-env ENV_NAME]\n\nSSE defaults: 127.0.0.1:3210; connect to http://127.0.0.1:3210/sse.\n--token-env reads a Bearer token from the named environment variable.\nA token is required for non-loopback binds. There is no --token argument\nso a secret cannot appear in process arguments. HTTP is not encrypted;\nuse loopback or a trusted TLS tunnel. This is legacy HTTP+SSE, not /mcp.\n";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(config) = parse_args(std::env::args().skip(1))? else {
        return Ok(());
    };
    match config {
        Transport::Stdio => stdio()?,
        Transport::Sse(config) => {
            let mut server = SseServer::start(config, Arc::new(scorestream_mcp::handle))?;
            eprintln!(
                "ScoreStream MCP SSE listening at http://{}/sse",
                server.local_addr()
            );
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(tokio::signal::ctrl_c())?;
            server.stop()?;
        }
    }
    Ok(())
}
enum Transport {
    Stdio,
    Sse(SseConfig),
}
fn parse_args(args: impl Iterator<Item = String>) -> Result<Option<Transport>, String> {
    let mut args = args;
    let mut config = SseConfig::default();
    let mut transport = "stdio".to_string();
    let mut http_options = false;
    let mut token_env = None;
    while let Some(arg) = args.next() {
        if arg == "--help" || arg == "-h" {
            print!("{HELP}");
            return Ok(None);
        }
        let value = args
            .next()
            .ok_or("Every option requires a value; use --help")?;
        match arg.as_str() {
            "--transport" => transport = value,
            "--bind" => {
                config.bind = value.parse().map_err(|_| "--bind requires an IP address")?;
                http_options = true;
            }
            "--port" => {
                config.port = value
                    .parse()
                    .map_err(|_| "--port requires an integer from 0 to 65535")?;
                http_options = true;
            }
            "--token-env" => {
                token_env = Some(value);
                http_options = true;
            }
            _ => return Err("Unknown option; use --help".into()),
        }
    }
    match transport.as_str() {
        "stdio" if !http_options => Ok(Some(Transport::Stdio)),
        "stdio" => Err("HTTP options require --transport sse".into()),
        "sse" => {
            if let Some(name) = token_env {
                config.token = Some(std::env::var(name).map_err(
                    |_| "The token environment variable is missing or not valid Unicode",
                )?);
            }
            config.validate().map_err(|e| e.to_string())?;
            Ok(Some(Transport::Sse(config)))
        }
        _ => Err("--transport must be stdio or sse".into()),
    }
}
fn stdio() -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str(&line) {
            Ok(v) => scorestream_mcp::handle(v),
            Err(_) => Some(
                serde_json::json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}}),
            ),
        };
        if let Some(response) = response {
            serde_json::to_writer(&mut stdout, &response)?;
            writeln!(stdout)?;
            stdout.flush()?;
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn args(args: &[&str]) -> Result<Option<Transport>, String> {
        parse_args(args.iter().map(|s| (*s).into()))
    }
    #[test]
    fn cli_defaults_and_validation() {
        assert!(matches!(args(&[]).unwrap(), Some(Transport::Stdio)));
        assert!(matches!(
            args(&["--transport", "sse"]).unwrap(),
            Some(Transport::Sse(_))
        ));
        assert!(args(&["--bind", "127.0.0.1"]).is_err());
        assert!(args(&["--transport", "sse", "--bind", "0.0.0.0"]).is_err());
        assert!(args(&["--transport", "sse", "--port", "65536"]).is_err());
        assert!(args(&["--transport", "sse", "--bind", "localhost"]).is_err());
        assert!(
            args(&[
                "--transport",
                "sse",
                "--token-env",
                "SCORESTREAM_TEST_MISSING_TOKEN"
            ])
            .is_err()
        );
        assert!(
            args(&["--token", "never-print-this"])
                .err()
                .unwrap()
                .find("never-print-this")
                .is_none()
        );
    }
}
