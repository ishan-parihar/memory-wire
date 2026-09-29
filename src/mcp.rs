//! MCP server: the same four operations, over JSON-RPC on stdio.
//!
//! Every tool is a thin adapter onto [`MemoryService`]. Nothing here re-derives
//! retrieval, redaction, or budgeting — an agent calling `memory_recall` over
//! MCP must get byte-identical answers to an agent calling
//! `POST /banks/:id/recall`, and the only way to guarantee that is to route both
//! through the same service methods.
//!
//! Failures are split in two, because MCP renders them differently:
//!
//! - an unknown tool name is unroutable, so it is a JSON-RPC
//!   `MethodNotFound` protocol error;
//! - a tool that ran and failed (blank bank, unknown bank, storage error) is a
//!   `CallToolResult` with `isError: true` and the same opaque message the HTTP
//!   surface returns, so a client never sees driver text or on-disk paths.
//!
//! # Bank scoping
//!
//! Two transports, one resolution rule. Over stdio (`memory-wire mcp`) and over
//! the bare `POST /mcp`, the bank is the call's `bank` argument, else the server
//! default — the rule below, unchanged. `POST /mcp/<bank>` pins the bank to the
//! path segment so a proxy can hand out one bank per mount point; there the
//! argument may agree or be absent, and a disagreement is refused rather than
//! overridden (`Server::resolve_bank` says why). `crate::mcp_http` owns the
//! mount; nothing in this file knows there is an HTTP at all.

use std::path::PathBuf;
use std::sync::Arc;

use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, ErrorCode, ErrorData, Implementation,
    ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
    ToolAnnotations,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData as McpError, ServerHandler, ServiceExt};
use serde_json::{json, Map, Value};

use memory_wire::api::{http_error, parse_update_mode, MemoryService};
use memory_wire::store::{default_db_path, SqliteStore};

/// Bank used when neither `--bank` nor `MEMORY_WIRE_BANK` names one.
pub const DEFAULT_BANK: &str = "memory-wire";

/// `memory_retain` — the only tool that writes.
pub const RETAIN: &str = "memory_retain";
/// `memory_recall` — budget-capped, tag-filterable retrieval.
pub const RECALL: &str = "memory_recall";
/// `memory_reflect` — top-hit citation.
pub const REFLECT: &str = "memory_reflect";
/// `memory_bank_config_get` — the bank's stored config object.
pub const CONFIG_GET: &str = "memory_bank_config_get";

/// A `tools/call` outcome: either a routable tool error, or no such tool.
enum CallError {
    /// The tool exists and ran; it failed in a way the caller must see.
    Failed(String),
    /// No such tool — a protocol-level `MethodNotFound`.
    UnknownTool(String),
}

impl From<memory_wire::api::ApiError> for CallError {
    /// The service already classifies every failure into a client-safe message;
    /// reusing `http_error` keeps MCP and HTTP from drifting apart.
    fn from(e: memory_wire::api::ApiError) -> Self {
        CallError::Failed(http_error(&e).1.to_string())
    }
}

/// The MCP surface: one service plus the bank tools default to.
pub struct Server {
    svc: MemoryService<SqliteStore>,
    bank: String,
}

/// Annotations for one tool.
///
/// `read_only` is the only thing that varies: nothing here is destructive —
/// `memory_retain` adds a memory, it never removes or overwrites one — and
/// none of the four is idempotent, because a repeated retain writes a second
/// memory rather than recognizing the first.
fn annotations(read_only: bool) -> ToolAnnotations {
    let mut a = ToolAnnotations::default();
    a.read_only_hint = Some(read_only);
    a.destructive_hint = Some(false);
    a.idempotent_hint = Some(false);
    a.open_world_hint = Some(false);
    a
}

