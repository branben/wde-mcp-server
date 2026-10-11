//! HTTP integration test: spawn the real server binary and drive it over TCP.
//!
//! Unlike `agent_drive.rs` (which calls `WdeServer` methods directly), this test
//! exercises the full stack: binary startup, TCP bind, axum HTTP transport,
//! JSON-RPC serialization, and the server handler — the same path an MCP
//! client (or the Inspector) takes.

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct ServerHandle {
    child: Child,
    port: u16,
    session_id: std::sync::Mutex<Option<String>>,
}

impl ServerHandle {
    /// Spawn `wde-mcp-http` on a free port and wait until it accepts connections.
    fn start() -> Self {
        let port = free_port();
        let exe = format!("{}/target/debug/wde-mcp-http", env!("CARGO_MANIFEST_DIR"));
        let child = Command::new(&exe)
            .arg("./skills")
            .env("WDE_HTTP_PORT", port.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("failed to spawn {exe}: {e}"));

        let handle = ServerHandle {
            child,
            port,
            session_id: std::sync::Mutex::new(None),
        };
        handle.wait_until_ready();
        handle
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}/mcp", self.port)
    }

    fn wait_until_ready(&self) {
        let client = reqwest::blocking::Client::new();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if Instant::now() > deadline {
                panic!(
                    "server did not become ready within 10s on port {}",
                    self.port
                );
            }
            // Any HTTP response proves the server is listening and the transport
            // layer is working. GET /mcp without proper Accept returns 406.
            if client.get(self.url()).send().is_ok() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Send a JSON-RPC request and return the response body as a string.
    /// Carries the session ID from the initialize response if we have one.
    fn call(&self, body: &str) -> String {
        let client = reqwest::blocking::Client::new();
        let mut req = client
            .post(self.url())
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .body(body.to_string());
        if let Some(sid) = self.session_id.lock().unwrap().as_ref() {
            req = req.header("mcp-session-id", sid);
        }
        req.send()
            .unwrap_or_else(|e| panic!("HTTP request failed: {e}"))
            .text()
            .unwrap_or_else(|e| panic!("failed to read response body: {e}"))
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn free_port() -> u16 {
    let s = std::net::TcpListener::bind("127.0.0.1:0").expect("bind free port");
    let port = s.local_addr().unwrap().port();
    drop(s);
    port
}

fn parse_json(text: &str) -> serde_json::Value {
    // The server may return SSE-wrapped or plain JSON. Try plain first.
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
        return v;
    }
    // SSE format: lines like "data: {...}"
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("data: ")
            && let Ok(v) = serde_json::from_str::<serde_json::Value>(rest)
        {
            return v;
        }
    }
    panic!("could not parse response as JSON or SSE: {text}");
}

fn initialize(server: &ServerHandle) {
    // Use reqwest directly to capture headers (session ID) from the response.
    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(server.url())
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .body(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2026-07-28","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#)
        .send()
        .unwrap_or_else(|e| panic!("initialize request failed: {e}"));

    // Capture the session ID from the response header.
    if let Some(sid) = resp.headers().get("mcp-session-id") {
        *server.session_id.lock().unwrap() = Some(sid.to_str().unwrap().to_string());
    }

    let text = resp
        .text()
        .unwrap_or_else(|e| panic!("failed to read body: {e}"));
    let v = parse_json(&text);
    assert_eq!(v["jsonrpc"], "2.0", "jsonrpc field");
    assert_eq!(v["id"], 1, "id echoes");
    assert!(
        !v["result"]["capabilities"]["extensions"]["io.modelcontextprotocol/skills"].is_null(),
        "skills extension must be advertised in capabilities.extensions (Option D); got: {v}"
    );
}

fn notify_initialized(server: &ServerHandle) {
    let resp = server.call(r#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#);
    // The server returns 202 Accepted with no JSON-RPC response body for
    // notifications. Any non-JSON-RPC body is fine.
    assert!(
        !resp.contains("\"jsonrpc\""),
        "initialized notification should get no JSON-RPC response; got: {resp}"
    );
}

#[test]
fn full_agent_loop_over_http() {
    let server = ServerHandle::start();
    initialize(&server);
    notify_initialized(&server);

    // 1. skills/list → discover the skill
    let resp = server.call(r#"{"jsonrpc":"2.0","id":2,"method":"skills/list","params":{}}"#);
    let v = parse_json(&resp);
    let skills = v["result"]["skills"].as_array().expect("skills array");
    assert_eq!(skills.len(), 1, "exactly one skill");
    let skill_uri = skills[0]["uri"].as_str().expect("skill uri");
    assert_eq!(skill_uri, "skill://worst-day-ever/SKILL.md");

    // 2. resources/directory/read → list the skill's resources
    let resp = server.call(r#"{"jsonrpc":"2.0","id":3,"method":"resources/directory/read","params":{"uri":"skill://worst-day-ever"}}"#);
    let v = parse_json(&resp);
    let resources = v["result"]["resources"]
        .as_array()
        .expect("resources array");
    assert!(
        resources.len() >= 10,
        "directory should list all skill files; got {}",
        resources.len()
    );

    // 3. resources/read → read the skill body
    let resp = server.call(r#"{"jsonrpc":"2.0","id":4,"method":"resources/read","params":{"uri":"skill://worst-day-ever/SKILL.md"}}"#);
    let v = parse_json(&resp);
    let text = v["result"]["contents"][0]["text"]
        .as_str()
        .expect("text content");
    assert!(
        text.contains("# Worst-Day-Ever"),
        "skill body must be returned over HTTP; got: {:.80}",
        text
    );

    // 4. tools/call → the assessment tool
    let resp = server.call(r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"worst_day_ever_assess","arguments":{"project_path":"/Users/brandonbennett/dev/wde-mcp-server"}}}"#);
    let v = parse_json(&resp);
    assert!(
        !v["result"]["isError"].as_bool().unwrap_or(false),
        "tools/call should succeed; got: {v}"
    );
}
