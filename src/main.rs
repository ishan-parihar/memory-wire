mod cli_home;
mod connect;
mod connect_codex;
mod connect_ext;
mod connect_opencode;
mod connect_plugin;
mod daemon;
mod doctor;
mod guidelines;
mod hooks;
mod http;
mod mcp;
mod mcp_http;
mod paths;
mod seed;
mod sweep;

use std::future::Future;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing_subscriber::EnvFilter;

use memory_wire::api::{
    bank_config_routes, blocking, http_error, memories_routes, parse_update_mode, ApiError,
    MemoryService,
};
use memory_wire::memory::Bank;
use memory_wire::store::{default_db_path, BankStats, SqliteStore, Store};

/// The compiled binary, which cargo builds before running any unit test.
///
/// One copy for the whole crate: `hooks`, `mcp`, `seed`, and the tests here all
/// drive the real argv rather than a harness, so the path to it is shared
/// instead of re-derived per module.
#[cfg(test)]
pub(crate) fn bin() -> PathBuf {
    let mut p = std::env::current_exe().expect("test exe");
    p.pop();
    p.pop();
    p.join("memory-wire")
}

// The route map, and the reason `--help` carries it.
//
// It used to print on a bare invocation, which is what this text replaced: an
// agent that ran `memory-wire` to find out what existed learned nothing about
// the state of anything and still had to make a second call. The map is not
// deleted, it moves here — `--help` is where a caller goes when it wants the
// manual rather than the state, and `-h` stays the concise form.
//
// Written inline in the attribute rather than through a `const` because a
// derive attribute is not a format string, and `{SURFACE}` in one of those is
// six literal characters rather than the block it looks like.
#[derive(Parser)]
#[command(
    name = "memory-wire",
    version,
    about = "memory-wire: Rust agent-memory infrastructure (Hindsight x agentmemory)",
    long_about = "memory-wire: Rust agent-memory infrastructure (Hindsight x agentmemory)\n\n\
                  Bare `memory-wire` prints live state — bank, store, server, newest memory,\n\
                  tags — not this list. `info` prints the audit and plan pointers.\n\n\
                  HTTP  POST /banks/:id/{retain,recall,reflect} · tags on retain and recall\n\
                  \x20     GET|PUT /banks/:id/config · GET .../memories[/:mid] · DELETE .../memories/:mid\n\
                  \x20     GET /banks/:id/stats · GET /health\n\
                  MCP   `memory-wire mcp` on stdio — retain / recall / reflect / bank config\n\
                  \x20     `memory-wire serve` also answers MCP at /mcp and /mcp/{bank}\n\
                  CLI   daemon start|stop|status — run the server in the background\n\
                  \x20     connect (hooks + MCP entries) · hook <lifecycle> · doctor [--strict]\n\
                  \x20     sweep [--dry-run] [--db PATH] — forget expired memories (per-bank, opt-in)\n\
                  \x20     seed [--commits N] [--transcripts] — one-shot bank seeding\n\n\
                  Audit: docs/AUDIT.md | Plan: PLAN.md"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Print the audit + plan pointers. The bare binary prints live state.
    Info,
    /// Start the HTTP server (retain/recall/reflect + health).
    Serve {
        /// Bind address. Loopback by default; there is no authentication.
        #[arg(long, default_value = paths::DEFAULT_ADDR)]
        addr: String,
        /// SQLite database path (default: $XDG_DATA_HOME/memory-wire/memory.db).
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Run the server in the background, and manage its lifetime.
    ///
    /// Additive to `serve`, which is untouched: `daemon start` re-execs this
    /// binary with `serve --addr … [--db …]`, detached into its own session, and
    /// records the child in `$XDG_DATA_HOME/memory-wire/serve.json`. It exists
    /// because the hooks are never-fail by contract, which means a stopped server
    /// is invisible to a model — `daemon status` is what tells the two apart.
    Daemon {
        /// Which lifecycle step to take.
        #[command(subcommand)]
        action: daemon::Action,
    },
    /// Wire memory-wire into agent hosts (default: every detected host).
    Connect {
        /// Hosts to wire instead of every detected one. Repeat the argument or
        /// comma-separate the names; a name repeated is one host. With no host
        /// named, every detected host is wired. `memory-wire connect --list`
        /// reports the names and which of them are on this machine.
        #[arg(value_name = "AGENT")]
        agent: Vec<String>,
        /// Remove memory-wire entries instead of adding them.
        #[arg(long)]
        uninstall: bool,
        /// Write the memory-wire rules block into this project's agent files.
        #[arg(long)]
        guidelines: bool,
        /// Report every host, whether it is on this machine and whether it is
        /// already wired, and write nothing at all.
        #[arg(long)]
        list: bool,
        /// Bake this bank into the hook commands that get written, so a host
        /// reaches the intended memories instead of the one its directory
        /// resolves to. Use it when the memories you want are in a shared bank
        /// and the per-project bank is empty — `doctor` names the bank to use.
        #[arg(long, value_name = "ID")]
        bank: Option<String>,
    },
    /// Lifecycle hook (hosts call this; reads hook JSON on stdin).
    Hook {
        #[command(subcommand)]
        lifecycle: hooks::Lifecycle,
        /// Bank to use, overriding the resolved one. Mostly for hosts that
        /// cannot pass an environment variable to the hook they launch.
        ///
        /// `global` so it is accepted on either side of the lifecycle name —
        /// `hook --bank x prompt` and `hook prompt --bank x` both work. A
        /// non-global arg on a command that only dispatches to subcommands is
        /// accepted *before* the name and rejected after it, which is a trap
        /// worth not shipping.
        #[arg(long, global = true)]
        bank: Option<String>,
    },
    /// Report endpoint, bank, store, and server health.
    ///
    /// The store is opened **read-only and is never migrated**: doctor must not
    /// mutate a store it only reports on, so a store written by an older build
    /// is reported as it is on disk. That makes it the wrong tool for verifying a
    /// migration — to check a migration, open the store with `serve`, `sweep` or
    /// any read/write path first, then point `--db` at it.
    Doctor {
        /// Inspect this SQLite database (default: $XDG_DATA_HOME/memory-wire/memory.db).
        /// Point it at the same file `serve --db X` uses, or the report describes
        /// a store the server never touches.
        #[arg(long)]
        db: Option<PathBuf>,
        /// Exit nonzero when the server or the store is unusable.
        #[arg(long)]
        strict: bool,
    },
    /// Serve MCP over stdio: memory_retain / recall / reflect / bank_config_get.
    Mcp {
        /// Default bank for every call (default: $MEMORY_WIRE_BANK, else `memory-wire`).
        #[arg(long)]
        bank: Option<String>,
        /// SQLite database path (default: $XDG_DATA_HOME/memory-wire/memory.db).
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Delete memories older than their bank's `ttl_days` (banks with none are skipped).
    Sweep {
        /// Report what would be deleted, and delete nothing.
        #[arg(long)]
        dry_run: bool,
        /// SQLite database path (default: $XDG_DATA_HOME/memory-wire/memory.db).
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Seed a bank once from this repo's git history (and, opt-in, transcripts).
    Seed {
        /// Commits to read (default: 100, capped at 500).
        #[arg(long)]
        commits: Option<usize>,
        /// Also seed from ~/.claude/projects transcripts (the 50 newest files).
        #[arg(long)]
        transcripts: bool,
        /// Target bank (default: this work tree's top-level directory name).
        #[arg(long)]
        bank: Option<String>,
        /// SQLite database path (default: $XDG_DATA_HOME/memory-wire/memory.db).
        #[arg(long)]
        db: Option<PathBuf>,
    },
    /// Embed text with the bundled 384-d model and print the vector as JSON.
    ///
    /// Only compiled with `--features embed`, which is also the only build that
    /// carries the 23 MB of weights — a build without it has nothing to print and
    /// no subcommand to print it with. Offline: no download, no cache, no file on
    /// disk. This is the Phase D arm's visible surface
    /// (`docs/EXCEED_PLAN.md` §2); it does not change any ranking, because
    /// `FusionWeights::vector` still ships at `0.0`.
    #[cfg(feature = "embed")]
    Embed {
        /// The text to embed.
        text: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        // stderr, never stdout: `mcp` speaks JSON-RPC on stdout, and one log
        // line there is a framing error the client reports as a broken server.
        .with_writer(std::io::stderr)
        .init();
    // `try_parse` rather than `parse`, so a rejected argument can be answered
    // with the subcommand's own concise help instead of exiting after one line
    // of error. The `--help` and `--version` paths land in the same place and
    // still exit 0, and nothing here runs unless the parse already failed —
    // `--version` is unchanged in what it does.
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => cli_home::usage_failure(&e),
    };
    // No subcommand is not "no information": AXI wants the most relevant live
    // content here, so this is the home view and not a route map. `info`
    // remains the explicit ask for the document pointers.
    let Some(cmd) = cli.cmd else {
        cli_home::print_home();
        return Ok(());
    };
    match cmd {
        Cmd::Info => {
            println!("memory-wire v{}", env!("CARGO_PKG_VERSION"));
            println!("Audit: docs/AUDIT.md | Plan: PLAN.md");
            println!("Run `memory-wire` for live state, or `memory-wire --help` for the surface.");
        }
        Cmd::Serve { addr, db } => {
            serve(&addr, db).await?;
        }
        Cmd::Daemon { action } => {
            std::process::exit(daemon::run(action));
        }
        Cmd::Connect { agent, uninstall, guidelines, list, bank } => {
            // A name that is not a host is a usage error, not a wiring failure,
            // so it goes out the same way a bad flag does: the reason and this
            // subcommand's own help together, for a caller to correct in one turn.
            let hosts = match connect::parse_hosts(&agent) {
                Ok(hosts) => hosts,
                Err(why) => cli_home::usage_failure(&clap::Error::raw(
                    clap::error::ErrorKind::InvalidValue,
                    why,
                )),
            };
            if let Some(why) = connect_conflict(hosts.len(), uninstall, guidelines, list) {
                eprintln!("memory-wire: {why}");
                std::process::exit(1);
            }
            if list {
                // Before anything below has opened a file: a listing cannot fail,
                // so there is no exit code to carry out of it.
                list_hosts();
                return Ok(());
            }
            let failed = if guidelines {
                write_guidelines()
            } else {
                wire(hosts, uninstall, bank.as_deref())
            };
            if failed {
                std::process::exit(1);
            }
        }
        Cmd::Hook { lifecycle, bank } => {
            std::process::exit(hooks::run(lifecycle, bank.as_deref()));
        }
        Cmd::Doctor { db, strict } => {
            // No `--db`: `doctor::run` is the original path, untouched. With one,
            // the same report is collected from that file instead of the default.
            let code = match db {
                Some(path) => doctor_at(&path, strict),
                None => doctor::run(strict),
            };
            std::process::exit(code);
        }
        Cmd::Mcp { bank, db } => {
            // `--bank` wins; `default_bank` covers the env var and the fallback.
            mcp::serve(db, bank.unwrap_or_else(mcp::default_bank)).await?;
        }
        Cmd::Sweep { dry_run, db } => {
            // The same opener and default path `serve` uses, so a sweep reads the
            // store the server is actually serving.
            let store = open_store(&db.unwrap_or_else(default_db_path))?;
            println!("{}", sweep::run(&store, dry_run)?.render(dry_run));
        }
        Cmd::Seed { commits, transcripts, bank, db } => {
            // The same opener and default path `serve` uses, so a seeded bank
            // lands in the store the server already serves. `--bank` wins;
            // `resolve_bank` is the work-tree basename, else `memory-wire`.
            let store = open_store(&db.unwrap_or_else(default_db_path))?;
            let bank = bank.unwrap_or_else(paths::resolve_bank);
            println!(
                "{}",
                seed::run(&MemoryService::new(store), &bank, commits, transcripts).render(&bank)
            );
        }
        // Phase D's visible surface. Present only in a build that carries the
        // model, which is the whole point: the weights are reachable from the
        // shipped executable rather than linked into a library the binary's LTO
        // pass would strip.
        #[cfg(feature = "embed")]
        Cmd::Embed { text } => {
            let mut embedder = memory_wire::vector::Embedder::new()
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let vector = embedder.embed(&text).map_err(|e| anyhow::anyhow!("{e}"))?;
            // A bare JSON array of 384 floats, so it pipes straight into
            // whatever compares two of them.
            println!("{}", serde_json::to_string(&vector)?);
        }
    }
    Ok(())
}

/// Refuse a flag combination that would half-run.
///
/// `--uninstall` and `--guidelines` write to disjoint places and disagree about
/// what "installed" means, so honouring both would report a teardown while
/// writing a rules block (or the reverse) — a state neither can be undone from.
/// `--list` writes nothing, so pairing it with either would answer a scan where a
/// change was asked for.
fn connect_conflict(
    hosts: usize,
    uninstall: bool,
    guidelines: bool,
    list: bool,
) -> Option<&'static str> {
    if uninstall && guidelines {
        return Some("cannot combine --uninstall with --guidelines");
    }
    if list && (uninstall || guidelines) {
        return Some("--list wires nothing; drop it to make a change");
    }
    if list && hosts > 0 {
        return Some("--list takes no host; `memory-wire connect <host>` wires one");
    }
    None
}

/// Print one line per host — is it here, is it already wired — and touch nothing.
fn list_hosts() {
    let home = match paths::home() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("memory-wire: {e}");
            return;
        }
    };
    for host in connect::ALL {
        println!("{}", connect::listing(*host, &home, connect::binary_on_path));
    }
}