impl Server {
    /// Open the file-backed store and bind the server to one default bank.
    ///
    /// No bank is created here: a retain creates its own bank implicitly
    /// (matching the HTTP route), and `memory_bank_config_get` must 404 on a
    /// bank that was never created rather than invent one.
    pub fn open(db: Option<PathBuf>, bank: String) -> anyhow::Result<Self> {
        let path = db.unwrap_or_else(default_db_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let svc = MemoryService::new(SqliteStore::open(&path)?);
        Ok(Self { svc, bank })
    }

    /// The same MCP surface over a store this process already has open.
    ///
    /// The HTTP mount in `crate::mcp_http` holds one service for the whole
    /// process and hands every MCP session a share of it, rather than opening a
    /// second SQLite connection per session. A session is not a database, and a
    /// connection per session is both a per-session cost and a way for the MCP
    /// surface and the REST surface to hold different views of what is
    /// committed. The share is the `Arc` the service already keeps internally, so
    /// this clones a pointer rather than a store.
    pub(crate) fn over(svc: Arc<MemoryService<SqliteStore>>, bank: String) -> Self {
        Self {
            svc: MemoryService { store: svc.store.clone() },
            bank,
        }
    }

    /// The four tool definitions, in listing order.
    pub fn tools() -> Vec<Tool> {
        [
            Tool::new(
                RETAIN,
                "Retain a memory in a bank. Content and context are redacted \
                 server-side before the write. Creates the bank if it is new.",
                schema(json!({
                    "type": "object",
                    "properties": {
                        "bank": { "type": "string", "description": "Bank id; defaults to the server's bank." },
                        "content": { "type": "string", "description": "What to remember. Required." },
                        "context": { "type": "string", "description": "Optional capture context; redacted too." },
                        "tags": { "type": "array", "items": { "type": "string" }, "description": "Tags for this memory." },
                        "document_id": { "type": "string", "description": "Re-write this document instead of adding another memory." },
                        "update_mode": { "type": "string", "enum": ["replace", "append"], "description": "Whether a document_id write overwrites or extends it. Defaults to replace." }
                    },
                    "required": ["content"]
                })),
            )
            .with_annotations(annotations(false)),
            Tool::new(
                RECALL,
                "Recall memories from a bank, most relevant first, as a JSON \
                 array of content strings. `budget` is a hard token cap, \
                 `tags` restricts the search to memories carrying any of them, \
                 and `format: \"full\"` returns {id, score, content} objects \
                 instead so the caller can cite what it got.",
                schema(json!({
                    "type": "object",
                    "properties": {
                        "bank": { "type": "string", "description": "Bank id; defaults to the server's bank." },
                        "query": { "type": "string", "description": "Search text. Required." },
                        "budget": { "type": "integer", "description": "Token cap; falls back to the bank's recallMaxTokens, then 2000." },
                        "tags": { "type": "array", "items": { "type": "string" }, "description": "Only memories carrying any of these tags." },
                        "format": { "type": "string", "enum": ["full"], "description": "Return {id, score, content} objects instead of bare strings." }
                    },
                    "required": ["query"]
                })),
            )
            .with_annotations(annotations(true)),
            Tool::new(
                REFLECT,
                "Answer a question from a bank, citing the memory it came from. \
                 Currently top-hit citation, not synthesis — there is no LLM in the loop.",
                schema(json!({
                    "type": "object",
                    "properties": {
                        "bank": { "type": "string", "description": "Bank id; defaults to the server's bank." },
                        "query": { "type": "string", "description": "The question. Required." },
                        "tags": { "type": "array", "items": { "type": "string" }, "description": "Only consider memories carrying any of these tags." }
                    },
                    "required": ["query"]
                })),
            )
            .with_annotations(annotations(true)),
            Tool::new(
                CONFIG_GET,
                "Read a bank's stored config object (recallMaxTokens, retainTags, \
                 and any keys this build does not act on, served verbatim).",
                schema(json!({
                    "type": "object",
                    "properties": {
                        "bank": { "type": "string", "description": "Bank id; defaults to the server's bank." }
                    },
                    "required": []
                })),
            )
            .with_annotations(annotations(true)),
        ]
        .into_iter()
        .collect()
    }

    /// Run one tool with no URL pin — the stdio and bare-`/mcp` resolution rule.
    ///
    /// Test-only, and deliberately so: this module's tests call it directly,
    /// which is what pins the unpinned rule without editing a single one of them
    /// when `/mcp/<bank>` arrived. Production reaches the same rule through
    /// `Self::call_scoped` with `None`.
    #[cfg(test)]
    fn call(&self, name: &str, args: &Value) -> Result<Value, CallError> {
        self.call_scoped(name, args, None)
    }

    /// Run one tool, optionally with the bank pinned by the URL.
    ///
    /// `url_bank` is `Some` only for a mount that names its bank in the path
    /// (`/mcp/<bank>`, see `crate::mcp_http`); stdio and the bare `/mcp` pass
    /// `None` and are byte-identical to before this argument existed.
    ///
    /// Argument resolution is uniform: an explicit `bank` wins, else the
    /// server's default. A present-but-blank `bank` is a caller error, not a
    /// fallback — silently serving the default bank instead would write the
    /// memory somewhere the caller did not name.
    fn call_scoped(
        &self,
        name: &str,
        args: &Value,
        url_bank: Option<&str>,
    ) -> Result<Value, CallError> {
        let bank = self.resolve_bank(args, url_bank)?;
        let tags = arg_tags(args);
        match name {
            RETAIN => {
                let content = required_str(args, "content")?;
                let mode = parse_update_mode(arg_str(args, "update_mode").as_deref())?;
                // The service declares the bank itself; declaring it here too was
                // a second write per retain for nothing.
                let id = self.svc.retain_doc(
                    &bank,
                    content,
                    arg_str(args, "context"),
                    &tags,
                    arg_str(args, "document_id").as_deref(),
                    mode,
                )?;
                Ok(json!({ "id": id }))
            }
            RECALL => {
                let query = required_str(args, "query")?;
                let budget = args
                    .get("budget")
                    .and_then(Value::as_u64)
                    .and_then(|n| usize::try_from(n).ok());
                let hits = self.svc.recall_filtered(&bank, query, budget, &tags)?;
                // `format: "full"` is the citing shape; the default stays a bare
                // array of content strings, exactly as the HTTP route serves it.
                Ok(if arg_str(args, "format").as_deref() == Some("full") {
                    Value::Array(
                        hits.iter()
                            .map(|h| {
                                json!({ "id": h.memory.id, "score": h.score, "content": h.memory.content })
                            })
                            .collect(),
                    )
                } else {
                    Value::Array(
                        hits.into_iter().map(|h| Value::String(h.memory.content)).collect(),
                    )
                })
            }
            REFLECT => {
                let query = required_str(args, "query")?;
                Ok(Value::String(self.svc.reflect(&bank, query, &tags)?))
            }
            CONFIG_GET => {
                let raw = self.svc.bank_config(&bank)?;
                Ok(serde_json::from_str(&raw).unwrap_or_else(|_| json!({})))
            }
            other => Err(CallError::UnknownTool(other.to_string())),
        }
    }

    /// The bank one call touches, from the URL pin and the call's own argument.
    ///
    /// Unpinned, this is the rule that has always applied: the `bank` argument
    /// wins, else the server's default, and a wrong-typed or blank `bank` is a
    /// caller error rather than a silent fallback.
    ///
    /// # Why a mismatched argument is refused rather than overridden
    ///
    /// `/mcp/<bank>` exists so a proxy or a second mount can hand out one bank
    /// per endpoint. "Pinned" has to mean the argument cannot talk it out of
    /// that: if the URL won silently, a client that asked for bank `other`
    /// would get a successful `retain` into `demo` and a successful `recall`
    /// that could not see what it had just written — the exact
    /// wrote-it-somewhere-the-caller-did-not-name failure the blank-bank rule
    /// above already refuses. An error the caller reads beats a success that
    /// quietly lands in a different namespace, and `CallError::Failed` is this
    /// server's normal tool-level failure channel (`isError: true` with the
    /// message), not a JSON-RPC fault: the tool exists, is routable, and ran.
    fn resolve_bank(&self, args: &Value, url_bank: Option<&str>) -> Result<String, CallError> {
        let asked = match args.get("bank") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) => Some(s.as_str()),
            Some(_) => return Err(CallError::Failed("invalid bank id".to_string())),
        };
        match (url_bank, asked) {
            (Some(pinned), None) => Ok(pinned.to_string()),
            (Some(pinned), Some(s)) if s == pinned => Ok(pinned.to_string()),
            (Some(pinned), Some(s)) => Err(CallError::Failed(format!(
                "`bank` is pinned to `{pinned}` by this endpoint; refusing `{s}`"
            ))),
            (None, Some(s)) => Ok(s.to_string()),
            (None, None) => Ok(self.bank.clone()),
        }
    }
}

