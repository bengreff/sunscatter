//! A minimal MCP server for the game (D044, D069): the Streamable HTTP
//! transport with plain JSON responses (no server-sent events), JSON-RPC 2.0,
//! the `initialize`, `ping`, `tools/list` and `tools/call` methods.
//!
//! The server runs on its own thread and knows nothing about the game: the
//! game registers tools (name, description, JSON schema) at start and
//! answers each call through [`Server::poll`], once per frame, on its own
//! thread. A call waits for the game's answer up to [`CALL_TIMEOUT`].
//!
//! Security: it binds to 127.0.0.1 only, rejects requests whose `Origin`
//! is not local (DNS rebinding), and requires `Authorization: Bearer <token>`
//! when a token is set.

mod protocol;

pub use protocol::{handle, Tool, ToolResult, PROTOCOL_VERSIONS};

use serde_json::Value;
use std::io;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

/// How long a tool call waits for the game (it answers once per frame).
pub const CALL_TIMEOUT: Duration = Duration::from_secs(10);

/// A tool call waiting for the game's answer.
pub struct Call {
    pub tool: String,
    pub arguments: Value,
    reply: Sender<ToolResult>,
}

impl Call {
    pub fn reply(self, result: ToolResult) {
        // The client may have given up (timeout); nothing to do then.
        let _ = self.reply.send(result);
    }
}

/// A running server.
pub struct Server {
    calls: Receiver<Call>,
    port: u16,
    http: std::sync::Arc<tiny_http::Server>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    /// Starts serving `tools` on 127.0.0.1:`port` (0: any free port).
    pub fn start(port: u16, token: Option<String>, tools: Vec<Tool>) -> io::Result<Server> {
        let http = tiny_http::Server::http(("127.0.0.1", port)).map_err(io::Error::other)?;
        let port = http.server_addr().to_ip().map_or(port, |a| a.port());
        let http = std::sync::Arc::new(http);
        let (tx, calls) = mpsc::channel::<Call>();
        let server = http.clone();
        let thread = std::thread::Builder::new().name("mcp".into()).spawn(move || {
            for request in server.incoming_requests() {
                serve(request, token.as_deref(), &tools, &tx);
            }
        })?;
        Ok(Server { calls, port, http, thread: Some(thread) })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// The calls waiting for an answer (never blocks).
    pub fn poll(&self) -> Vec<Call> {
        self.calls.try_iter().collect()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.http.unblock();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Whether an `Origin` header (if any) is a local page or tool.
pub fn origin_allowed(origin: Option<&str>) -> bool {
    let Some(o) = origin else { return true };
    let host = o.split("://").nth(1).unwrap_or(o);
    let host = host.split(['/', ':']).next().unwrap_or("");
    matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}

fn header<'a>(request: &'a tiny_http::Request, name: &str) -> Option<&'a str> {
    request.headers().iter().find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name)).map(|h| h.value.as_str())
}

fn respond(request: tiny_http::Request, status: u16, body: Option<String>) {
    let mut response = tiny_http::Response::from_string(body.unwrap_or_default()).with_status_code(status);
    if let Ok(h) = tiny_http::Header::from_bytes("Content-Type", "application/json") {
        response.add_header(h);
    }
    let _ = request.respond(response);
}

fn serve(mut request: tiny_http::Request, token: Option<&str>, tools: &[Tool], calls: &Sender<Call>) {
    if !origin_allowed(header(&request, "Origin")) {
        return respond(request, 403, None);
    }
    if let Some(t) = token {
        if header(&request, "Authorization") != Some(&format!("Bearer {t}")) {
            return respond(request, 401, None);
        }
    }
    if request.url().split('?').next() != Some("/mcp") {
        return respond(request, 404, None);
    }
    match request.method() {
        tiny_http::Method::Post => {}
        // No server-initiated stream; sessions are not tracked.
        tiny_http::Method::Delete => return respond(request, 200, None),
        _ => return respond(request, 405, None),
    }
    let mut body = String::new();
    if request.as_reader().read_to_string(&mut body).is_err() {
        return respond(request, 400, None);
    }
    let call = |tool: &str, arguments: Value| {
        let (reply, answer) = mpsc::channel();
        let sent = calls.send(Call { tool: tool.to_string(), arguments, reply });
        if sent.is_err() {
            return ToolResult::Err("the game is not running".into());
        }
        answer.recv_timeout(CALL_TIMEOUT).unwrap_or_else(|_| ToolResult::Err("the game did not answer in time".into()))
    };
    let response = match serde_json::from_str::<Value>(&body) {
        Err(e) => Some(protocol::error(Value::Null, protocol::PARSE_ERROR, &e.to_string())),
        Ok(Value::Array(batch)) => {
            let out: Vec<Value> = batch.into_iter().filter_map(|m| handle(m, tools, call)).collect();
            (!out.is_empty()).then_some(Value::Array(out))
        }
        Ok(message) => handle(message, tools, call),
    };
    match response {
        Some(v) => respond(request, 200, Some(v.to_string())),
        // Only notifications or responses: accepted.
        None => respond(request, 202, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::{Read, Write};

    fn post(port: u16, body: &str, auth: Option<&str>) -> (u16, String) {
        let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        let auth = auth.map_or(String::new(), |t| format!("Authorization: Bearer {t}\r\n"));
        write!(
            s,
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\n\
             Accept: application/json, text/event-stream\r\n{auth}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        let status = out.split(' ').nth(1).unwrap().parse().unwrap();
        let body = out.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
        (status, body)
    }

    #[test]
    fn local_origins_only() {
        assert!(origin_allowed(None));
        assert!(origin_allowed(Some("http://localhost:3000")));
        assert!(origin_allowed(Some("http://127.0.0.1")));
        assert!(!origin_allowed(Some("https://evil.example")));
        assert!(!origin_allowed(Some("http://localhost.evil.example")));
    }

    #[test]
    fn a_tool_call_round_trips_through_the_game_loop() {
        let tool = Tool {
            name: "echo".into(),
            description: "Echoes its arguments".into(),
            input_schema: json!({"type": "object"}),
        };
        let server = Server::start(0, Some("secret".into()), vec![tool]).unwrap();
        let port = server.port();
        // The "game": answers calls as they come.
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let game = std::thread::spawn(move || {
            while stop_rx.try_recv().is_err() {
                for c in server.poll() {
                    let args = c.arguments.clone();
                    c.reply(ToolResult::Ok(args));
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        });
        let (status, _) = post(port, r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#, None);
        assert_eq!(status, 401, "the token is required");
        let (status, body) = post(port, r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#, Some("secret"));
        assert_eq!(status, 200);
        assert!(body.contains("\"echo\""), "{body}");
        let call = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"echo","arguments":{"x":42}}}"#;
        let (status, body) = post(port, call, Some("secret"));
        assert_eq!(status, 200);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["result"]["structuredContent"]["x"], 42, "{body}");
        let (status, _) = post(port, r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#, Some("secret"));
        assert_eq!(status, 202);
        stop_tx.send(()).unwrap();
        game.join().unwrap();
    }
}