/// Wire (or unwire) the selected hosts; returns true when any host was refused.
fn wire(hosts: Vec<connect::Host>, uninstall: bool, bank: Option<&str>) -> bool {
    let home = match paths::home() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("memory-wire: {e}");
            return true;
        }
    };
    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "memory-wire".to_string());
    // Only a bare `connect` is `IMPLICIT` — and not `connect::ALL`: see the note
    // below. A caller who names every host has still chosen, and is not told
    // about the one they left out.
    let implicit = hosts.is_empty();
    let selected = if implicit { connect::IMPLICIT.to_vec() } else { hosts };
    let mut failed = false;
    for host in selected {
        let outcome = connect::run(host, &exe, &home, uninstall, bank);
        failed |= outcome.is_failure();
        println!("{}", outcome.render(host));
    }
    if implicit {
        // Built from `ALL - IMPLICIT` rather than written out, so a host added to
        // one list and not the other cannot leave a stale name in this sentence.
        let skipped: Vec<&str> = connect::ALL
            .iter()
            .filter(|h| !connect::IMPLICIT.contains(h))
            .map(|h| h.id())
            .collect();
        if !skipped.is_empty() {
            println!(
                "note       {} not wired by a bare `connect` — it takes over the single\n            \
                 memory.provider slot. Run `memory-wire connect {}` to switch to us.",
                skipped.join(", "),
                skipped.join("` / `connect ")
            );
        }
    }
    failed
}

/// Write the managed rules block into this project's agent files.
fn write_guidelines() -> bool {
    let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let block = guidelines::block(&paths::endpoint(), &paths::resolve_bank_in(&base));
    let mut failed = false;
    for (line, applied) in guidelines::apply_all(&base, &block) {
        failed |= matches!(applied, guidelines::Applied::Refused(_));
        println!("{line}");
    }
    failed
}

/// `doctor` against an explicit store path.
///
/// The path is threaded in from the CLI rather than read from the environment
/// inside `doctor`, so `doctor --db X` reports on the same file `serve --db X`
/// opens. `doctor::collect_at` is already the injectable entry point, so this
/// only re-applies the print + exit-code shell around it.
fn doctor_at(store: &std::path::Path, strict: bool) -> i32 {
    let report = doctor::collect_at(&paths::endpoint(), &paths::resolve_bank(), store);
    println!("{}", report.render());
    match (strict, report.strict_failure()) {
        (true, Some(why)) => {
            eprintln!("memory-wire doctor: {why}");
            1
        }
        _ => 0,
    }
}

/// Shared service handle.
type Svc = Arc<MemoryService<SqliteStore>>;

/// Retain request body.
#[derive(Deserialize)]
struct RetainReq {
    /// Content to retain (redacted server-side).
    content: String,
    /// Optional capture context.
    context: Option<String>,
    /// Tags stored with this memory (unioned with the bank's `retainTags`).
    #[serde(default)]
    tags: Option<Vec<String>>,
    /// Re-write this document instead of adding another memory.
    document_id: Option<String>,
    /// `replace` (default) gives the document one current revision, discarding
    /// the previous one. `append` only *adds* a memory: it never extends a row,
    /// and appending under a `document_id` that already has a row in this bank is
    /// refused with `409` — use `replace` for repeated writes, or a distinct
    /// `document_id` per memory.
    update_mode: Option<String>,
}

/// Retain response body.
#[derive(Serialize)]
struct RetainRes {
    /// New memory id.
    id: String,
}

/// Recall request body.
#[derive(Deserialize)]
struct RecallReq {
    /// Query text.
    query: String,
    /// Token budget; `None` falls back to the bank's `recallMaxTokens`, then 2000.
    budget: Option<usize>,
    /// Only memories carrying any of these tags.
    #[serde(default)]
    tags: Option<Vec<String>>,
    /// `full` returns `{id, score, content}` objects; anything else (the
    /// default) keeps the bare array of content strings.
    format: Option<String>,
}

/// Delete response body.
#[derive(Serialize)]
struct DeleteRes {
    /// False when the memory was already gone.
    deleted: bool,
}

/// Start the axum server (retain/recall/reflect + bank config + lifecycle + health).
///
/// Serves until SIGINT or SIGTERM, then stops accepting and lets the requests
/// already in flight finish. A retain is one `BEGIN`/`COMMIT` and a recall is
/// bounded, so a drain is short — but cutting a connection mid-request is how a
/// stop-the-world `SIGKILL` half-writes one, and the signal this waits for is
/// the one a supervisor or a Ctrl-C actually sends.
async fn serve(addr: &str, db: Option<PathBuf>) -> anyhow::Result<()> {
    // stderr, once, before the bind: a warning nobody sees until after the port
    // is already answering is not a warning.
    if let Some(warning) = exposure_warning(addr) {
        eprintln!("memory-wire: {warning}");
    }
    let store = open_store(&db.unwrap_or_else(default_db_path))?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("memory-wire serving on {addr}");
    serve_until(
        listener,
        router(Arc::new(MemoryService::new(store))),
        shutdown_signal(),
    )
    .await
}

/// Serve `app` on an already-bound listener until `shutdown` resolves, then let
/// in-flight requests drain before returning.
///
/// The listener is a parameter and the shutdown is a future so a test can drive
/// both ends itself: a real signal to a real process is a test that hangs on
/// timing rather than on behaviour.
async fn serve_until<F>(
    listener: tokio::net::TcpListener,
    app: Router,
    shutdown: F,
) -> anyhow::Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}

/// Resolve on the first SIGINT or SIGTERM.
///
/// A later signal has nobody listening, so it changes nothing: this is not a
/// force-quit, and the drain is what decides when the process ends. That is
/// bounded — every handler is one SQLite transaction or one bounded recall, and
/// the surface has no long-poll — so waiting is short, and `SIGKILL` remains
/// available to an operator who wants the alternative.
///
/// `ctrl_c` covers SIGINT on every platform, and the term handler is the one
/// thing that has to be named, so it is unix-gated the same way the rest of the
/// crate's platform assumptions are.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let interrupted = async {
            tokio::signal::ctrl_c().await.expect("install SIGINT handler");
        };
        let terminated = async {
            signal(SignalKind::terminate())
                .expect("install SIGTERM handler")
                .recv()
                .await;
        };
        tokio::select! {
            _ = interrupted => tracing::info!("memory-wire: interrupted; draining in-flight requests"),
            _ = terminated => tracing::info!("memory-wire: terminated; draining in-flight requests"),
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await.expect("install SIGINT handler");
    }
}

/// What serving on `addr` exposes to anyone who can reach it, or `None` when the
/// address is provably loopback.
///
/// The bind default is loopback and this build has no authentication, so a
/// non-loopback bind hands the whole store — every bank's memories, readable and
/// deletable — to the network. That is a legitimate choice on a host you control
/// and an accident on one you do not, so it is named once, on stderr, and never
/// refused: refusing would break the deployment an operator asked for, and
/// staying silent is what turns it into a surprise.
///
/// `/mcp` is named in the text because it is the route a user is likelier to
/// expose on purpose — an MCP endpoint exists to be pointed at by another tool —
/// and a warning that only named the REST surface would understate what a bind
/// actually hands over. There is still no authentication on either.
///
/// An address that does not parse as a socket address (`localhost:8899`, a
/// hostname, a Unix path) is unproven rather than proven-open, and is reported on
/// the same reasoning: warn unless loopback can be shown.
fn exposure_warning(addr: &str) -> Option<String> {
    if addr.parse::<SocketAddr>().is_ok_and(|a| a.ip().is_loopback()) {
        return None;
    }
    Some(format!(
        "serving on {addr} is reachable from outside this machine and memory-wire has no \
         authentication: anything that can reach the port can read and delete these banks, and \
         can drive the same four tools over /mcp"
    ))
}