impl ServerHandler for Server {
    fn get_info(&self) -> ServerInfo {
        // `ServerInfo`/`Implementation` are `#[non_exhaustive]`, so they are
        // built from `default()` and mutated rather than struct-literal'd.
        let mut server_info = Implementation::from_build_env();
        server_info.name = "memory-wire".to_string();
        server_info.title = Some("memory-wire".to_string());
        server_info.version = env!("CARGO_PKG_VERSION").to_string();

        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.server_info = server_info;
        info.instructions = Some(
            "Bank-isolated agent memory. Retain what matters, recall before \
             answering, and treat a returned `based on [<id>]:` reflect answer \
             as a citation rather than as the current truth."
                .to_string(),
        );
        info
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult { tools: Self::tools(), ..Default::default() })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let args = request.arguments.map(Value::Object).unwrap_or(Value::Null);
        // `None` on stdio, which never puts an HTTP request in the context, so
        // the stdio path is this module's pre-existing resolution rule verbatim.
        let pinned = crate::mcp_http::url_bank(&context);
        match self.call_scoped(&request.name, &args, pinned.as_deref()) {
            Ok(v) => Ok(CallToolResult::success(vec![Content::text(v.to_string())])),
            // Unroutable: a JSON-RPC error the client surfaces opaquely.
            Err(CallError::UnknownTool(name)) => Err(ErrorData::new(
                ErrorCode::METHOD_NOT_FOUND,
                format!("unknown tool: {name}"),
                None,
            )),
            // The tool ran and failed: the caller must be able to read why.
            Err(CallError::Failed(why)) => Ok(CallToolResult::error(vec![Content::text(why)])),
        }
    }
}

