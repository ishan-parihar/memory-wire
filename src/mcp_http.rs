//! MCP over the server that is already running: `/mcp` and `/mcp/<bank>`.
//!
//! One [`Server`](crate::mcp::Server), one set of four tools, a second
//! transport. The tools are defined once, in `crate::mcp`, and the store is
//! shared with the REST routes; this module only decides which transport a
//! request arrives on. `memory-wire mcp` on stdio and this mount are the same
//! server, so a fix to a tool reaches both.
//!
//! # The two paths
//!
//! - `POST /mcp` — the bank is the tool call's `bank` argument, else the
//!   server's default. Byte-identical to stdio.
//! - `POST /mcp/<bank>` — the bank is pinned to the path segment, so a proxy can
//!   hand out one mount per bank. A call that also names `bank` may agree or
//!   omit it; a disagreement is refused. `Server::resolve_bank` in `crate::mcp`
//!   owns that rule and why.
//!
//! rmcp's streamable-HTTP service dispatches on the HTTP method and ignores the
//! path, so one `nest_service` covers both: the path reaches the handler as the
//! `http::request::Parts` rmcp puts in the request's extensions, and
//! [`url_bank`] reads the segment out of it. That is also why nothing here has
//! to select a service per bank — one service, no sessions at all, and the bank
//! is a property of the call rather than of the connection.
//!
//! # Security posture
//!
//! There is no authentication, exactly as on the REST routes: anything that can
//! reach the port can read, write and delete every bank through this mount too.
//! `serve` binds loopback by default and warns once on stderr otherwise, and that
//! warning covers this path because it is the same port.
//!
//! rmcp additionally rejects any request whose `Host` header is not
//! `localhost`/`127.0.0.1`/`::1`, which is its DNS-rebinding guard. That guard is
//! left at its default on purpose. It means a deliberately non-loopback bind
//! serves the REST API but answers `/mcp` with `403` until someone extends the
//! allow-list — a second, independent stop in front of an endpoint a user is more
//! likely to expose on purpose. Turning it off is a separate decision, and
//! disabling it here would be the wrong default to ship.

use std::sync::Arc;