/// Open (creating parents) the file-backed store and seed the `default` bank.
fn open_store(path: &std::path::Path) -> anyhow::Result<SqliteStore> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let store = SqliteStore::open(path)?;
    tracing::info!("memory-wire db: {}", path.display());
    store.put_bank(&Bank {
        id: "default".to_string(),
        name: "default".to_string(),
    })?;
    Ok(store)
}

/// The whole HTTP surface: health, the three operations, bank config, the
/// memory lifecycle, and MCP on `/mcp` and `/mcp/<bank>`.
fn router(svc: Svc) -> Router {
    // Same service, same store, a second transport. Built before `with_state`
    // so both halves are state-free and merge as peers.
    let mcp = mcp_http::routes(svc.clone());
    Router::new()
        .route("/health", get(health))
        .route("/banks/:id/retain", post(retain))
        .route("/banks/:id/recall", post(recall))
        .route("/banks/:id/reflect", post(reflect))
        // The lifecycle half: read a bank's memories back, take one away, and
        // ask what is in the bank at all. `recall` is for relevance, these are
        // for custody — an agent that stored a secret needs to find and delete it.
        .merge(memories_routes::<SqliteStore>())
        .route("/banks/:id/memories/:mid", axum::routing::delete(delete_memory))
        .route("/banks/:id/stats", get(bank_stats))
        // GET/PUT /banks/:id/config — recallMaxTokens, retainTags, and the hooks'
        // `GET .../config` preamble probe. The handlers live next to the service
        // so their validation cannot drift from it.
        .merge(bank_config_routes::<SqliteStore>())
        // MCP over the same port. `/mcp` resolves the bank from the call, and
        // `/mcp/<bank>` pins it to the path.
        .merge(mcp)
        .with_state(svc)
}

/// Health probe.
async fn health() -> &'static str {
    "ok"
}

/// Retain handler.
async fn retain(
    State(svc): State<Svc>,
    Path(bank): Path<String>,
    Json(req): Json<RetainReq>,
) -> Result<Json<RetainRes>, HttpErr> {
    let mode = parse_update_mode(req.update_mode.as_deref()).map_err(to_http)?;
    // The service declares the bank itself, so declaring it here as well was a
    // second write per retain for nothing.
    let id = blocking(move || {
        svc.retain_doc(
            &bank,
            &req.content,
            req.context,
            &req.tags.unwrap_or_default(),
            req.document_id.as_deref(),
            mode,
        )
    })
    .await
    .map_err(to_http)?;
    Ok(Json(RetainRes { id }))
}

/// Recall handler.
async fn recall(
    State(svc): State<Svc>,
    Path(bank): Path<String>,
    Json(req): Json<RecallReq>,
) -> Result<Json<Value>, HttpErr> {
    // `budget` stays an Option all the way down: `recall_filtered` resolves it
    // against the bank's `recallMaxTokens` before the 2000 default, and
    // unwrapping here would silently ignore that config.
    let hits = blocking(move || {
        svc.recall_filtered(
            &bank,
            &req.query,
            req.budget,
            &req.tags.unwrap_or_default(),
        )
    })
    .await
    .map_err(to_http)?;
    // The default shape is a bare array of strings and stays that way; `full` is
    // for a caller that has to cite what it got back.
    Ok(Json(if req.format.as_deref() == Some("full") {
        Value::Array(
            hits.iter()
                .map(|h| json!({ "id": h.memory.id, "score": h.score, "content": h.memory.content }))
                .collect(),
        )
    } else {
        Value::Array(hits.into_iter().map(|h| Value::String(h.memory.content)).collect())
    }))
}

/// Reflect handler.
async fn reflect(
    State(svc): State<Svc>,
    Path(bank): Path<String>,
    Json(req): Json<RecallReq>,
) -> Result<Json<String>, HttpErr> {
    let answer = blocking(move || svc.reflect(&bank, &req.query, &req.tags.unwrap_or_default()))
        .await
        .map_err(to_http)?;
    Ok(Json(answer))
}

/// `DELETE /banks/:id/memories/:mid` — 200 with `deleted: false` when already gone.
async fn delete_memory(
    State(svc): State<Svc>,
    Path((bank, mid)): Path<(String, String)>,
) -> Result<Json<DeleteRes>, HttpErr> {
    let deleted = blocking(move || svc.delete_memory(&bank, &mid))
        .await
        .map_err(to_http)?;
    Ok(Json(DeleteRes { deleted }))
}

/// `GET /banks/:id/stats` — how much is in the bank, and over what span.
async fn bank_stats(
    State(svc): State<Svc>,
    Path(bank): Path<String>,
) -> Result<Json<BankStats>, HttpErr> {
    Ok(Json(blocking(move || svc.bank_stats(&bank)).await.map_err(to_http)?))
}

/// Client-facing error: an opaque status + message pair.
type HttpErr = (StatusCode, String);