/// Serve MCP on stdio until the client disconnects.
pub async fn serve(db: Option<PathBuf>, bank: String) -> anyhow::Result<()> {
    let server = Server::open(db, bank)?;
    tracing::info!("memory-wire mcp on stdio (bank {})", server.bank);
    // stderr, not stdout: stdout is the JSON-RPC channel.
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}

/// `--bank`, else `MEMORY_WIRE_BANK`, else [`DEFAULT_BANK`].
pub fn default_bank() -> String {
    std::env::var("MEMORY_WIRE_BANK")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_BANK.to_string())
}

/// A JSON Schema object, as MCP wants it.
fn schema(value: Value) -> Arc<Map<String, Value>> {
    Arc::new(serde_json::from_value(value).expect("literal schema is an object"))
}

/// A required string argument, or a caller-visible failure.
fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, CallError> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| CallError::Failed(format!("`{key}` is required and must be a string")))
}

/// An optional string argument; `null`, absent, and a wrong type are all absent.
fn arg_str(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(String::from)
}

/// An optional `tags: string[]`; anything else is treated as no filter.
fn arg_tags(args: &Value) -> Vec<String> {
    args.get("tags")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::service::serve_directly;
    use rmcp::transport::async_rw::AsyncRwTransport;
    use memory_wire::store::Store;
    use rmcp::RoleServer;
    use std::time::Duration;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};
    use tokio::time::timeout;

    /// The client half of a duplex: a writer plus a line reader.
    struct Client {
        out: WriteHalf<tokio::io::DuplexStream>,
        inp: BufReader<ReadHalf<tokio::io::DuplexStream>>,
    }

    impl Client {
        async fn send(&mut self, v: Value) {
            self.out.write_all(format!("{v}\n").as_bytes()).await.expect("write");
            self.out.flush().await.expect("flush");
        }

        async fn recv(&mut self) -> Value {
            let mut line = String::new();
            timeout(Duration::from_secs(10), self.inp.read_line(&mut line))
                .await
                .expect("MCP response timed out")
                .expect("read");
            serde_json::from_str(line.trim()).unwrap_or_else(|e| panic!("bad reply {line:?}: {e}"))
        }

        /// `request` -> one response, skipping notifications.
        async fn request(&mut self, id: u64, method: &str, params: Value) -> Value {
            self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))
                .await;
            loop {
                let reply = self.recv().await;
                // A server may interleave notifications; only the id we sent counts.
                if reply.get("id").and_then(Value::as_u64) == Some(id) {
                    return reply;
                }
            }
        }
    }

    /// A live MCP session over an in-memory duplex: the same newline-delimited
    /// JSON-RPC `memory-wire mcp` puts on stdout, minus the process.
    fn session(s: Server) -> (Client, rmcp::service::RunningService<RoleServer, Server>) {
        let (server_io, client_io) = tokio::io::duplex(64 * 1024);
        let (server_r, server_w) = tokio::io::split(server_io);
        let transport = AsyncRwTransport::<RoleServer, _, _>::new(server_r, server_w);
        // Held by the caller for the life of the test: dropping it stops the loop.
        let running = serve_directly(s, transport, None);
        let (client_r, client_w) = tokio::io::split(client_io);
        (Client { out: client_w, inp: BufReader::new(client_r) }, running)
    }

    fn server() -> Server {
        Server {
            svc: MemoryService::new(SqliteStore::open_in_memory().expect("open")),
            bank: "agent".to_string(),
        }
    }

    /// Run one async test body on its own runtime.
    fn run<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt")
            .block_on(f)
    }

    fn call_json(s: &Server, name: &str, args: Value) -> Result<Value, String> {
        s.call(name, &args).map_err(|e| match e {
            CallError::Failed(why) => why,
            CallError::UnknownTool(n) => panic!("unexpected unknown tool {n}"),
        })
    }

    // The full MCP handshake, against a live session.
    #[test]
    fn initialize_should_advertise_the_tools_capability_and_the_binary_version() {
        run(async {
            let (mut c, _running) = session(server());
            let reply = c
                .request(
                    1,
                    "initialize",
                    json!({
                        "protocolVersion": "2025-06-18",
                        "capabilities": {},
                        "clientInfo": { "name": "test", "version": "1" }
                    }),
                )
                .await;
            let result = reply.get("result").unwrap_or_else(|| panic!("{reply}"));
            assert_eq!(result["serverInfo"]["name"], "memory-wire");
            assert_eq!(result["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
            assert!(
                result["capabilities"]["tools"].is_object(),
                "the tools capability must be advertised: {result}"
            );
        });
    }

    // `tools/list` over the wire: four tools, each annotated.
    #[test]
    fn tools_list_should_show_four_annotated_tools() {
        run(async {
            let (mut c, _running) = session(server());
            let reply = c.request(2, "tools/list", json!({})).await;
            let tools = reply["result"]["tools"].as_array().unwrap_or_else(|| panic!("{reply}"));
            assert_eq!(tools.len(), 4, "{reply}");

            let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
            assert_eq!(names, vec![RETAIN, RECALL, REFLECT, CONFIG_GET], "{names:?}");

            for tool in tools {
                let name = tool["name"].as_str().unwrap_or_default();
                let ann = &tool["annotations"];
                assert_eq!(ann["destructiveHint"], json!(false), "{name} must not be destructive");
                // Only retain writes; the other three must be marked read-only.
                let expected = name != RETAIN;
                assert_eq!(ann["readOnlyHint"], json!(expected), "{name}");
                assert_eq!(
                    tool["inputSchema"]["type"], json!("object"),
                    "{name} needs an object schema"
                );
            }
            // `content` is required for retain, `query` for recall/reflect, and
            // config-get takes no required argument at all.
            let required = |name: &str| -> Vec<String> {
                tools
                    .iter()
                    .find(|t| t["name"] == json!(name))
                    .and_then(|t| t["inputSchema"]["required"].as_array())
                    .map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect())
                    .unwrap_or_default()
            };
            assert_eq!(required(RETAIN), vec!["content"]);
            assert_eq!(required(RECALL), vec!["query"]);
            assert_eq!(required(REFLECT), vec!["query"]);
            assert!(required(CONFIG_GET).is_empty());
        });
    }

    // retain then recall over the wire, both against the server's default bank.
    #[test]
    fn tools_call_should_retain_then_recall_hits() {
        run(async {
            let (mut c, _running) = session(server());

            let kept = c
                .request(3, "tools/call", json!({
                    "name": RETAIN,
                    "arguments": { "content": "auth uses jose middleware" }
                }))
                .await;
            assert_ne!(kept["result"]["isError"], json!(true), "{kept}");
            assert!(kept["result"]["content"][0]["text"].as_str().unwrap().contains("\"id\""));

            let found = c
                .request(4, "tools/call", json!({
                    "name": RECALL,
                    "arguments": { "query": "jose" }
                }))
                .await;
            assert_eq!(found["result"]["isError"], json!(false), "{found}");
            let text = found["result"]["content"][0]["text"].as_str().unwrap().to_string();
            let hits: Vec<String> = serde_json::from_str(&text).expect("a JSON array of strings");
            assert_eq!(hits.len(), 1, "{hits:?}");
            assert!(hits[0].contains("jose"), "{hits:?}");
        });
    }

    // An unknown tool is unroutable, so it is a JSON-RPC protocol error
    // (-32601) — not a tool result, and not a crash.
    #[test]
    fn an_unknown_tool_should_be_a_method_not_found_protocol_error() {
        run(async {
            let (mut c, _running) = session(server());
            let reply = c
                .request(5, "tools/call", json!({ "name": "memory_nope", "arguments": {} }))
                .await;
            assert_eq!(reply["error"]["code"], json!(-32601), "{reply}");
            assert!(
                reply["error"]["message"].as_str().unwrap().contains("memory_nope"),
                "{reply}"
            );
            assert!(reply.get("result").is_none(), "an error carries no result: {reply}");
        });
    }

    // A tool that runs and fails is `isError: true` with readable text, and
    // never leaks driver text or on-disk paths.
    #[test]
    fn a_failing_tool_should_be_an_is_error_text_result_never_a_panic() {
        run(async {
            let (mut c, _running) = session(server());
            for (id, name, args) in [
                (10u64, RETAIN, json!({})),                             // missing `content`
                (11, RETAIN, json!({ "bank": "  ", "content": "x" })),  // blank bank
                (12, RECALL, json!({ "query": "x", "bank": 7 })),      // wrong-typed bank
                (13, CONFIG_GET, json!({ "bank": "ghost" })),           // bank never created
            ] {
                let reply = c
                    .request(id, "tools/call", json!({ "name": name, "arguments": args }))
                    .await;
                assert_eq!(reply["result"]["isError"], json!(true), "{name} {args}: {reply}");
                let text = reply["result"]["content"][0]["text"].as_str().unwrap_or_default();
                assert!(!text.is_empty(), "{name} {args} must explain itself");
                assert!(!text.contains("FOREIGN KEY"), "raw driver text leaked: {text}");
                assert!(!text.contains('/'), "a path leaked: {text}");
            }
            // The session survives every one of them.
            let alive = c.request(20, "tools/list", json!({})).await;
            assert!(alive["result"]["tools"].is_array(), "{alive}");
        });
    }

    #[test]
    fn retain_should_default_to_the_server_bank_and_honor_a_per_call_override() {
        let s = server();
        let id = call_json(&s, RETAIN, json!({ "content": "rate limiting via token bucket" }))
            .expect("retain");
        let id = id["id"].as_str().expect("id").to_string();
        assert!(
            s.svc.store.get("agent", &id).expect("get").is_some(),
            "no bank argument must land in the server's bank"
        );

        call_json(&s, RETAIN, json!({ "bank": "other", "content": "second bank note" }))
            .expect("retain other");
        assert_eq!(s.svc.recall("other", "second bank", 2000).expect("recall").len(), 1);
        assert!(s.svc.recall("agent", "second bank", 2000).expect("recall").is_empty());
    }

    #[test]
    fn config_get_should_serve_the_stored_object_verbatim() {
        let s = server();
        call_json(&s, RETAIN, json!({ "content": "seed the bank" })).expect("retain");
        s.svc
            .set_bank_config("agent", r#"{"recallMaxTokens":32,"retainTags":["ops"]}"#)
            .expect("config");

        let got = call_json(&s, CONFIG_GET, json!({})).expect("config");
        assert_eq!(got["recallMaxTokens"], json!(32));
        assert_eq!(got["retainTags"], json!(["ops"]));
    }

    #[test]
    fn tags_should_round_trip_from_a_tool_call_into_a_filtered_recall() {
        let s = server();
        call_json(&s, RETAIN, json!({ "content": "auth uses jose middleware", "tags": ["Auth"] }))
            .expect("retain tagged");
        call_json(&s, RETAIN, json!({ "content": "auth uses jose untagged" })).expect("retain plain");

        let tagged: Vec<String> = serde_json::from_value(
            call_json(&s, RECALL, json!({ "query": "jose", "tags": ["auth"] })).expect("recall"),
        )
        .expect("array");
        assert_eq!(tagged.len(), 1, "{tagged:?}");
        assert!(tagged[0].contains("middleware"), "{tagged:?}");

        // Both memories are still visible without the filter.
        let all: Vec<String> = serde_json::from_value(
            call_json(&s, RECALL, json!({ "query": "jose" })).expect("all"),
        )
        .expect("array");
        assert_eq!(all.len(), 2, "{all:?}");
    }

    #[test]
    fn an_explicit_budget_should_win_over_the_bank_default() {
        let s = server();
        call_json(&s, RETAIN, json!({ "content": "jose ".repeat(400) })).expect("retain");
        s.svc.set_bank_config("agent", r#"{"recallMaxTokens":5}"#).expect("config");

        let bank_default: Vec<String> = serde_json::from_value(
            call_json(&s, RECALL, json!({ "query": "jose" })).expect("bank default"),
        )
        .expect("array");
        let explicit: Vec<String> = serde_json::from_value(
            call_json(&s, RECALL, json!({ "query": "jose", "budget": 200 })).expect("explicit"),
        )
        .expect("array");

        assert_eq!(bank_default[0].chars().count(), 5 * 4, "bank recallMaxTokens must apply");
        assert!(
            explicit[0].chars().count() > bank_default[0].chars().count(),
            "an explicit budget must override the bank default"
        );
    }

    // `format: "full"` is the citing shape over MCP too; the default stays the
    // bare array of strings so an existing caller sees no change.
    #[test]
    fn recall_should_serve_the_full_shape_only_when_asked() {
        let s = server();
        let kept = call_json(&s, RETAIN, json!({ "content": "auth uses jose middleware" }))
            .expect("retain");
        let id = kept["id"].as_str().expect("id");

        let plain: Vec<String> =
            serde_json::from_value(call_json(&s, RECALL, json!({ "query": "jose" })).expect("recall"))
                .expect("array of strings");
        assert_eq!(plain.len(), 1, "{plain:?}");

        let full: Vec<Value> = serde_json::from_value(
            call_json(&s, RECALL, json!({ "query": "jose", "format": "full" })).expect("full"),
        )
        .expect("array of objects");
        assert_eq!(full.len(), 1, "{full:?}");
        assert_eq!(full[0]["id"], json!(id), "{full:?}");
        assert_eq!(full[0]["content"].as_str(), Some("auth uses jose middleware"));
        // Same fused RRF value the HTTP surface serves: rank 1 in both streams,
        // each contributing its stream's weight. One rule, two adapters — the
        // score is not re-derived per surface. Built from the shipped weights so
        // the assertion tracks the fusion rather than a literal that went stale
        // when Phase E1 moved the overlap weight.
        let w = memory_wire::recall::FusionWeights::SHIPPED;
        let expected = w.bm25 / (w.k + 1.0) + w.overlap / (w.k + 1.0);
        assert_eq!(full[0]["score"].as_f64(), Some(expected), "{full:?}");
    }

    // The documented default when no flag and no env var say otherwise.
    // `default_bank()` reads process-global state, so it is asserted alone.
    #[test]
    fn default_bank_should_fall_back_to_the_documented_name() {
        assert_eq!(default_bank(), DEFAULT_BANK);
    }

    /// One real `memory-wire mcp` process driven over a piped stdin: the frames
    /// written, then EOF, which is how a client says it is done. Returns both
    /// streams so a test can check where the logging went.
    fn run_mcp(dir: &std::path::Path, level: &str, frames: &[&str]) -> (String, String) {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new(crate::bin())
            .args(["mcp", "--bank", "agent"])
            .arg("--db")
            .arg(dir.join(format!("{level}.db")))
            .env("RUST_LOG", level)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn memory-wire mcp");
        {
            let mut pipe = child.stdin.take().expect("stdin pipe");
            for frame in frames {
                writeln!(pipe, "{frame}").expect("write frame");
            }
            // The handle drops here, closing the pipe.
        }
        let out = child.wait_with_output().expect("wait for mcp");
        assert!(
            out.status.success(),
            "mcp exited {:?}: {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
        (
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }

    // H5.1 — stdout is the protocol channel, so not one byte of logging may
    // land on it at any level. The in-process sessions above cannot catch this:
    // they have no `main`, no subscriber, and therefore no stderr to leak into.
    // One process per level, and every non-empty stdout line has to parse as
    // JSON-RPC 2.0 — the rule the mechanism already follows, pinned.
    #[test]
    fn the_mcp_stdout_should_be_pure_json_rpc_at_every_log_level() {
        let frames = [
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"hygiene","version":"1"}}}"#,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"memory_recall","arguments":{"query":"jose"}}}"#,
        ];
        let dir = std::env::temp_dir().join(format!("mw-mcp-hygiene-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("tmp");
        for level in ["off", "info", "debug", "trace"] {
            let (stdout, stderr) = run_mcp(&dir, level, &frames);
            let lines: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();
            assert!(lines.len() >= 2, "RUST_LOG={level} left no protocol: {stdout:?}");
            let ids: Vec<Option<u64>> = lines
                .iter()
                .map(|line| {
                    let v: Value = serde_json::from_str(line).unwrap_or_else(|e| {
                        panic!("RUST_LOG={level} put a non-JSON line on stdout: {line:?}: {e}")
                    });
                    assert_eq!(v["jsonrpc"], json!("2.0"), "RUST_LOG={level}: {line}");
                    v["id"].as_u64()
                })
                .collect();
            // Both replies arrived, so the stdout check saw a live session rather
            // than a process that said nothing at all.
            assert_eq!(ids, vec![Some(1), Some(2)], "RUST_LOG={level}: {lines:?}");
            if level == "trace" {
                assert!(
                    stderr.contains("memory-wire mcp on stdio"),
                    "the level has to be in effect for the stdout check to mean anything: {stderr:?}"
                );
            }
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}