use axum::Router;
use memory_wire::api::MemoryService;
use memory_wire::store::SqliteStore;
use rmcp::service::RequestContext;
use rmcp::transport::streamable_http_server::{
    session::never::NeverSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use rmcp::RoleServer;

use crate::mcp::{default_bank, Server};

/// The MCP mount, ready to merge into the `serve` router.
///
/// `bank` is the default the unpinned `/mcp` resolves against: the same
/// `--bank`/`MEMORY_WIRE_BANK`/`memory-wire` order `memory-wire mcp` uses, read
/// once here rather than per request so one process serves one default.
pub fn routes(svc: Arc<MemoryService<SqliteStore>>) -> Router<Arc<MemoryService<SqliteStore>>> {
    let transport = transport(Arc::clone(&svc), default_bank());
    // `with_state` before returning, because `Router::merge` takes the other
    // router already in the caller's state type. The return type has to name
    // that state: `with_state` infers its target from the expected type, and a
    // bare `Router` would infer `()` and quietly hand back a state-free router.
    Router::new().nest_service("/mcp", transport).with_state(svc)
}

/// The streamable-HTTP service, over the store this process already has open.
///
/// # Stateless, and both halves of it
///
/// [`NeverSessionManager`] and `legacy_session_mode(false)` are one decision
/// reached two ways, and setting only one leaves this server sessionful in a way
/// that is easy to miss. rmcp defaults `legacy_session_mode` to `true`, which
/// routes any request whose negotiated protocol is older than 2026-07-28 through
/// the session path — and a request with no `MCP-Protocol-Version` header at all
/// is read as 2025-03-26, so a client that simply POSTs `tools/list` would be
/// handed a 400 for the missing handshake. The flag turns that path off for
/// every client, versioned or not; the manager is what stops rmcp minting a
/// session even where the path is reachable (an SSE `GET`, a `DELETE`).
///
/// The cost is that `Mcp-Session-Id` is never issued, so a client that treated it
/// as mandatory rather than optional has nothing to send. The 2025-06-18 spec
/// makes it a MAY, and `tests/` pins that a client which handshakes anyway still
/// gets its four tools answered.
fn transport(
    svc: Arc<MemoryService<SqliteStore>>,
    bank: String,
) -> StreamableHttpService<Server, NeverSessionManager> {
    StreamableHttpService::new(
        // Sync by rmcp's design: the service builds the transport around whatever
        // the factory returns. The factory cannot fail for a reason the caller
        // can act on — the store is already open — so the only error case is
        // left to the type rather than invented here.
        move || Ok(Server::over(svc.clone(), bank.clone())),
        Arc::new(NeverSessionManager::default()),
        StreamableHttpServerConfig::default().with_legacy_session_mode(false),
    )
}

/// The bank this call's URL pins, or `None` on stdio and on the bare `/mcp`.
///
/// The `http::request::Parts` here are rmcp's own, reached through
/// `axum::http` — both resolve to the single `http` 1.5 in the lock file, so this
/// names the type without a new dependency. A stdio request never has one, which
/// is what keeps that path on the pre-existing resolution rule.
pub(crate) fn url_bank(context: &RequestContext<RoleServer>) -> Option<String> {
    let parts = context.extensions.get::<axum::http::request::Parts>()?;
    bank_from_path(parts.uri.path())
}

/// The one path segment after the mount point, if there is exactly one.
///
/// `nest_service` strips `/mcp`, so the service sees `/` for the bare mount and
/// `/<bank>` for a pinned one. A deeper path is not a mount point this build
/// offers, and treating it as one would mint a bank id containing `/` that no
/// REST route can address — so it falls through to the argument rule, which is
/// today's behaviour, rather than becoming a bank nothing else can name.
///
/// The segment is taken verbatim rather than percent-decoded: the REST routes
/// decode through axum's `Path` extractor, and adding a decoder here for a
/// convenience the `bank` argument already provides is not worth a dependency. A
/// caller who needs an exotic id uses the argument on `/mcp`.
fn bank_from_path(path: &str) -> Option<String> {
    let rest = path.strip_prefix('/')?;
    if rest.is_empty() || rest.contains('/') {
        return None;
    }
    Some(rest.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::{RETAIN, RECALL};
    use serde_json::{json, Value};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    #[test]
    fn only_a_single_path_segment_pins_a_bank() {
        assert_eq!(bank_from_path("/"), None, "the bare mount pins nothing");
        assert_eq!(bank_from_path(""), None);
        assert_eq!(bank_from_path("/demo"), Some("demo".to_string()));
        assert_eq!(bank_from_path("/acme-api"), Some("acme-api".to_string()));
        // Not a mount point: a bank id with a `/` in it is unreachable through
        // the REST routes, so it must not be created from a URL.
        assert_eq!(bank_from_path("/a/b"), None);
    }

    /// A live `serve` router on a real port, and the port it took.
    ///
    /// A real listener and a real socket, not `tower::ServiceExt::oneshot`: the
    /// claim under test is that a client on the other end of TCP gets MCP, and
    /// the session id, the SSE framing and the `Host` allow-list only exist on
    /// the wire.
    async fn serving() -> (String, Arc<MemoryService<SqliteStore>>) {
        let svc = Arc::new(MemoryService::new(SqliteStore::open_in_memory().expect("store")));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let app = crate::router(svc.clone());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        (format!("127.0.0.1:{}", addr.port()), svc)
    }

    /// One JSON-RPC request over a fresh connection, and the reply.
    ///
    /// `frame` is the whole JSON-RPC message, not just its `params`: the wire
    /// format has no separate envelope, and building a request and a reply the
    /// same way is what keeps the two from drifting.
    ///
    /// A fresh connection per request is what a real client does, and it is also
    /// the only way to read a streamable-HTTP reply: the response is SSE, and a
    /// request-wise stream stays open after the answer, so the connection is
    /// dropped as soon as the matching `id` arrives rather than at end of body.
    async fn post(
        addr: &str,
        path: &str,
        id: u64,
        frame: Value,
        session: Option<&str>,
    ) -> (String, Value) {
        let mut stream = TcpStream::connect(addr).await.expect("connect");
        let mut request = format!(
            "POST {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\n\
             Accept: application/json, text/event-stream\r\n\
             MCP-Protocol-Version: 2025-06-18\r\nContent-Length: {}\r\nConnection: close\r\n",
            frame.to_string().len()
        );
        if let Some(s) = session {
            request.push_str(&format!("Mcp-Session-Id: {s}\r\n"));
        }
        request.push_str("\r\n");
        request.push_str(&frame.to_string());
        stream.write_all(request.as_bytes()).await.expect("write");
        stream.flush().await.expect("flush");

        let mut raw = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let read = tokio::time::timeout(Duration::from_secs(10), stream.read(&mut buf))
                .await
                .expect("MCP over HTTP timed out")
                .expect("read");
            if read == 0 {
                panic!("connection closed before a reply arrived: {}", String::from_utf8_lossy(&raw));
            }
            raw.extend_from_slice(&buf[..read]);
            let text = String::from_utf8_lossy(&raw).into_owned();
            for line in text.lines() {
                let Some(payload) = line.strip_prefix("data:") else { continue };
                let Ok(message) = serde_json::from_str::<Value>(payload.trim()) else { continue };
                if message.get("id").and_then(Value::as_u64) == Some(id) {
                    let head = text.split("\r\n\r\n").next().unwrap_or("").to_string();
                    return (head, message);
                }
            }
        }
    }

    /// The `data:` payload of a `tools/call` reply, as the caller reads it.
    fn call_text(reply: &Value) -> &str {
        reply["result"]["content"][0]["text"].as_str().unwrap_or_else(|| {
            panic!("not a tool result: {reply}");
        })
    }

    fn is_error(reply: &Value) -> bool {
        reply["result"]["isError"] == json!(true)
    }

    // The whole claim, end to end, on a real socket: the four tools, a retain
    // and a recall into the bank the URL names, and the same four tool
    // definitions stdio serves. The `initialize` at the top is deliberate even
    // though the mount no longer requires it: it is the compat half, proving a
    // client that still handshakes is answered and issued nothing to carry.
    #[tokio::test]
    async fn a_client_can_reach_the_four_tools_over_http_with_the_bank_in_the_path() {
        let (addr, _svc) = serving().await;

        // initialize -> answered, and with no session id to carry forward.
        let init = json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18", "capabilities": {},
                "clientInfo": { "name": "roundtrip", "version": "1" }
            }
        });
        let (head, reply) = post(&addr, "/mcp/demo", 1, init, None).await;
        assert!(head.starts_with("HTTP/1.1 200"), "{head}");
        assert!(
            !head.to_ascii_lowercase().contains("mcp-session-id"),
            "a stateless mount issues no session id: {head}"
        );
        assert_eq!(reply["result"]["serverInfo"]["name"], json!("memory-wire"), "{reply}");
        assert!(reply["result"]["capabilities"]["tools"].is_object(), "{reply}");

        // tools/list -> the same definitions `memory-wire mcp` serves, byte for
        // byte. This is the parity assertion: a second implementation of the
        // tools would diverge here.
        let (head, reply) = post(
            &addr,
            "/mcp/demo",
            2,
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }),
            None,
        )
        .await;
        assert!(head.starts_with("HTTP/1.1 200"), "{head}");
        let over_http = reply["result"]["tools"].clone();
        // The tool listing is a static of the production server, so it is named
        // with its concrete store type: `Server`'s default parameter makes the
        // bare `Server` spelling mean the same thing in a type, but an
        // associated call carries no such default.
        let over_stdio =
            serde_json::to_value(crate::mcp::Server::<memory_wire::store::SqliteStore>::tools())
                .expect("tools serialize");
        assert_eq!(over_http, over_stdio, "HTTP and stdio must serve one tool set");

        // retain, with no `bank` argument: the URL decides.
        let (head, reply) = post(
            &addr,
            "/mcp/demo",
            3,
            json!({
                "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": { "name": RETAIN, "arguments": { "content": "auth uses jose middleware" } }
            }),
            None,
        )
        .await;
        assert!(head.starts_with("HTTP/1.1 200"), "{head}");
        assert!(!is_error(&reply), "retain failed: {reply}");
        assert!(call_text(&reply).contains("\"id\""), "{reply}");

        // recall, same shape stdio serves: a bare JSON array of content strings.
        let (head, reply) = post(
            &addr,
            "/mcp/demo",
            4,
            json!({
                "jsonrpc": "2.0", "id": 4, "method": "tools/call",
                "params": { "name": RECALL, "arguments": { "query": "jose" } }
            }),
            None,
        )
        .await;
        assert!(head.starts_with("HTTP/1.1 200"), "{head}");
        assert!(!is_error(&reply), "recall failed: {reply}");
        let hits: Vec<String> = serde_json::from_str(call_text(&reply)).expect("a JSON array");
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert!(hits[0].contains("jose"), "{hits:?}");

        // The memory really is in the bank the URL named, and in no other.
        let svc = _svc;
        assert_eq!(svc.recall("demo", "jose", 2000).expect("recall").len(), 1);
        assert!(svc.recall("other", "jose", 2000).expect("recall").is_empty());
    }

    // The reason the mount exists in this shape: the 2026-07-28 revision has no
    // initialize at all, so a client that POSTs a method cold — no handshake,
    // no `Mcp-Session-Id`, no `MCP-Protocol-Version` header to say so — is the
    // normal case rather than the edge case, and is answered the same way.
    #[tokio::test]
    async fn a_method_posted_with_no_handshake_and_no_session_id_is_served() {
        let (addr, _svc) = serving().await;
        for (id, method) in [(1, "tools/list"), (2, "resources/list"), (3, "prompts/list")] {
            let (head, reply) = post(
                &addr,
                "/mcp",
                id,
                json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": {} }),
                None,
            )
            .await;
            assert!(head.starts_with("HTTP/1.1 200"), "{method} without a handshake: {head}");
            assert!(
                !head.to_ascii_lowercase().contains("mcp-session-id"),
                "{method} was issued a session id: {head}"
            );
            assert!(reply.get("result").is_some(), "{method} produced no result: {reply}");
            assert!(reply.get("error").is_none(), "{method} failed: {reply}");
        }
        // And the tools are the contract four, not a subset or a superset.
        let (_, reply) = post(
            &addr,
            "/mcp",
            4,
            json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/list", "params": {} }),
            None,
        )
        .await;
        let names: Vec<&str> = reply["result"]["tools"]
            .as_array()
            .expect("tools is an array")
            .iter()
            .filter_map(|t| t["name"].as_str())
            .collect();
        assert_eq!(names.len(), 4, "{names:?}");
    }

    // The decision `Server::resolve_bank` makes, over the wire: an argument that
    // agrees is fine, one that disagrees is refused rather than overridden.
    #[tokio::test]
    async fn a_bank_argument_may_agree_with_the_url_and_may_not_disagree_with_it() {
        let (addr, _svc) = serving().await;
        let init = json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18", "capabilities": {},
                "clientInfo": { "name": "pin", "version": "1" }
            }
        });
        let (head, _) = post(&addr, "/mcp/pinned", 1, init, None).await;
        assert!(head.starts_with("HTTP/1.1 200"), "{head}");

        // Agreeing: served, out of the pinned bank.
        let (_, reply) = post(
            &addr,
            "/mcp/pinned",
            2,
            json!({
                "jsonrpc": "2.0", "id": 2, "method": "tools/call",
                "params": { "name": RETAIN, "arguments": { "bank": "pinned", "content": "agreed" } }
            }),
            None,
        )
        .await;
        assert!(!is_error(&reply), "a matching bank must be served: {reply}");
        assert_eq!(_svc.recall("pinned", "agreed", 2000).expect("recall").len(), 1);

        // Disagreeing: refused, and nothing lands anywhere.
        let (_, reply) = post(
            &addr,
            "/mcp/pinned",
            3,
            json!({
                "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": { "name": RETAIN, "arguments": { "bank": "elsewhere", "content": "smuggled" } }
            }),
            None,
        )
        .await;
        assert!(is_error(&reply), "a mismatched bank must be refused: {reply}");
        let text = call_text(&reply);
        assert!(text.contains("pinned") && text.contains("elsewhere"), "the message names both: {text}");
        assert!(_svc.recall("elsewhere", "smuggled", 2000).expect("recall").is_empty());
        assert!(_svc.recall("pinned", "smuggled", 2000).expect("recall").is_empty());
    }

    // The bare mount is the other half of the feature: no bank in the URL, so
    // the call's own argument resolves exactly as it does on stdio.
    #[tokio::test]
    async fn the_bare_mount_resolves_the_bank_from_the_call() {
        let (addr, _svc) = serving().await;
        let init = json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18", "capabilities": {},
                "clientInfo": { "name": "bare", "version": "1" }
            }
        });
        let (head, _) = post(&addr, "/mcp", 1, init, None).await;
        assert!(head.starts_with("HTTP/1.1 200"), "{head}");

        let (_, reply) = post(
            &addr,
            "/mcp",
            2,
            json!({
                "jsonrpc": "2.0", "id": 2, "method": "tools/call",
                "params": { "name": RETAIN, "arguments": { "bank": "chosen", "content": "by argument" } }
            }),
            None,
        )
        .await;
        assert!(!is_error(&reply), "{reply}");
        assert_eq!(_svc.recall("chosen", "argument", 2000).expect("recall").len(), 1);
    }

    // rmcp's DNS-rebinding guard is left at its default, which is a second stop
    // in front of an endpoint a user is likelier to expose on purpose. Pinned so
    // that turning it off is a decision somebody has to make on purpose.
    #[tokio::test]
    async fn a_non_loopback_host_header_is_refused_on_the_mcp_path() {
        let (addr, _svc) = serving().await;
        let mut stream = TcpStream::connect(&addr).await.expect("connect");
        let body = json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18", "capabilities": {},
                "clientInfo": { "name": "rebind", "version": "1" }
            }
        })
        .to_string();
        let request = format!(
            "POST /mcp HTTP/1.1\r\nHost: evil.example.com\r\nContent-Type: application/json\r\n\
             Accept: application/json, text/event-stream\r\nMCP-Protocol-Version: 2025-06-18\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(request.as_bytes()).await.expect("write");
        let mut raw = Vec::new();
        let mut buf = [0u8; 1024];
        loop {
            let read = tokio::time::timeout(Duration::from_secs(10), stream.read(&mut buf))
                .await
                .expect("timed out")
                .expect("read");
            if read == 0 {
                break;
            }
            raw.extend_from_slice(&buf[..read]);
        }
        let text = String::from_utf8_lossy(&raw);
        assert!(
            text.starts_with("HTTP/1.1 403"),
            "a foreign Host must not reach MCP: {}",
            &text[..text.len().min(200)]
        );
    }
}