/// Map a service error to a client-safe response.
///
/// Client input errors (blank bank id) surface as 400; every other failure —
/// including driver text that would echo SQL fragments and on-disk paths —
/// becomes a generic 500 `storage error`.
fn to_http(e: ApiError) -> HttpErr {
    let (status, msg) = http_error(&e);
    (status, msg.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http;
    use memory_wire::memory::Memory;
    use std::time::Duration;

    /// The real router on an ephemeral port, over a throwaway store file.
    ///
    /// Driven with the project's own minimal HTTP client rather than a router
    /// harness: that keeps the assertions on the actual wire contract (status
    /// code + JSON body) and needs no dev-dependency.
    fn serve_scratch(name: &str) -> (String, PathBuf) {
        // The name is per-test: these run in parallel threads of one process,
        // and a shared directory would have one test's `remove_dir_all` pull the
        // store out from under another's open handle.
        let dir = std::env::temp_dir().join(format!("mw-http-{}-{name}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let db = dir.join("memory.db");
        let store = open_store(&db).expect("store");
        let app = router(Arc::new(MemoryService::new(store)));
        let (tx, rx) = std::sync::mpsc::channel();
        // A current-thread runtime on its own thread, serving for the life of
        // the test process: the test body runs on the main thread, so the two
        // never contend, and the port is published as soon as it is bound.
        std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("rt")
                .block_on(async {
                    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                        .await
                        .expect("bind");
                    tx.send(listener.local_addr().expect("addr")).ok();
                    let _ = axum::serve(listener, app).await;
                });
        });
        let addr = rx.recv().expect("server never bound");
        (format!("http://{addr}"), db)
    }

    fn post(base: &str, path: &str, body: &str) -> http::Response {
        let r = http::post_json(
            &format!("{base}{path}"),
            body,
            Duration::from_secs(10),
        )
        .expect("request");
        assert!(r.ok(), "POST {path} -> {} {}", r.status, r.body);
        r
    }

    fn get(base: &str, path: &str) -> http::Response {
        let r = http::get(&format!("{base}{path}"), Duration::from_secs(10)).expect("request");
        assert!(r.ok(), "GET {path} -> {} {}", r.status, r.body);
        r
    }

    // Wave-2 leftover A: the config routes are reachable from the binary.
    //
    // The hook preamble probe is the real consumer: it issues
    // `GET /banks/:id/config` best-effort, so a 404 here means the merge
    // silently regressed and every session falls back to local framing.
    #[test]
    fn config_routes_should_be_merged_into_the_serve_router() {
        let (base, _db) = serve_scratch("config");
        post(&base, "/banks/demo/retain", r#"{"content":"auth uses jose middleware"}"#);

        // A bank that exists, with no config yet: 200 and an empty object,
        // not the 404 the hook probe used to see.
        let got = get(&base, "/banks/demo/config");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&got.body).expect("json"),
            serde_json::json!({}),
            "an unset config must read back as {{}}, not 404"
        );

        // PUT replaces the whole config; GET serves it back verbatim, including
        // the keys this build stores but does not act on.
        let cfg = r#"{"recallMaxTokens":128,"retainTags":["ops"],"retain_mission":"own the release"}"#;
        put(&base, "/banks/demo/config", cfg);
        let after: serde_json::Value =
            serde_json::from_str(&get(&base, "/banks/demo/config").body).expect("json");
        assert_eq!(after["recallMaxTokens"], serde_json::json!(128));
        assert_eq!(after["retainTags"], serde_json::json!(["ops"]));
        assert_eq!(after["retain_mission"], serde_json::json!("own the release"));

        // A bank that was never created is a 404, not an empty config — a typo
        // must not read back as "this bank has no settings".
        let ghost = http::get(&format!("{base}/banks/ghost/config"), Duration::from_secs(10))
            .expect("request");
        assert_eq!(ghost.status, 404, "{}", ghost.body);
        assert_eq!(ghost.body, "unknown bank");
    }

    #[test]
    fn tags_should_pass_through_retain_and_recall() {
        let (base, _db) = serve_scratch("tags");
        post(
            &base,
            "/banks/t/retain",
            r#"{"content":"auth uses jose middleware","tags":["Auth"]}"#,
        );
        post(&base, "/banks/t/retain", r#"{"content":"auth uses jose untagged"}"#);

        let hits: Vec<String> = serde_json::from_str(
            &post(&base, "/banks/t/recall", r#"{"query":"jose","tags":["auth"]}"#).body,
        )
        .expect("array");
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert!(hits[0].contains("middleware"), "{hits:?}");

        let all: Vec<String> =
            serde_json::from_str(&post(&base, "/banks/t/recall", r#"{"query":"jose"}"#).body)
                .expect("array");
        assert_eq!(all.len(), 2, "an unfiltered recall is unchanged: {all:?}");
    }

    // The bug the `Option<usize>` fix exists for: an absent `budget` used to be
    // unwrapped to 2000 in the handler, so a bank's `recallMaxTokens` was
    // stored, served, and then ignored on the one path that mattered.
    #[test]
    fn an_absent_budget_should_honor_the_banks_recall_max_tokens() {
        let (base, _db) = serve_scratch("budget");
        post(&base, "/banks/b/retain", &format!(r#"{{"content":"{}"}}"#, "jose ".repeat(400)));
        let cfg = r#"{"recallMaxTokens":5}"#;
        put(&base, "/banks/b/config", cfg);

        let bank_default: Vec<String> =
            serde_json::from_str(&post(&base, "/banks/b/recall", r#"{"query":"jose"}"#).body)
                .expect("array");
        let explicit: Vec<String> = serde_json::from_str(
            &post(&base, "/banks/b/recall", r#"{"query":"jose","budget":200}"#).body,
        )
        .expect("array");

        assert_eq!(bank_default[0].chars().count(), 5 * 4, "bank recallMaxTokens must apply");
        assert!(
            explicit[0].chars().count() > bank_default[0].chars().count(),
            "an explicit budget must still win"
        );
    }

    #[test]
    fn health_and_the_plain_shapes_should_be_unchanged() {
        let (base, _db) = serve_scratch("shapes");
        let h = http::get(&format!("{base}/health"), Duration::from_secs(10)).expect("health");
        assert!(h.ok());
        assert_eq!(h.body, "ok");

        let id: serde_json::Value = serde_json::from_str(
            &post(&base, "/banks/s/retain", r#"{"content":"plain"}"#).body,
        )
        .expect("json");
        assert!(id["id"].as_str().is_some(), "{id}");

        let r: Vec<String> = serde_json::from_str(
            &post(&base, "/banks/s/recall", r#"{"query":"plain","budget":2000}"#).body,
        )
        .expect("array");
        assert_eq!(r.len(), 1, "{r:?}");
    }

    /// PUT and DELETE need their verbs, and the minimal client is GET/POST only,
    /// so those requests speak HTTP/1.1 directly. `base` is a bare
    /// `http://host:port`, so the authority is the whole remainder.
    fn raw(base: &str, method: &str, path: &str, body: &str) -> (u16, String) {
        use std::io::{Read, Write};
        let host = base.trim_start_matches("http://");
        let mut sock = std::net::TcpStream::connect(host).expect("connect");
        sock.set_read_timeout(Some(Duration::from_secs(10))).expect("read timeout");
        sock.set_write_timeout(Some(Duration::from_secs(10))).expect("write timeout");
        let mut req = format!("{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n");
        if !body.is_empty() {
            req.push_str(&format!(
                "Content-Type: application/json\r\nContent-Length: {}\r\n",
                body.len()
            ));
        }
        req.push_str(&format!("\r\n{body}"));
        sock.write_all(req.as_bytes()).expect("write");
        sock.flush().expect("flush");
        let mut raw = Vec::new();
        sock.read_to_end(&mut raw).expect("read");
        let text = String::from_utf8_lossy(&raw).into_owned();
        let status: u16 = text
            .split_whitespace()
            .nth(1)
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        (status, text)
    }

    fn put(base: &str, path: &str, body: &str) -> String {
        let (status, text) = raw(base, "PUT", path, body);
        assert!((200..300).contains(&status), "PUT {path} -> {status}: {text}");
        text
    }

    /// Status plus the body as sent, so a test can assert on both. `body` is
    /// sent only when non-empty, so the same helper serves GET and PUT.
    fn body_of(base: &str, method: &str, path: &str, body: &str) -> (u16, String) {
        let (status, text) = raw(base, method, path, body);
        let body = text.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("").to_string();
        (status, body)
    }

    // The lifecycle half of the surface: what an agent stored can be listed,
    // read back, counted, and deleted.
    #[test]
    fn the_memory_lifecycle_routes_should_list_read_delete_and_count() {
        let (base, _db) = serve_scratch("life");
        let with_ctx: Value = serde_json::from_str(
            &post(
                &base,
                "/banks/l/retain",
                r#"{"content":"auth uses jose","context":"hook:stop"}"#,
            )
            .body,
        )
        .expect("json");
        let id = with_ctx["id"].as_str().expect("id").to_string();
        post(&base, "/banks/l/retain", r#"{"content":"rate limiting via token bucket"}"#);

        // List: both memories, `created_at` always, `context` only when present.
        let list: Vec<Value> =
            serde_json::from_str(&get(&base, "/banks/l/memories").body).expect("array");
        assert_eq!(list.len(), 2, "{list:?}");
        for m in &list {
            assert!(m["id"].as_str().is_some(), "{m}");
            assert!(m["content"].as_str().is_some(), "{m}");
            assert!(
                m["created_at"].as_str().is_some_and(|t| t.len() >= 10),
                "created_at must always be present: {m}"
            );
        }
        let with = list.iter().find(|m| m["id"] == json!(id)).expect("our memory");
        assert_eq!(with["context"], json!("hook:stop"));
        let without = list
            .iter()
            .find(|m| m["context"].is_null())
            .expect("a memory stored without context");
        assert!(
            without.get("context").is_none(),
            "an absent context must be omitted, not null: {without}"
        );

        // Paging: limit and offset select, and the cap holds.
        let one: Vec<Value> =
            serde_json::from_str(&get(&base, "/banks/l/memories?limit=1").body).expect("array");
        assert_eq!(one.len(), 1, "{one:?}");
        let next: Vec<Value> = serde_json::from_str(
            &get(&base, "/banks/l/memories?limit=1&offset=1").body,
        )
        .expect("array");
        assert_eq!(next.len(), 1, "{next:?}");
        assert_ne!(one[0]["id"], next[0]["id"], "offset must advance the page");
        let capped: Vec<Value> =
            serde_json::from_str(&get(&base, "/banks/l/memories?limit=99999").body).expect("array");
        assert_eq!(capped.len(), 2, "an over-large limit is capped, not refused");

        // Read one back.
        let got = get(&base, &format!("/banks/l/memories/{id}"));
        let one: Value = serde_json::from_str(&got.body).expect("json");
        assert_eq!(one["id"], json!(id));
        assert_eq!(one["content"].as_str(), Some("auth uses jose"));
        let (status, body) = body_of(&base, "GET", "/banks/l/memories/does-not-exist", "");
        assert_eq!(status, 404, "{body}");
        assert_eq!(body, "unknown memory");

        // Count the bank.
        let stats: Value = serde_json::from_str(&get(&base, "/banks/l/stats").body).expect("json");
        assert_eq!(stats["memories"], json!(2), "{stats}");
        assert!(stats["tags"].is_number(), "{stats}");
        assert!(stats["oldest"].as_str().is_some(), "{stats}");
        assert!(stats["newest"].as_str().is_some(), "{stats}");

        // Delete: true once, false after, and gone from the reads either way.
        let (status, body) = body_of(&base, "DELETE", &format!("/banks/l/memories/{id}"), "");
        assert_eq!(status, 200, "{body}");
        let gone: Value = serde_json::from_str(&body).expect("json");
        assert_eq!(gone["deleted"], json!(true), "{gone}");
        let (status, body) = body_of(&base, "DELETE", &format!("/banks/l/memories/{id}"), "");
        assert_eq!(status, 200, "{body}");
        assert_eq!(
            serde_json::from_str::<Value>(&body).expect("json")["deleted"],
            json!(false),
            "{body}"
        );
        assert_eq!(body_of(&base, "GET", &format!("/banks/l/memories/{id}"), "").0, 404);
        let after: Vec<Value> =
            serde_json::from_str(&get(&base, "/banks/l/memories").body).expect("array");
        assert_eq!(after.len(), 1, "{after:?}");
        let stats: Value = serde_json::from_str(&get(&base, "/banks/l/stats").body).expect("json");
        assert_eq!(stats["memories"], json!(1), "{stats}");
    }

    // The reflect route scopes its answer to the request's tags, exactly as
    // recall does — a tag filter that works on one and not the other would make
    // a filtered reflect cite something the caller excluded.
    #[test]
    fn reflect_should_honor_the_requests_tags() {
        let (base, _db) = serve_scratch("reflecttags");
        post(
            &base,
            "/banks/r/retain",
            r#"{"content":"auth uses jose middleware","tags":["auth"]}"#,
        );
        post(&base, "/banks/r/retain", r#"{"content":"deployment runs on fridays"}"#);

        let hit: String = serde_json::from_str(
            &post(
                &base,
                "/banks/r/reflect",
                r#"{"query":"how does auth work","tags":["auth"]}"#,
            )
            .body,
        )
        .expect("json");
        assert!(hit.contains("jose"), "{hit}");
        assert!(hit.starts_with("based on ["), "{hit}");

        let miss: String = serde_json::from_str(
            &post(
                &base,
                "/banks/r/reflect",
                r#"{"query":"how does auth work","tags":["secrets"]}"#,
            )
            .body,
        )
        .expect("json");
        assert_eq!(miss, "no relevant memories", "a tag no memory carries finds nothing");
    }

    // `format: "full"` is the citing shape; without it recall is unchanged.
    #[test]
    fn recall_should_serve_the_full_shape_only_when_asked() {
        let (base, _db) = serve_scratch("format");
        let kept: Value = serde_json::from_str(
            &post(
                &base,
                "/banks/f/retain",
                r#"{"content":"auth uses jose middleware","tags":["Auth"]}"#,
            )
            .body,
        )
        .expect("json");
        let id = kept["id"].as_str().expect("id");

        // The default is untouched: a bare array of content strings.
        let plain: Vec<String> =
            serde_json::from_str(&post(&base, "/banks/f/recall", r#"{"query":"jose"}"#).body)
                .expect("array of strings");
        assert_eq!(plain.len(), 1, "{plain:?}");

        let full: Vec<Value> = serde_json::from_str(
            &post(
                &base,
                "/banks/f/recall",
                r#"{"query":"jose","format":"full"}"#,
            )
            .body,
        )
        .expect("array of objects");
        assert_eq!(full.len(), 1, "{full:?}");
        assert_eq!(full[0]["id"], json!(id), "{full:?}");
        assert_eq!(full[0]["content"].as_str(), Some("auth uses jose middleware"));
        // The score served is the fused RRF one, not a token count: the single
        // hit ranks 1st in both streams, so it carries each stream's weight over
        // (k + 1). Built from the shipped weights rather than written out, so
        // this keeps testing that the served value is the fused one even after
        // Phase E1 moved the overlap weight off 1.0.
        let w = memory_wire::recall::FusionWeights::SHIPPED;
        let expected = w.bm25 / (w.k + 1.0) + w.overlap / (w.k + 1.0);
        assert_eq!(
            full[0]["score"].as_f64(),
            Some(expected),
            "full must carry the fused RRF score: {full:?}"
        );

        // An unknown format is the default, not an error.
        let other: Vec<String> = serde_json::from_str(
            &post(
                &base,
                "/banks/f/recall",
                r#"{"query":"jose","format":"verbose"}"#,
            )
            .body,
        )
        .expect("array of strings");
        assert_eq!(other, plain, "an unrecognised format must not change the shape");
    }

    // A misspelled `update_mode` is a 400, and the document keeps the revision
    // it had.
    //
    // It used to be a destructive replace: the handler unwrapped the field with
    // `unwrap_or("replace")` and the store branched on "is it exactly append",
    // so `"append "`, `"APPEND"`, or any typo took the other path and deleted
    // the document's prior row. A refused request that still deletes a revision
    // is worse than the typo it came from, so both halves are asserted here:
    // the status, and the row that survived it.
    #[test]
    fn a_misspelled_update_mode_should_be_a_400_and_keep_the_prior_revision() {
        let (base, _db) = serve_scratch("update-mode");
        post(
            &base,
            "/banks/u/retain",
            r#"{"content":"jose v1","document_id":"handbook"}"#,
        );
        let before: Vec<Value> =
            serde_json::from_str(&get(&base, "/banks/u/memories").body).expect("array");
        assert_eq!(before.len(), 1, "one revision to lose: {before:?}");

        for typo in ["append ", "APPEND", "upsert", ""] {
            let body = serde_json::json!({
                "content": "jose v2",
                "document_id": "handbook",
                "update_mode": typo,
            })
            .to_string();
            let (status, got) = body_of(&base, "POST", "/banks/u/retain", &body);
            assert_eq!(status, 400, "{typo:?} must be refused, got {got}");
            assert_eq!(got, "invalid content", "the frozen 400, unchanged: {got}");
        }

        // The refused writes changed nothing at all: the revision is still there,
        // byte-unchanged, and no second row was added.
        let after: Vec<Value> =
            serde_json::from_str(&get(&base, "/banks/u/memories").body).expect("array");
        assert_eq!(after.len(), 1, "a refused retain must not supersede: {after:?}");
        assert_eq!(after[0]["content"], json!("jose v1"));

        // The two documented values still work. `append` gets a fresh document id
        // because appending under one that already holds a row is a 409 by
        // design — that is a separate contract, asserted elsewhere.
        post(
            &base,
            "/banks/u/retain",
            r#"{"content":"jose appended","document_id":"notes","update_mode":"append"}"#,
        );
        let listed: Vec<Value> =
            serde_json::from_str(&get(&base, "/banks/u/memories").body).expect("array");
        assert_eq!(listed.len(), 2, "append still adds: {listed:?}");

        // …and the default is still the one that supersedes.
        post(
            &base,
            "/banks/u/retain",
            r#"{"content":"jose v3","document_id":"handbook"}"#,
        );
        let replaced: Vec<Value> =
            serde_json::from_str(&get(&base, "/banks/u/memories").body).expect("array");
        assert_eq!(replaced.len(), 2, "an absent mode replaces: {replaced:?}");
        let handbook = replaced
            .iter()
            .find(|m| m["content"] == json!("jose v3"))
            .expect("the default replaced the revision");
        assert_eq!(handbook["content"], json!("jose v3"));
    }

    // The storage-failure path records itself.
    //
    // Every 500 used to be silent server-side: the mapping dropped the source
    // and nothing logged, so disk-full, a corrupt page, a locked database and a
    // poisoned lock all produced one indistinguishable `500 storage error` with
    // no record anywhere. `http_error` is the one place both surfaces classify
    // through, so one `tracing::error!` there covers HTTP and MCP at once.
    #[test]
    fn the_storage_failure_path_should_have_exactly_one_error_log_site() {
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut sites: Vec<(String, usize, String)> = Vec::new();
        for entry in std::fs::read_dir(&src).expect("read src") {
            let path = entry.expect("src entry").path();
            if !path.extension().is_some_and(|e| e == "rs") {
                continue;
            }
            let file = path.file_name().expect("file name").to_string_lossy().into_owned();
            for (n, line) in std::fs::read_to_string(&path).expect("read source").lines().enumerate() {
                if line.trim_start().starts_with("tracing::error!") {
                    sites.push((file.clone(), n + 1, line.trim().to_string()));
                }
            }
        }
        assert_eq!(
            sites.len(),
            1,
            "two sites means one of the surfaces logs and the other does not: {sites:?}"
        );
        assert_eq!(sites[0].0, "api.rs", "both HTTP and MCP classify in api.rs");
        // It lives on the 500 arm, not on a 4xx: a client mistake is the
        // caller's problem and must not fill the server's error log.
        let api = std::fs::read_to_string(src.join("api.rs")).expect("read api.rs");
        let arm = api
            .split("_ => {")
            .nth(1)
            .and_then(|rest| rest.split("}").next())
            .unwrap_or_default();
        assert!(arm.contains("tracing::error!"), "the log must be on the 500 arm: {arm}");
    }

    // The memory-list paging bounds (default 50, cap 500) moved to
    // `api::memories_routes`, which is where the handler now lives; its tests
    // moved with it. Asserted here instead, against the route itself, so the
    // default the wire actually serves stays pinned in this file too.
    #[test]
    fn memory_list_should_default_to_fifty_and_cap_at_five_hundred() {
        let (base, _db) = serve_scratch("page-bounds");
        let first = get(&base, "/banks/pb/memories");
        assert_eq!(first.status, 200, "{}", first.body);
        let arr: Vec<Value> = serde_json::from_str(&first.body).expect("array body");
        assert!(arr.len() <= memory_wire::api::MAX_PAGE, "{} rows", arr.len());
        let capped = get(&base, "/banks/pb/memories?limit=10000");
        assert_eq!(capped.status, 200, "{}", capped.body);
        let capped: Vec<Value> = serde_json::from_str(&capped.body).expect("array body");
        assert!(capped.len() <= memory_wire::api::MAX_PAGE, "{} rows", capped.len());
        assert_eq!(capped.len(), arr.len(), "an empty bank is empty either way");
        let default_body: Vec<Value> = serde_json::from_str(&first.body).expect("array body");
        assert!(default_body.is_empty(), "a fresh bank lists nothing");
        // The compatibility property that matters: the default body is still a
        // bare JSON array, not an envelope. Totals ride a header, so a client
        // written against the old shape keeps working byte for byte.
        assert!(
            first.body.trim_start().starts_with('['),
            "default body is an array, got: {}",
            &first.body[..first.body.len().min(40)]
        );
    }

    /// One row of the unknown-bank matrix: operation, method, path with `{b}`,
    /// body, expected status for an unknown bank, expected status for an
    /// existing-but-empty bank, and the body fragment each of those two must
    /// carry. `{b}` is the bank under test, and the two fragments differ exactly
    /// where the two statuses do.
    type BankOpCase = (
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        u16,
        u16,
        [&'static str; 2],
    );

    // The unknown-bank contract, pinned for every bank-scoped operation.
    //
    // The rule: an operation that *names one resource* answers `404` when that
    // resource is absent, so a typo cannot read back as a plausible empty
    // answer; an operation that reads a *collection or an aggregate* answers
    // `200` with the empty one, because banks are created implicitly by their
    // first retain and a pre-retain recall is normal rather than exceptional.
    // `DELETE` names a resource but stays idempotent — `200` with
    // `{"deleted":false}` — and `retain` creates the bank it names.
    //
    // Nothing asserted this before, so one careless route refactor could have
    // changed half the surface. This changes no behavior: it is the rule the
    // routes already follow, written down.
    #[test]
    fn every_bank_scoped_operation_should_follow_the_unknown_bank_rule() {
        let (base, _db) = serve_scratch("unknown-bank");

        const MATRIX: &[BankOpCase] = &[
            // retain names no resource that has to exist — it is what makes one.
            ("retain", "POST", "/banks/{b}/retain", r#"{"content":"jose note"}"#, 200, 200,
             ["\"id\"", "\"id\""]),
            // Collections and aggregates: an absent bank *is* the empty answer.
            ("recall", "POST", "/banks/{b}/recall", r#"{"query":"jose"}"#, 200, 200, ["[]", "[]"]),
            ("reflect", "POST", "/banks/{b}/reflect", r#"{"query":"jose"}"#, 200, 200,
             ["no relevant memories", "no relevant memories"]),
            ("memories", "GET", "/banks/{b}/memories", "", 200, 200, ["[]", "[]"]),
            ("stats", "GET", "/banks/{b}/stats", "", 200, 200, ["\"memories\":0", "\"memories\":0"]),
            // A named resource: absent is 404, never a plausible empty answer.
            ("get memory", "GET", "/banks/{b}/memories/nope", "", 404, 404,
             ["unknown memory", "unknown memory"]),
            ("get config", "GET", "/banks/{b}/config", "", 404, 200, ["unknown bank", "{}"]),
            ("put config", "PUT", "/banks/{b}/config", r#"{"recallMaxTokens":64}"#, 404, 200,
             ["unknown bank", "recallMaxTokens"]),
            // Named, but idempotent by contract: 200 with the flag saying no.
            ("delete memory", "DELETE", "/banks/{b}/memories/nope", "", 200, 200,
             ["\"deleted\":false", "\"deleted\":false"]),
        ];

        // Each row gets its own pair of banks, so a row that writes (retain, put
        // config) cannot leave a row behind that makes the next row's "empty"
        // assertion false. `empty{i}` is built through the routes themselves: a
        // retain, then the delete that empties it again.
        for (i, (op, method, path, body, unknown, empty, want)) in MATRIX.iter().enumerate() {
            let exists = format!("empty{i}");
            post(&base, &format!("/banks/{exists}/retain"), r#"{"content":"temporary jose row"}"#);
            let listed: Vec<Value> =
                serde_json::from_str(&get(&base, &format!("/banks/{exists}/memories")).body)
                    .expect("array");
            let temp = listed[0]["id"].as_str().expect("id").to_string();
            assert_eq!(
                body_of(&base, "DELETE", &format!("/banks/{exists}/memories/{temp}"), "").0,
                200,
                "the fixture bank must be emptied through the route"
            );

            // `ghost{i}` is never created, so it stands for a bank that does not
            // exist; `empty{i}` exists and holds nothing.
            for (n, (bank, expected)) in
                [(format!("ghost{i}"), *unknown), (exists, *empty)].into_iter().enumerate()
            {
                let (status, got) = body_of(&base, method, &path.replace("{b}", &bank), body);
                assert_eq!(status, expected, "{method} {path} on bank {bank}: {got}");
                assert!(
                    got.contains(want[n]),
                    "{op} on bank {bank} must answer with {:?}: {got}",
                    want[n]
                );
            }
        }

        // The corollary the rule implies and the config rows above prove: a bank
        // exists once something has been retained into it, and `PUT .../config`
        // is not a way to conjure one.
        let (status, text) = raw(&base, "PUT", "/banks/ghost/config", "{}");
        assert_eq!(status, 404, "a refused PUT must not create the bank: {text}");
        assert_eq!(
            body_of(&base, "GET", "/banks/ghost/config", "").0,
            404,
            "the bank is still absent after the refused PUT"
        );
    }

    // `--uninstall` and `--guidelines` are refused together, and the refusal
    // happens before anything is written.
    #[test]
    fn uninstall_with_guidelines_should_fail_without_writing_anything() {
        assert_eq!(
            connect_conflict(0, true, true, false),
            Some("cannot combine --uninstall with --guidelines")
        );
        assert_eq!(connect_conflict(0, true, false, false), None);
        assert_eq!(connect_conflict(0, false, true, false), None);
        // `--list` reports and writes nothing, so it cannot stand in for a change.
        assert_eq!(connect_conflict(0, true, false, true), Some("--list wires nothing; drop it to make a change"));
        assert_eq!(connect_conflict(1, false, false, true), Some("--list takes no host; `memory-wire connect <host>` wires one"));
        assert_eq!(connect_conflict(0, false, false, true), None);

        let dir = std::env::temp_dir().join(format!("mw-conflict-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("tmp");
        let out = std::process::Command::new(crate::bin())
            .args(["connect", "--uninstall", "--guidelines"])
            .current_dir(&dir)
            .env("HOME", &dir)
            .env_remove("XDG_DATA_HOME")
            .output()
            .expect("run memory-wire");
        let err = String::from_utf8_lossy(&out.stderr).into_owned();
        assert_eq!(out.status.code(), Some(1), "stderr: {err}");
        assert!(err.contains("cannot combine --uninstall with --guidelines"), "{err}");
        let left: Vec<String> = std::fs::read_dir(&dir)
            .expect("readdir")
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(left.is_empty(), "a refused combination must write nothing: {left:?}");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Every file under `root` as (path relative to `root`, bytes) — a comparison
    /// that notices a file created, removed or rewritten, not only a changed one.
    fn snapshot(root: &std::path::Path) -> std::collections::BTreeMap<String, Vec<u8>> {
        let mut out = std::collections::BTreeMap::new();
        let mut queue = vec![root.to_path_buf()];
        while let Some(dir) = queue.pop() {
            for entry in std::fs::read_dir(&dir).expect("read_dir") {
                let path = entry.expect("entry").path();
                if path.is_dir() {
                    queue.push(path);
                } else {
                    let rel = path.strip_prefix(root).expect("under root").display().to_string();
                    out.insert(rel, std::fs::read(&path).expect("read"));
                }
            }
        }
        out
    }

    /// One `connect` run against a scratch `HOME` with **no** `PATH`, so host
    /// detection sees only what the test put there and cannot depend on which
    /// agents happen to be installed on the machine running the suite.
    fn connect_run(args: &[&str], home: &std::path::Path) -> (String, String, i32) {
        let out = std::process::Command::new(crate::bin())
            .args(args)
            .current_dir(home)
            .env("HOME", home)
            .env_remove("XDG_DATA_HOME")
            .env_remove("PATH")
            .env("RUST_LOG", "off")
            .output()
            .expect("run memory-wire");
        (
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
            out.status.code().expect("an exit code, not a signal"),
        )
    }

    /// `--list` is the input to the decision of which hosts to wire, so it must
    /// not make one: every file under `HOME` is byte-identical afterwards, a
    /// host this machine does not have still appears, and the answer is one line
    /// per host with nothing else around it.
    #[test]
    fn list_should_report_every_host_and_write_nothing() {
        let home = cli_scratch("list");
        // One host present and already wired, one present and holding only a
        // foreign hook: both states of the second column read off a real file.
        std::fs::create_dir_all(home.join(".codex")).expect("mkdir");
        std::fs::write(
            home.join(".codex/hooks.json"),
            r#"{"hooks":{"Stop":[{"matcher":"","hooks":[{"type":"command","command":"/usr/bin/memory-wire hook stop"}]}]}}"#,
        )
        .expect("write");
        std::fs::create_dir_all(home.join(".claude")).expect("mkdir");
        std::fs::write(home.join(".claude/settings.json"), "{\n  \"model\": \"opus\"\n}\n")
            .expect("write");
        let before = snapshot(&home);

        let (out, err, code) = connect_run(&["connect", "--list"], &home);
        assert_eq!(code, 0, "a machine with no host at all is not an error: {err}");
        assert_eq!(snapshot(&home), before, "--list must not create, edit or remove a file");

        let lines: Vec<&str> = out.lines().filter(|l| !l.trim().is_empty()).collect();
        assert_eq!(lines.len(), connect::ALL.len(), "one line per host, no banner: {out}");
        for (line, host) in lines.iter().zip(connect::ALL) {
            assert!(line.starts_with(&format!("{:<12}", host.id())), "{line}");
        }
        let fields = |id: &str| -> Vec<String> {
            lines
                .iter()
                .find(|l| l.starts_with(id))
                .unwrap_or_else(|| panic!("{id} is missing from:\n{out}"))
                .split_whitespace()
                .map(str::to_string)
                .collect()
        };
        assert_eq!(fields("codex"), ["codex", "detected=yes", "wired=yes"]);
        assert_eq!(fields("claude-code"), ["claude-code", "detected=yes", "wired=no"]);
        assert_eq!(fields("hermes"), ["hermes", "detected=no", "wired=no"]);
        // pi and omp take an extension as well as a config, and a host with
        // neither surface on this machine still has to be named.
        assert_eq!(fields("pi"), ["pi", "detected=no", "wired=no"]);
        assert_eq!(fields("omp"), ["omp", "detected=no", "wired=no"]);
        std::fs::remove_dir_all(&home).ok();
    }

    /// pi and omp take an extension as well as a config, and a host with neither
    /// surface on this machine still has to be named. An omp home directory with
    /// no `mcp.json` and no extension is the "detected but not wired" state, read
    /// off the real tree rather than stubbed.
    #[test]
    fn the_extension_hosts_are_named_by_connect_list_too() {
        let home = cli_scratch("list-ext");
        std::fs::create_dir_all(home.join(".omp")).expect("mkdir");
        let (out, err, code) = connect_run(&["connect", "--list"], &home);
        assert_eq!(code, 0, "{err}");
        let fields = |id: &str| -> Vec<String> {
            out.lines()
                .find(|l| l.starts_with(id))
                .unwrap_or_else(|| panic!("{id} missing from:\n{out}"))
                .split_whitespace()
                .map(str::to_string)
                .collect()
        };
        assert_eq!(fields("omp"), ["omp", "detected=yes", "wired=no"]);
        assert_eq!(fields("pi"), ["pi", "detected=no", "wired=no"]);
        std::fs::remove_dir_all(&home).ok();
    }

    /// One call may name several hosts, and a name repeated inside it is one
    /// host: wired once, reported once, one entry per lifecycle event.
    #[test]
    fn repeated_hosts_in_one_invocation_should_wire_once() {
        for argv in [
            &["connect", "codex,codex"][..],
            &["connect", "codex", "codex"][..],
            &["connect", "codex, codex"][..],
        ] {
            let home = cli_scratch("dupe");
            std::fs::create_dir_all(home.join(".codex")).expect("mkdir");
            let (out, err, code) = connect_run(argv, &home);
            assert_eq!(code, 0, "{argv:?} stderr: {err}");
            let report: Vec<&str> = out.lines().filter(|l| l.starts_with("codex")).collect();
            assert_eq!(report.len(), 1, "{argv:?} reported codex {} times:\n{out}", report.len());
            assert!(report[0].contains(" wired"), "{out}");
            // Counted structurally rather than by the marker substring: the exe
            // path this run wires in is the build tree, whose directory is named
            // after the project too, so one entry can carry the marker twice.
            let doc: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(home.join(".codex/hooks.json")).expect("read"))
                    .expect("json");
            let entries: usize = doc["hooks"]
                .as_object()
                .expect("hooks")
                .values()
                .map(|ev| ev.as_array().map_or(0, Vec::len))
                .sum();
            assert_eq!(entries, 5, "{argv:?} must leave one entry per lifecycle event");
            std::fs::remove_dir_all(&home).ok();
        }
    }

    /// A name that is not a host is refused in one turn: nonzero, repeating the
    /// name it was given and naming every name that would have worked.
    #[test]
    fn an_unknown_host_name_should_be_refused_with_the_valid_ones() {
        let home = cli_scratch("bogus");
        let (out, err, code) = connect_run(&["connect", "claude_code"], &home);
        assert_ne!(code, 0, "a name that is not a host must not exit zero");
        let said = format!("{out}{err}");
        assert!(said.contains("claude_code"), "{said}");
        for host in connect::ALL {
            assert!(said.contains(host.id()), "{} missing from the refusal:\n{said}", host.id());
        }
        assert!(snapshot(&home).is_empty(), "a refused call must write nothing");
        std::fs::remove_dir_all(&home).ok();
    }

    /// A private, empty directory for one CLI run: its cwd, its `HOME`, and
    /// nowhere near the real store.
    fn cli_scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mw-cli-{}-{name}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    /// One run of the compiled binary with `RUST_LOG` at `level`, both streams
    /// captured separately.
    fn run_cli(level: &str, args: &[String], home: &std::path::Path) -> (String, String, Option<i32>) {
        let out = std::process::Command::new(crate::bin())
            .args(args)
            .current_dir(home)
            .env("HOME", home)
            .env_remove("XDG_DATA_HOME")
            .env("RUST_LOG", level)
            .output()
            .expect("run memory-wire");
        (
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
            out.status.code(),
        )
    }

    /// The argv one command needs, given its own scratch directory. `seed`
    /// writes, so it is always pointed at a throwaway store; `doctor` is pointed
    /// at a path that does not exist, which it must report without creating.
    fn argv(name: &str, scratch: &std::path::Path) -> Vec<String> {
        match name {
            "seed" => vec![
                "seed".into(),
                "--commits".into(),
                "1".into(),
                "--db".into(),
                scratch.join("memory.db").display().to_string(),
            ],
            "doctor" => vec![
                "doctor".into(),
                "--db".into(),
                scratch.join("missing.db").display().to_string(),
            ],
            other => vec![other.into()],
        }
    }

    // H5.3 — the log level must not move a byte of any command's stdout, and
    // the startup line must be on stderr. `off` is the baseline and `trace` is
    // the loudest the subscriber can get; byte equality between the two is
    // exactly the claim, and it is a claim about placement rather than about
    // any one command's formatting. `serve` never returns, so it has its own
    // test below.
    #[test]
    fn the_log_level_should_never_reach_a_commands_stdout() {
        for name in ["info", "seed", "doctor", "connect"] {
            let quiet_home = cli_scratch(&format!("{name}-off"));
            let (quiet, _, code) = run_cli("off", &argv(name, &quiet_home), &quiet_home);
            assert_eq!(code, Some(0), "{name} at RUST_LOG=off");
            assert!(!quiet.is_empty(), "{name} must report on stdout: {quiet:?}");

            // `connect` writes to HOME, so it gets a virgin one for its second
            // run; the other three are pure functions of the directory they are
            // pointed at, and reusing it is what makes the two stdouts comparable
            // byte for byte — `seed` names its directory in the "skipped git" line.
            let loud_home = match name {
                "connect" => cli_scratch(&format!("{name}-trace")),
                _ => quiet_home,
            };
            let (loud, err, code) = run_cli("trace", &argv(name, &loud_home), &loud_home);
            assert_eq!(code, Some(0), "{name} at RUST_LOG=trace");
            assert_eq!(
                loud, quiet,
                "{name}: RUST_LOG=trace changed stdout — a log line reached it"
            );
            if name == "seed" {
                // The startup line exists, and it is on the other stream.
                assert!(err.contains("memory-wire db:"), "the store it opened: {err:?}");
                assert!(!quiet.contains("memory-wire db:"), "{quiet:?}");
            }
        }
    }

    // `serve` is the one command that never returns, so it is checked by
    // starting it, reading its startup lines off stderr, and killing it: for
    // its whole life stdout must stay empty.
    #[test]
    fn serve_should_log_its_startup_lines_on_stderr_and_nothing_on_stdout() {
        use std::io::{BufRead, BufReader};
        let dir = cli_scratch("serve");
        let mut child = std::process::Command::new(crate::bin())
            .args(["serve", "--addr", "127.0.0.1:0"])
            .arg("--db")
            .arg(dir.join("memory.db"))
            .current_dir(&dir)
            .env("HOME", &dir)
            .env_remove("XDG_DATA_HOME")
            .env("RUST_LOG", "trace")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn memory-wire serve");
        let (tx, rx) = std::sync::mpsc::channel();
        let stderr = child.stderr.take().expect("stderr pipe");
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines() {
                if tx.send(line.unwrap_or_default()).is_err() {
                    return;
                }
            }
        });

        // Two `info!` calls and nothing else: the store it opened, then the
        // address it bound. Bounded by a deadline rather than a line count, so a
        // crash shows up as a missing line instead of a hang.
        let mut seen = String::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while std::time::Instant::now() < deadline && !seen.contains("serving on") {
            match rx.recv_timeout(Duration::from_millis(250)) {
                Ok(line) => seen.push_str(&line),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => break,
            }
        }
        let _ = child.kill();
        let out = child.wait_with_output().expect("wait for serve");
        assert!(seen.contains("memory-wire db:"), "the store it opened: {seen:?}");
        assert!(seen.contains("serving on"), "the address it bound: {seen:?}");
        assert!(
            out.stdout.is_empty(),
            "stdout is not a channel serve may write to: {:?}",
            String::from_utf8_lossy(&out.stdout)
        );
    }

    // The `sweep` subcommand end to end, because the four unit tests above prove
    // the arithmetic and not that argv reaches it: a `--dry-run` that forgets
    // nothing, then a real sweep that forgets exactly the one expired row of the
    // one bank that asked. The `default` bank `open_store` seeds has no policy,
    // so the skipped bucket is populated by the fixture itself.
    #[test]
    fn sweep_should_forget_only_the_bank_that_asked_and_only_when_asked() {
        let dir = cli_scratch("sweep");
        let db = dir.join("memory.db");
        {
            let store = open_store(&db).expect("store");
            store.put_bank(&Bank { id: "aging".into(), name: "aging".into() }).expect("bank");
            store
                .set_bank_config("aging", r#"{"ttl_days":1}"#)
                .expect("config");
            let row = |id: &str, content: &str, created_at: &str| Memory {
                id: id.to_string(),
                bank_id: "aging".to_string(),
                content: content.to_string(),
                context: None,
                created_at: (!created_at.is_empty()).then(|| created_at.to_string()),
            };
            store
                .put(&row("gone", "jose middleware from last year", "2020-01-01T00:00:00.000Z"))
                .expect("put");
            // `None`, so the store stamps it on insert: newer than a one-day cutoff.
            store
                .put(&row("kept", "jose middleware from today", ""))
                .expect("put");
        }
        let argv = |dry: bool| {
            let mut a = vec!["sweep".to_string()];
            if dry {
                a.push("--dry-run".to_string());
            }
            a.push("--db".to_string());
            a.push(db.display().to_string());
            a
        };
        // Reopened from the file each time, so the assertions read what is on disk
        // rather than what this test's own handle believes it wrote.
        let rows_in_aging = |want: usize| {
            let got = SqliteStore::open(&db)
                .expect("reopen")
                .list("aging")
                .expect("list")
                .len();
            assert_eq!(got, want, "the aging bank holds {got} rows, wanted {want}");
        };

        // --dry-run reports the count and writes nothing.
        let (dry, _, code) = run_cli("off", &argv(true), &dir);
        assert_eq!(code, Some(0), "a dry run is a normal run: {dry}");
        assert!(dry.contains("aging"), "the swept bank is named: {dry}");
        assert!(dry.contains("would delete 1"), "{dry}");
        assert!(
            dry.contains("default") && dry.contains("skipped (no ttl_days)"),
            "a bank with no policy is listed as skipped: {dry}"
        );
        rows_in_aging(2);

        // Without it, the same invocation forgets the expired row and keeps the
        // rest, and the FTS index follows so the content is unfindable.
        let (out, _, code) = run_cli("off", &argv(false), &dir);
        assert_eq!(code, Some(0), "{out}");
        assert!(out.contains("deleted 1"), "{out}");
        assert!(!out.contains("would"), "{out}");
        let store = SqliteStore::open(&db).expect("reopen");
        let left = store.list("aging").expect("list");
        assert_eq!(left.len(), 1, "{left:?}");
        assert_eq!(left[0].id, "kept");
        assert!(
            store.keyword_search("aging", "middleware", 10).expect("search").len() == 1,
            "the expired row must leave the index with it"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    // A bank whose retention window reaches further out than a date can hold is
    // skipped, not a panic: `ttl_days` is a number a human typed into a config.
    #[test]
    fn sweep_should_skip_a_ttl_no_date_can_reach() {
        let dir = cli_scratch("sweep-range");
        let db = dir.join("memory.db");
        {
            let store = open_store(&db).expect("store");
            store.put_bank(&Bank { id: "forever".into(), name: "forever".into() }).expect("bank");
            store
                .set_bank_config("forever", r#"{"ttl_days":4294967295}"#)
                .expect("config");
            store
                .put(&Memory {
                    id: "m1".into(),
                    bank_id: "forever".into(),
                    content: "a very old note".into(),
                    context: None,
                    created_at: Some("1970-01-01T00:00:00.000Z".into()),
                })
                .expect("put");
        }
        let (out, _, code) = run_cli(
            "off",
            &["sweep".to_string(), "--db".to_string(), db.display().to_string()],
            &dir,
        );
        assert_eq!(code, Some(0), "{out}");
        assert!(out.contains("skipped"), "{out}");
        assert_eq!(
            SqliteStore::open(&db).expect("reopen").list("forever").expect("list").len(),
            1,
            "a skipped bank keeps everything"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    // Only a loopback socket address passes in silence. `localhost:8899` is not
    // one: it is unproven rather than proven-open, and warning on it costs a
    // line nobody needs to act on, while staying quiet on a real exposure costs
    // the store.
    #[test]
    fn a_non_loopback_bind_should_be_named_once_on_stderr() {
        assert_eq!(exposure_warning("127.0.0.1:8899"), None);
        assert_eq!(exposure_warning("[::1]:8899"), None);
        let wide = exposure_warning("0.0.0.0:8899").expect("0.0.0.0 is not loopback");
        assert!(wide.contains("0.0.0.0:8899"), "{wide}");
        assert!(wide.contains("no authentication"), "{wide}");
        assert!(!wide.contains('\n'), "one line, or it is a paragraph: {wide}");
        assert!(exposure_warning("192.168.1.5:8899").is_some());
        assert!(exposure_warning("localhost:8899").is_some());
    }

    // The bind default and the client default are one value, pinned.
    //
    // `serve --addr` and `daemon start --addr` each carried their own literal,
    // and both said 8899 while `paths::endpoint()` said 8888 — so a server
    // started with no arguments sat on a port no hook, no `doctor` and no
    // documented client ever looked for. The default is read back out of the
    // built clap command rather than off the source text, so this fails if
    // either command's default is edited away from `paths::DEFAULT_ADDR` again.
    #[test]
    fn the_addr_default_should_be_the_port_every_client_looks_for() {
        use clap::CommandFactory;

        /// The `--addr` default clap would apply along `path`, from the real command.
        fn addr_default(path: &[&str]) -> String {
            let mut cmd = Cli::command();
            for name in path {
                cmd = cmd
                    .find_subcommand_mut(name)
                    .unwrap_or_else(|| panic!("no `{name}` subcommand"))
                    .clone();
            }
            // Bound to a local: the iterator borrows `cmd`, and naming it in the
            // chain leaves a temporary that outlives the value it borrows from.
            let arg = cmd
                .get_arguments()
                .find(|a| a.get_id() == "addr")
                .cloned();
            let found = arg
                .and_then(|a| a.get_default_values().first().map(|v| v.to_string_lossy().into_owned()));
            found.unwrap_or_else(|| panic!("`{}` has no --addr default", path.join(" ")))
        }

        assert_eq!(addr_default(&["serve"]), paths::DEFAULT_ADDR);
        assert_eq!(addr_default(&["daemon", "start"]), paths::DEFAULT_ADDR);

        // Pinned to the port the docs, the installer and every client already
        // assume. 8888 is the choice that breaks the least, so it is the one
        // that stays.
        assert_eq!(paths::DEFAULT_ADDR, "127.0.0.1:8888");

        // And the URL a client falls back to with no override must be that same
        // address. Asserted on `default_endpoint()` rather than `endpoint()`,
        // because `endpoint()` also consults a running daemon's `serve.json` --
        // which is the point of it, and which would make this test depend on
        // whether a daemon happens to be up on the machine running it.
        assert_eq!(paths::default_endpoint(), format!("http://{}", paths::DEFAULT_ADDR));
    }

    // The drain, driven directly instead of by signalling a real process.
    //
    // "In flight" is produced rather than waited for: a second connection holds
    // the store's write lock, so the retain's handler cannot return until this
    // test releases it. Asserting that no answer arrives while the lock is held
    // is therefore not a timing guess — a response would mean the write had
    // succeeded, which the lock forbids. The signal then lands with the handler
    // provably mid-request, and the response still has to come.
    #[test]
    fn a_shutdown_should_drain_the_request_in_flight_and_then_stop_accepting() {
        use std::io::{ErrorKind, Read, Write};

        let dir = cli_scratch("drain");
        let db = dir.join("memory.db");
        let store = open_store(&db).expect("store");
        let (bound_tx, bound_rx) = std::sync::mpsc::channel();
        let (stopped_tx, stopped_rx) = std::sync::mpsc::channel();
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
        let server = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("rt")
                .block_on(async {
                    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                        .await
                        .expect("bind");
                    bound_tx.send(listener.local_addr().expect("addr")).ok();
                    let shutdown = async {
                        let _ = stop_rx.await;
                    };
                    serve_until(listener, router(Arc::new(MemoryService::new(store))), shutdown)
                        .await
                        .expect("serve");
                    stopped_tx.send(()).ok();
                });
        });
        let addr = bound_rx.recv().expect("server never bound");

        // The blocker goes first: the handler must already be waiting on the lock
        // by the time the signal fires, and it cannot pass this point until later.
        let blocker = rusqlite::Connection::open(&db).expect("open blocker");
        blocker.execute_batch("BEGIN IMMEDIATE").expect("hold the write lock");

        let body = format!(r#"{{"content":"{}"}}"#, "jose middleware ".repeat(500));
        let mut sock = std::net::TcpStream::connect(addr).expect("connect");
        sock.set_read_timeout(Some(Duration::from_millis(100)))
            .expect("short read timeout");
        sock.write_all(
            format!(
                "POST /banks/drain/retain HTTP/1.1\r\nHost: {addr}\r\n\
                 Content-Type: application/json\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .expect("request");
        sock.flush().expect("flush");

        // A bounded window for the handler to start, with no guess in it.
        let mut answered = false;
        for _ in 0..8 {
            match sock.read(&mut [0u8; 64]) {
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                _ => {
                    answered = true;
                    break;
                }
            }
        }
        assert!(!answered, "the handler cannot finish while the write lock is held");

        // The signal lands with that handler still running, and the lock is
        // released behind it.
        stop_tx.send(()).ok();
        blocker.execute_batch("ROLLBACK").expect("release the write lock");

        sock.set_read_timeout(Some(Duration::from_secs(20)))
            .expect("long read timeout");
        let mut reply = String::new();
        sock.read_to_string(&mut reply).expect("read");
        assert!(
            reply.starts_with("HTTP/1.1 200"),
            "a request in flight when the signal fires must be answered, not cut: {reply}"
        );

        // And the drain is over: the listener is gone, so nothing new is accepted.
        stopped_rx
            .recv_timeout(Duration::from_secs(20))
            .expect("serve_until must return once the drain finishes");
        server.join().expect("server thread");
        assert!(
            std::net::TcpStream::connect(addr).is_err(),
            "the port must stop accepting once the drain is done"
        );
        // The drained write landed, not just a response header: the memory is there.
        assert_eq!(
            SqliteStore::open(&db).expect("reopen").list("drain").expect("list").len(),
            1,
            "the retained row must survive the shutdown that followed it"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    // A request whose body is still arriving when the signal fires is a
    // different case from a handler already running, and it is the one the
    // connection layer decides rather than the handler: an upload the client
    // abandons leaves the parser short of `Content-Length`, so the retain never
    // runs. Pinned here because it is the shape an interrupted upload takes, and
    // a caller has to be able to tell it apart from a lost memory — a retain
    // that was still arriving wrote nothing, and the next one can simply be
    // repeated.
    //
    // The ordering is a round-trip, not a sleep. `Expect: 100-continue` makes
    // hyper send its interim response at the moment it starts reading the body,
    // so receiving it proves the head landed and the request is in flight with
    // the body still incomplete. Only then is the signal sent. Previously the
    // signal raced a bare `flush`, so under load the client could finish the
    // upload and get a 200 before the server ever saw the shutdown; a graceful
    // drain lets an in-flight body finish, so that 200 was the correct server
    // behaviour and the old timing was simply the bug.
    #[test]
    fn a_shutdown_mid_upload_should_reset_rather_than_half_write() {
        use std::io::{Read, Write};
        use std::net::Shutdown;

        let dir = cli_scratch("drain-upload");
        let db = dir.join("memory.db");
        let store = open_store(&db).expect("store");
        let (bound_tx, bound_rx) = std::sync::mpsc::channel();
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
        let server = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("rt")
                .block_on(async {
                    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                        .await
                        .expect("bind");
                    bound_tx.send(listener.local_addr().expect("addr")).ok();
                    let shutdown = async {
                        let _ = stop_rx.await;
                    };
                    serve_until(listener, router(Arc::new(MemoryService::new(store))), shutdown)
                        .await
                        .expect("serve");
                });
        });
        let addr = bound_rx.recv().expect("server never bound");

        let body = format!(r#"{{"content":"{}"}}"#, "jose middleware ".repeat(20_000));
        let mut sock = std::net::TcpStream::connect(addr).expect("connect");
        sock.set_read_timeout(Some(Duration::from_secs(20)))
            .expect("read timeout");
        // Head only. The declared `Content-Length` is the whole body and not one
        // byte of it is sent, so this request can only ever end in a parse error.
        sock.write_all(
            format!(
                "POST /banks/upload/retain HTTP/1.1\r\nHost: {addr}\r\n\
                 Content-Type: application/json\r\nContent-Length: {}\r\n\
                 Expect: 100-continue\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .as_bytes(),
        )
        .expect("head");
        sock.flush().expect("flush");

        // The round-trip: the interim response is the server telling us it has
        // the head and is now blocked on a body that is not coming.
        let mut interim = Vec::new();
        let mut byte = [0u8; 1];
        for _ in 0..64 {
            match sock.read(&mut byte) {
                Ok(1) => interim.push(byte[0]),
                _ => break,
            }
            if interim.ends_with(b"\r\n\r\n") {
                break;
            }
        }
        let interim = String::from_utf8_lossy(&interim);
        assert!(
            interim.starts_with("HTTP/1.1 100"),
            "the server must confirm it has the head before the cut, or the signal is \
             not provably racing an in-flight upload: {interim:?}"
        );

        // The signal lands with the body provably incomplete.
        stop_tx.send(()).ok();
        // And the client abandons the upload rather than completing it: the FIN
        // leaves the server short of the `Content-Length` it declared.
        sock.shutdown(Shutdown::Both).ok();

        let mut reply = String::new();
        let _ = sock.read_to_string(&mut reply);
        // Either a reset or a 4xx/5xx: what must never happen is a 200, which
        // would be a memory stored from a request the client cannot trust it
        // finished sending.
        assert!(
            !reply.starts_with("HTTP/1.1 200"),
            "an interrupted upload must not report a stored memory: {reply}"
        );
        // Nothing was stored either way, so the half-received retain left no row.
        assert!(
            SqliteStore::open(&db)
                .expect("reopen")
                .list("upload")
                .expect("list")
                .is_empty(),
            "an upload cut off mid-body must leave no memory behind"
        );
        // The server returns once the abandoned connection is gone; nothing is left
        // running against the scratch directory the assertion below removes.
        server.join().expect("server thread");
        std::fs::remove_dir_all(&dir).ok();
    }

    // H5.4 — one subscriber, one writer, verified by grep rather than trusted:
    // `main` is the only site that installs a subscriber, and its writer is
    // stderr. That is what lets `mcp` and `hook` treat stdout as a channel, and
    // a second install would either double every line or panic on the global
    // default — so the count is the record.
    #[test]
    fn tracing_should_have_exactly_one_subscriber_and_it_should_write_to_stderr() {
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut sites: Vec<(String, usize, String)> = Vec::new();
        for entry in std::fs::read_dir(&src).expect("read src") {
            let path = entry.expect("src entry").path();
            if !path.extension().is_some_and(|e| e == "rs") {
                continue;
            }
            let file = path.file_name().expect("file name").to_string_lossy().into_owned();
            let source = std::fs::read_to_string(&path).expect("read source");
            let lines: Vec<&str> = source.lines().collect();
            for (n, line) in lines.iter().enumerate() {
                let trimmed = line.trim_start();
                // The install is a call, not a mention: matching the path at the
                // start of a line finds the builder chain and not this test's own
                // assertions about it. The chain spans several lines, so the
                // record keeps a short window around the call.
                if trimmed.starts_with("tracing_subscriber::") {
                    let window = &lines[n..n.saturating_add(5).min(lines.len())];
                    sites.push((file.clone(), n + 1, window.join(" ")));
                }
            }
        }
        assert_eq!(
            sites.len(),
            1,
            "a second subscriber install would double-log or panic on the global default: {sites:?}"
        );
        assert_eq!(sites[0].0, "main.rs", "{}", sites[0].2);
        assert!(
            sites[0].2.contains("with_writer(std::io::stderr)"),
            "the writer is the whole point: {}",
            sites[0].2
        );
    }
}
