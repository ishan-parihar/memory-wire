//! What a bare `memory-wire` prints, and how a rejected flag gets fixed.
//!
//! Two jobs, both from the same place — the caller is an agent that has one
//! turn to spend:
//!
//! - **No arguments is not "no information".** The home view reports live state
//!   — bank, store, newest memory, tags, whether a server is answering — so the
//!   first line read is actionable. A usage manual instead makes the caller
//!   spend a second turn to find out what is true right now.
//! - **A usage error is correctable in one turn.** The error and the offending
//!   subcommand's own concise help land on stdout together, so the fix is in the
//!   response that reported the problem rather than in a follow-up call.
//!
//! The never-fail discipline is [`crate::hooks`]'s and is inherited on purpose:
//! a hook must never put an error in front of a model, and a home view that
//! exits nonzero because a database is missing is that same failure in
//! different clothes. Every field here therefore degrades to a plain statement
//! about what could not be determined, and the process exits 0.

use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};
use clap::error::ErrorKind;
use clap::CommandFactory;

use crate::paths;
use crate::Cli;

/// One sentence: what this binary is, on the first line of every home view.
///
/// First because it is the one line an agent needs before anything else
/// matters, and because a tool that only announces itself after you have read
/// five lines of state is already a cost.
const DESCRIPTION: &str = "Agent memory over HTTP and MCP — bank-isolated SQLite + FTS5, \
                          no daemon until you start one";

/// Ceiling on the one network call the home view makes.
///
/// A quarter of a second, not the two seconds `paths::IO_TIMEOUT` allows a
/// hook. `/health` on loopback answers in microseconds, so the long budget
/// exists for a hook that must not hang a session; a view exists to be read
/// first, and one that waits two seconds to say "nothing is listening" is a view
/// the caller stops calling. The deadline still covers connect, write and read
/// together — `http::get` takes one total budget — so this is a hard ceiling
/// and not three of them.
const PROBE_TIMEOUT: Duration = Duration::from_millis(250);

/// Ceiling on a single read of the local store.
///
/// rusqlite's default is five seconds, which for a view is an eternity and, on
/// a database a live server is writing, is reachable. A read-only WAL reader
/// does not block on the writer, so a short budget is never spent in the normal
/// case; it is here so the abnormal one cannot turn the view into a hang.
const BUSY_TIMEOUT: Duration = Duration::from_millis(200);

/// Tags named on the `tags` line. Enough to orient, few enough to stay a line.
const TAG_LIMIT: usize = 5;

/// The stamp `store::UNKNOWN_CREATED_AT` writes to a row that predates
/// timestamping.
///
/// Spelled out rather than imported: that constant is private to the library
/// crate, and making it public is a change to `src/store.rs`, which this work
/// does not own. Epoch is a fixed literal in the schema default as well, so
/// there is nothing to drift — but it is the one value in this file copied from
/// another module, and it is copied because the alternative was an edit outside
/// the fence.
const PRETIMESTAMP_STAMP: &str = "1970-01-01T00:00:00Z";

/// Longest driver message kept on a value cell; past this the cell stops being
/// readable and starts being a stack trace with a newline in it.
const WHY_CHARS: usize = 60;

/// What one bounded probe of `/health` found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Server {
    /// 2xx with a body of exactly `ok`.
    Up,
    /// Nothing answered inside [`PROBE_TIMEOUT`].
    Silent,
    /// Something answered, and it is not us.
    Foreign,
}

impl Server {
    /// The word on the `server` line.
    /// The bare word for the state, with the reason kept in the field so the
    /// caller can place it: the line already ends in `(endpoint ...)`, and two
    /// adjacent parenthesised clauses read as one run-on.
    fn label(self) -> &'static str {
        match self {
            Server::Up => "running",
            Server::Silent => "not running",
            // A 2xx from a stranger is the case `doctor` refuses to call healthy,
            // because any process on this port answers 200. Naming it stops the
            // line from reading as "start the server" when starting it would
            // fail on the bind.
            Server::Foreign => "not running — another process holds the port",
        }
    }
}

/// What the store holds for one bank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Facts {
    /// No store at this path.
    Missing,
    /// Opened and read.
    Read(BankFacts),
    /// Present but not readable; the driver's own words, trimmed.
    Unreadable(String),
}

/// One bank's rows, its newest stamp, and its most-used tags.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BankFacts {
    /// Rows in the bank.
    pub memories: i64,
    /// Newest `created_at`, exactly as the store holds it.
    pub newest: Option<String>,
    /// Tags by descending use, ties broken by name.
    pub tags: Vec<String>,
}

/// The home view, with every field already decided.
///
/// Split from [`render`] so the text can be asserted on without a database, a
/// clock, or a network. [`collect`] is the half that touches the outside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    /// This executable's own path, or `None` when it cannot be resolved.
    pub bin: Option<String>,
    /// Endpoint every client resolves to.
    pub endpoint: String,
    /// What the bounded probe found.
    pub server: Server,
    /// Bank this directory resolves to.
    pub bank: String,
    /// Store path, home-collapsed.
    pub store: String,
    /// Bytes the store occupies, sidecars included.
    pub store_bytes: u64,
    /// Whether a store is on disk at all.
    pub store_present: bool,
    /// The bank's contents.
    pub facts: Facts,
}

/// Print the home view to stdout.
///
/// No failure path: `render` cannot fail, and the only work that could is
/// already degrading to a plain statement. Falls out of `main` normally so the
/// process exits 0.
pub fn print_home() {
    print!("{}", render(&collect(), Utc::now()));
}

/// The home view as text.
fn render(view: &View, now: DateTime<Utc>) -> String {
    let mut out = String::new();
    if let Some(bin) = &view.bin {
        out.push_str(&format!("bin: {bin}\n"));
    }
    out.push_str(&format!("description: {DESCRIPTION}\n\n"));
    out.push_str(&row(
        "server",
        &format!("{}  (endpoint {})", view.server.label(), view.endpoint),
    ));
    out.push('\n');
    out.push_str(&row("bank", &view.bank));
    out.push('\n');
    out.push_str(&row("store", &view.store_label()));
    out.push('\n');
    out.push_str(&row("memories", &view.memories_label(now)));
    out.push('\n');
    if let Some(tags) = view.tags_label() {
        out.push_str(&row("tags", &tags));
        out.push('\n');
    }
    let help = view.help();
    out.push_str(&format!("\nhelp[{}]:\n", help.len()));
    for line in help {
        out.push_str(&format!("  {line}\n"));
    }
    out
}

/// One `label   value` line, padded so the values form a column.
fn row(label: &str, value: &str) -> String {
    format!("{label:<10} {value}")
}

impl View {
    /// The `store` value: the path, and either its size or that there is none.
    fn store_label(&self) -> String {
        if self.store_present {
            format!("{}  ({})", self.store, paths::human_bytes(self.store_bytes))
        } else {
            format!("{}  (no store yet)", self.store)
        }
    }

    /// The `memories` value: the count, plus how old the newest row is.
    ///
    /// `unknown` rather than `0` whenever the store could not be read: zero is a
    /// claim about a bank, and a bank nobody could open is not an empty one.
    fn memories_label(&self, now: DateTime<Utc>) -> String {
        match &self.facts {
            Facts::Missing => "unknown  ·  no store at this path yet".to_string(),
            Facts::Unreadable(why) => format!("unknown  ·  {why}"),
            Facts::Read(f) => match f.newest.as_deref() {
                Some(stamp) => format!("{}  ·  newest {}", f.memories, ago(stamp, now)),
                None => f.memories.to_string(),
            },
        }
    }

    /// The `tags` value, or `None` when the line would carry nothing worth it.
    ///
    /// Withheld rather than filled when the store is unreadable: the
    /// `memories` line has already said so, and a second copy of the same
    /// failure is noise.
    fn tags_label(&self) -> Option<String> {
        match &self.facts {
            Facts::Read(f) if !f.tags.is_empty() => Some(f.tags.join(", ")),
            _ => None,
        }
    }

    /// The next steps worth naming, most useful first.
    ///
    /// Correct before they are useful: every one is a command that exists in
    /// this binary today, runnable as written, with no placeholder standing in
    /// for a value that has to be invented. The starting one is chosen by what
    /// the view just observed, so a caller that already has a server is never
    /// told to start another.
    fn help(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if self.server != Server::Up {
            lines.push(format!(
                "Run `memory-wire daemon start` to serve on {}",
                self.endpoint
            ));
        }
        lines.push("Run `memory-wire doctor` for endpoint, store and server health".to_string());
        lines.push("Run `memory-wire connect` to wire up agent harnesses".to_string());
        lines.push("Run `memory-wire --help` for the full surface".to_string());
        lines
    }
}

/// Gather the home view's facts.
///
/// Every field is decided independently, so one that cannot be resolved leaves
/// its line out or says why and never removes another line or fails.
fn collect() -> View {
    let endpoint = paths::endpoint();
    let bank = paths::resolve_bank();
    // The same opener `serve`, `sweep` and `seed` default to, so the store
    // reported on is the store those commands would touch.
    let store = memory_wire::store::default_db_path();
    View {
        bin: bin_path(),
        server: probe(&endpoint),
        endpoint,
        bank: bank.clone(),
        store: collapse(&store),
        store_bytes: store_bytes(&store),
        store_present: store.exists(),
        facts: facts(&store, &bank),
    }
}

/// This executable's own path, home-collapsed, or `None` when unresolvable.
///
/// `current_exe` rather than a name off `argv[0]`: a PATH-resolved
/// `memory-wire` is not a path, and the whole point of the line is that the
/// caller can copy it into something that will run. Collapsed to `~` because an
/// absolute `/home/<user>/…` is noise in every report and does not survive
/// being pasted to a machine with a different home.
///
/// `None` rather than a guess: a line that names a path this process is not at
/// is worse than no line, because it is believed.
fn bin_path() -> Option<String> {
    Some(collapse(&std::env::current_exe().ok()?))
}

/// A path with a leading home directory replaced by `~`.
fn collapse(path: &Path) -> String {
    match paths::home().ok().and_then(|home| tilde(path, &home)) {
        Some(t) => t,
        None => path.display().to_string(),
    }
}

/// `path` with a leading `home` replaced by `~`, or `None` when it is not under
/// `home` at all.
///
/// `None` rather than a partial match: `~`-collapsing `/home/x/y` against home
/// `/home/x` produces `/home/x/y` with only its tail rewritten, which is a path
/// that does not exist. The comparison is component-wise, so `/home/xa` is not
/// under `/home/x` either.
fn tilde(path: &Path, home: &Path) -> Option<String> {
    let rest = path.strip_prefix(home).ok()?;
    // The home itself collapses to `~`, not to `~/`.
    if rest.as_os_str().is_empty() {
        return Some("~".to_string());
    }
    Some(format!("~/{}", rest.display()))
}

/// Bytes the store occupies, SQLite sidecars included.
///
/// The same three files `doctor` measures, and for the same reason: a WAL
/// database keeps uncommitted pages in `-wal` and a shared index in `-shm`, so
/// the main file alone under-reports what the store costs. `doctor`'s own
/// helper is private and its only public entry point folds the WAL into the
/// main file and probes with a two-second budget — and a view must neither
/// write to the store it reports on nor wait two seconds on a port, so the
/// three `stat` calls are repeated rather than reached for.
fn store_bytes(store: &Path) -> u64 {
    let mut total = 0u64;
    for suffix in ["", "-wal", "-shm"] {
        let mut name = store.as_os_str().to_os_string();
        name.push(suffix);
        total += std::fs::metadata(PathBuf::from(name)).map_or(0, |m| m.len());
    }
    total
}

/// One bounded `/health` probe, graded exactly as `doctor::probe_server` grades
/// it: 2xx **and** a body of exactly `ok`, because any process on this port
/// answers 200. Same question, same answer, shorter deadline — see
/// [`PROBE_TIMEOUT`].
fn probe(endpoint: &str) -> Server {
    match crate::http::get(&format!("{endpoint}/health"), PROBE_TIMEOUT) {
        Ok(r) if r.ok() && r.body.trim() == "ok" => Server::Up,
        Ok(_) => Server::Foreign,
        Err(_) => Server::Silent,
    }
}

/// Read the bank's rows read-only, or say why it could not.
///
/// Read-only **and never migrated**, on `doctor`'s reasoning:
/// `SqliteStore::open` creates schema, FTS tables and a WAL, so reaching for it
/// would let a view that only reports on a store mutate it — the exact thing
/// `doctor`'s module docs refuse. The two counts and the tag query are the ones
/// `SqliteStore::bank_stats` runs, plus the tag *names* it reduces to a count:
/// `tags deploy, auth, ops` is the useful half of `BankStats::tags`.
fn facts(store: &Path, bank: &str) -> Facts {
    if !store.exists() {
        return Facts::Missing;
    }
    let conn = match rusqlite::Connection::open_with_flags(
        store,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    ) {
        Ok(conn) => conn,
        Err(e) => return Facts::Unreadable(short(&e.to_string())),
    };
    let _ = conn.busy_timeout(BUSY_TIMEOUT);
    match read_facts(&conn, bank) {
        Ok(f) => Facts::Read(f),
        Err(why) => Facts::Unreadable(short(&why)),
    }
}

/// Three `SELECT`s on an already-open read-only handle.
fn read_facts(conn: &rusqlite::Connection, bank: &str) -> Result<BankFacts, String> {
    let memories = conn
        .query_row(
            "SELECT COUNT(*) FROM memories WHERE bank_id = ?1",
            [bank],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let newest = conn
        .query_row(
            "SELECT MAX(created_at) FROM memories WHERE bank_id = ?1",
            [bank],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(BankFacts {
        memories,
        newest,
        tags: top_tags(conn, bank).map_err(|e| e.to_string())?,
    })
}

/// The bank's most-used tags, most first, ties broken by name.
///
/// Bounded by `GROUP BY t.tag` over an index on the join column and by the
/// `LIMIT`, so the cost does not grow with the bank — the same shape as
/// `bank_stats`'s tag count, which the HTTP route serves per bank.
fn top_tags(conn: &rusqlite::Connection, bank: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT t.tag FROM memory_tags t JOIN memories m ON m.id = t.memory_id \
         WHERE m.bank_id = ?1 GROUP BY t.tag ORDER BY COUNT(*) DESC, t.tag LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![bank, TAG_LIMIT as i64], |r| r.get(0))?;
    rows.collect()
}

/// How long ago an RFC 3339 stamp was, in the largest unit that reads true.
fn ago(stamp: &str, now: DateTime<Utc>) -> String {
    // The store's pre-timestamp sentinel is a real `created_at` value, and the
    // oldest it can hold. Rendering it as "56y ago" would be a confident answer
    // to a question the row does not answer, so it reads `unknown` — which is
    // what the sentinel means everywhere else in this crate.
    if stamp == PRETIMESTAMP_STAMP {
        return "unknown".to_string();
    }
    let Ok(then) = DateTime::parse_from_rfc3339(stamp) else {
        return "unknown".to_string();
    };
    // Clamped at zero rather than reported as negative: a stamp in the future is
    // clock skew, and `newest 0s ago` is the least alarming true statement.
    let secs = now
        .signed_duration_since(then.with_timezone(&Utc))
        .num_seconds()
        .max(0);
    const MINUTE: i64 = 60;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    const MONTH: i64 = 30 * DAY;
    const YEAR: i64 = 365 * DAY;
    if secs < MINUTE {
        "just now".to_string()
    } else if secs < HOUR {
        format!("{}m ago", secs / MINUTE)
    } else if secs < DAY {
        format!("{}h ago", secs / HOUR)
    } else if secs < MONTH {
        format!("{}d ago", secs / DAY)
    } else if secs < YEAR {
        format!("{}mo ago", secs / MONTH)
    } else {
        format!("{}y ago", secs / YEAR)
    }
}

/// A driver's complaint, on one line, short enough for a value cell.
fn short(why: &str) -> String {
    let line = why.lines().next().unwrap_or(why).trim();
    if line.chars().count() <= WHY_CHARS {
        return line.to_string();
    }
    let kept: String = line.chars().take(WHY_CHARS - 1).collect();
    format!("{kept}…")
}

// -- the one-turn usage error -----------------------------------------------

/// A near-miss a caller is documented to try, and where the value comes from.
///
/// **Not a rename table**, because there is nothing to rename: every `#[arg(long)]`
/// field in this CLI's git history is `addr`, `bank`, `commits`, `db`,
/// `dry_run`, `guidelines`, `strict`, `transcripts`, `uninstall`, and that is
/// exactly the current set. `docs/CONSISTENCY.md` and `docs/INTEGRATION_GAPS.md`
/// record no renamed or removed flag either — their one row that reads like one
/// (G6.4, "no `--endpoint` flag on the hook path") records a flag this project
/// never had, and its answer is the environment, not a different flag.
///
/// Which is what this table is instead: an agent that has read that row, or that
/// carries agentmemory's `--api-url` over, spends a turn on a flag that cannot
/// work. Both resolve to the one sentence that does.
const NEAR_MISS: &[(&str, &str)] = &[
    (
        "--endpoint",
        "the endpoint comes from $MEMORY_WIRE_URL (default http://127.0.0.1:8888) — there is no --endpoint flag",
    ),
    (
        "--api-url",
        "the endpoint comes from $MEMORY_WIRE_URL (default http://127.0.0.1:8888) — there is no --api-url flag",
    ),
];

/// Report a parse failure and exit, with the fix in the same response.
///
/// The point is that the caller can correct the call without a second turn: the
/// error and the offending subcommand's own concise help land on stdout
/// together, which is the stream it is reading.
///
/// stdout rather than clap's default stderr, and the split is deliberate: `mcp`
/// reserves stdout for JSON-RPC, `--help` and a usage error are the two things a
/// caller asks for on purpose, and an agent capturing only stdout sees clap's
/// default as an empty response it cannot interpret. Tracing stays on stderr.
///
/// Exit 2 for a usage error — unchanged, and what a harness already expects.
/// Exit 0 for `--help` and `--version`, because a help request that reported
/// failure would teach a caller to distrust every invocation.
pub fn usage_failure(err: &clap::Error) -> ! {
    let argv: Vec<String> = std::env::args().collect();
    let requested = matches!(
        err.kind(),
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
    );
    println!("{}", err.render().to_string().trim_end());
    if !requested {
        if let Some(why) = hint(&argv) {
            println!("\nhint: {why}");
        }
        println!("\n{}", concise_help(&argv).trim_end());
    }
    std::process::exit(if requested { 0 } else { 2 })
}

/// The targeted sentence for the first near-miss the caller actually typed.
///
/// Matched against `argv` rather than scraped out of the error text: the error's
/// wording is clap's, not ours, and a table keyed on a substring of someone
/// else's sentence breaks the first time clap rewords an error. A token really
/// being on the command line is a fact we can read directly.
fn hint(argv: &[String]) -> Option<&'static str> {
    argv.iter()
        .skip(1)
        .find_map(|token| NEAR_MISS.iter().find(|(name, _)| name == token))
        .map(|(_, why)| *why)
}

/// The concise help block for the subcommand the caller actually named.
///
/// Resolved by walking `argv` against the live clap tree rather than by reading
/// the error back: the tree is the same one the parse just ran, so a subcommand
/// cannot be spelled here and spelled differently there, and a flag *value*
/// that happens to be a subcommand name (`--bank daemon`) cannot be mistaken for
/// a subcommand — tokens beginning with `-` are skipped, and a token that is not
/// a subcommand of the command reached so far ends the walk.
///
/// The bin name is carried down the walk rather than left to clap's own build,
/// because a subcommand lifted out of its parent renders as `Usage: connect` —
/// a command that does not exist. The caller has to be able to paste the usage
/// line back, so it keeps the full `memory-wire connect [OPTIONS] [AGENT]`.
fn concise_help(argv: &[String]) -> String {
    let mut cmd = Cli::command();
    let mut bin = "memory-wire".to_string();
    for name in named_subcommands(argv) {
        let Some(next) = cmd.find_subcommand(&name).cloned() else {
            break;
        };
        bin = format!("{bin} {name}");
        cmd = next.bin_name(bin.clone());
    }
    cmd.render_help().to_string()
}

/// The longest prefix of `argv` naming nested subcommands.
fn named_subcommands(argv: &[String]) -> Vec<String> {
    let mut path = Vec::new();
    let mut cmd = Cli::command();
    for token in argv.iter().skip(1) {
        if token.starts_with('-') {
            continue;
        }
        let Some(sub) = cmd.find_subcommand(token) else {
            break;
        };
        path.push(token.clone());
        cmd = sub.clone();
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fixed clock, so "2h ago" is a fact about the test and not about when it
    /// ran.
    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-29T12:00:00Z")
            .expect("stamp")
            .with_timezone(&Utc)
    }

    /// A stamp `ago` seconds before [`now`].
    fn ago_stamp(ago: i64) -> String {
        (now() - chrono::Duration::seconds(ago)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    }

    fn view() -> View {
        View {
            bin: Some("~/.local/bin/memory-wire".to_string()),
            endpoint: "http://127.0.0.1:8888".to_string(),
            server: Server::Silent,
            bank: "acme-api".to_string(),
            store: "~/.local/share/memory-wire/memory.db".to_string(),
            store_bytes: 3_022_848,
            store_present: true,
            facts: Facts::Read(BankFacts {
                memories: 41,
                newest: Some(ago_stamp(2 * 3600)),
                tags: vec!["deploy".into(), "auth".into(), "ops".into()],
            }),
        }
    }

    /// The screen the task specifies, field for field.
    #[test]
    fn a_healthy_home_view_should_read_as_the_specified_screen() {
        let text = render(&view(), now());
        assert_eq!(
            text,
            "bin: ~/.local/bin/memory-wire\n\
             description: Agent memory over HTTP and MCP — bank-isolated SQLite + FTS5, \
             no daemon until you start one\n\
             \n\
             server     not running  (endpoint http://127.0.0.1:8888)\n\
             bank       acme-api\n\
             store      ~/.local/share/memory-wire/memory.db  (2.9 MiB)\n\
             memories   41  ·  newest 2h ago\n\
             tags       deploy, auth, ops\n\
             \n\
             help[4]:\n  \
             Run `memory-wire daemon start` to serve on http://127.0.0.1:8888\n  \
             Run `memory-wire doctor` for endpoint, store and server health\n  \
             Run `memory-wire connect` to wire up agent harnesses\n  \
             Run `memory-wire --help` for the full surface\n"
        );
    }

    // AXI §8: the thing printed on a bare invocation has to be state, and a
    // caller that already has a server must not be told to start one.
    #[test]
    fn a_running_server_should_drop_the_start_a_server_step() {
        let mut v = view();
        v.server = Server::Up;
        let text = render(&v, now());
        assert!(text.contains("server     running  (endpoint"), "{text}");
        assert!(!text.contains("daemon start"), "{text}");
        assert!(text.contains("help[3]:"), "{text}");
    }

    // A stranger on the port answers 200. Saying "not running" without saying
    // why sends the caller to `daemon start`, which then fails on the bind.
    #[test]
    fn a_foreign_process_on_the_port_should_be_named_rather_than_hidden() {
        let mut v = view();
        v.server = Server::Foreign;
        let text = render(&v, now());
        assert!(text.contains("another process holds the port"), "{text}");
    }

    // Never-fail, clause by clause: nothing is dropped, nothing is invented, and
    // the exit code is 0 in every one of these.
    #[test]
    fn a_missing_store_should_still_render_a_complete_screen() {
        let mut v = view();
        v.store_present = false;
        v.store_bytes = 0;
        v.facts = Facts::Missing;
        let text = render(&v, now());
        assert!(
            text.contains("store      ~/.local/share/memory-wire/memory.db  (no store yet)"),
            "{text}"
        );
        assert!(
            text.contains("memories   unknown  ·  no store at this path yet"),
            "{text}"
        );
        assert!(
            !text.contains("tags       "),
            "no tags line without a store: {text}"
        );
        assert!(text.contains("help[4]:"), "{text}");
    }

    #[test]
    fn an_unreadable_store_should_say_so_rather_than_report_zero() {
        let mut v = view();
        v.facts = Facts::Unreadable("no such table: memories".to_string());
        let text = render(&v, now());
        assert!(
            text.contains("memories   unknown  ·  no such table: memories"),
            "{text}"
        );
        assert!(
            !text.contains("memories   41"),
            "an unreadable bank is not an empty one: {text}"
        );
    }

    #[test]
    fn an_empty_bank_should_report_zero_and_no_newest() {
        let mut v = view();
        v.facts = Facts::Read(BankFacts::default());
        let text = render(&v, now());
        assert!(text.contains("memories   0\n"), "{text}");
        assert!(!text.contains("newest"), "{text}");
    }

    // An unresolvable executable means no `bin:` line at all — never a path this
    // process is not at.
    #[test]
    fn an_unresolvable_binary_should_omit_the_line() {
        let mut v = view();
        v.bin = None;
        let text = render(&v, now());
        assert!(!text.contains("bin:"), "{text}");
        assert!(text.starts_with("description: "), "{text}");
    }

    #[test]
    fn home_should_be_collapsed_to_a_tilde_and_only_the_home() {
        let home = Path::new("/home/someone");
        let under = |p: &str| tilde(Path::new(p), home);
        assert_eq!(
            under("/home/someone/.local/bin/memory-wire").as_deref(),
            Some("~/.local/bin/memory-wire")
        );
        assert_eq!(under("/home/someone").as_deref(), Some("~"));
        // Not under the home, and only sharing a name prefix with it: no
        // `~`-form, because either would name a path that does not exist.
        assert_eq!(under("/opt/memory-wire"), None);
        assert_eq!(under("/home/someonep/memory-wire"), None);
        // Nothing to collapse against: printed whole rather than mangled. `/opt`
        // is under nobody's home on either platform this ships to.
        assert_eq!(collapse(Path::new("/opt/memory-wire")), "/opt/memory-wire");
    }

    #[test]
    fn ages_should_pick_the_largest_unit_that_reads_true() {
        for (secs, want) in [
            (0, "just now"),
            (59, "just now"),
            (60, "1m ago"),
            (2 * 3600, "2h ago"),
            (3 * 86_400, "3d ago"),
            (90 * 86_400, "3mo ago"),
            (800 * 86_400, "2y ago"),
        ] {
            assert_eq!(ago(&ago_stamp(secs), now()), want, "{secs}s");
        }
        // The store's pre-timestamp sentinel is a real row whose age the row does
        // not carry, and clock skew is clamped rather than reported as negative.
        assert_eq!(ago(PRETIMESTAMP_STAMP, now()), "unknown");
        assert_eq!(ago("not a timestamp", now()), "unknown");
        assert_eq!(ago(&ago_stamp(-86_400), now()), "just now");
    }

    // Every next step has to be a command this binary actually has, spelled
    // exactly as it would be typed. A suggestion that does not run is worse
    // than none: it costs the caller the turn it was meant to save.
    #[test]
    fn every_next_step_should_be_a_command_this_binary_has() {
        let known = [
            "memory-wire daemon start",
            "memory-wire doctor",
            "memory-wire connect",
            "memory-wire --help",
        ];
        for text in [render(&view(), now()), {
            let mut v = view();
            v.server = Server::Up;
            render(&v, now())
        }] {
            let steps: Vec<&str> = text
                .lines()
                .filter_map(|l| l.strip_prefix("  Run `").and_then(|l| l.split('`').next()))
                .collect();
            assert!(!steps.is_empty(), "{text}");
            assert!(
                steps.iter().all(|s| known.contains(s)),
                "a suggestion outside the real surface: {steps:?}"
            );
        }
    }

    // The walk has to stop at a flag's value: `--bank daemon` must not send the
    // caller `daemon`'s help.
    #[test]
    fn a_flag_value_that_looks_like_a_subcommand_should_not_change_the_help() {
        let argv = ["memory-wire", "hook", "--bank", "daemon", "--nope"]
            .iter()
            .map(|s| (*s).to_string())
            .collect::<Vec<_>>();
        assert_eq!(named_subcommands(&argv), vec!["hook".to_string()]);
        let help = concise_help(&argv);
        assert!(help.contains("--bank"), "{help}");
        assert!(!help.contains("daemon start"), "{help}");
    }

    // …and it has to descend when the name really is a subcommand, because
    // `daemon` on its own lists no flags and `daemon start` does.
    #[test]
    fn a_real_subcommand_should_get_its_own_help_block() {
        let argv = ["memory-wire", "daemon", "start", "--nope"]
            .iter()
            .map(|s| (*s).to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            named_subcommands(&argv),
            vec!["daemon".to_string(), "start".to_string()]
        );
        assert!(
            concise_help(&argv).contains("--addr"),
            "{}",
            concise_help(&argv)
        );
    }

    #[test]
    fn a_near_miss_should_be_answered_only_when_it_was_typed() {
        let typed: Vec<String> = ["memory-wire", "hook", "--endpoint", "x"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        assert!(
            hint(&typed).unwrap().contains("MEMORY_WIRE_URL"),
            "{typed:?}"
        );
        let other: Vec<String> = ["memory-wire", "hook", "--bank", "x"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        assert_eq!(hint(&other), None);
    }

    #[test]
    fn a_drivers_complaint_should_fit_on_one_line() {
        assert_eq!(short("one line"), "one line");
        assert_eq!(short("first\nsecond"), "first");
        assert_eq!(short(&"x".repeat(200)).chars().count(), WHY_CHARS);
    }

    // The end-to-end contracts, driven against the real binary so the argv, the
    // exit code and the stream are all the shipped ones.
    fn run(args: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
        let mut cmd = std::process::Command::new(crate::bin());
        cmd.args(args).env_remove("MEMORY_WIRE_URL");
        for (k, v) in env {
            cmd.env(k, v);
        }
        let out = cmd.output().expect("run memory-wire");
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }

    /// A scratch home, so the view reports on a store nobody else owns.
    fn scratch(tag: &str) -> (PathBuf, Vec<(String, String)>) {
        let dir = std::env::temp_dir().join(format!("mw-home-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join(".local/share/memory-wire")).expect("tmp");
        let env = vec![
            ("HOME".to_string(), dir.display().to_string()),
            (
                "XDG_DATA_HOME".to_string(),
                dir.join(".local/share").display().to_string(),
            ),
            ("USERPROFILE".to_string(), dir.display().to_string()),
        ];
        (dir, env)
    }

    // The AXI §8 contract: a bare invocation exits 0 and prints state, even with
    // no store anywhere near the path it resolves to.
    #[test]
    fn a_bare_invocation_should_print_live_state_and_exit_zero() {
        let (dir, env) = scratch("bare");
        let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let (code, out, _err) = run(&[], &env);
        assert_eq!(code, 0, "a view that fails is worse than the manual: {out}");
        for field in [
            "bin: ",
            "description: ",
            "server  ",
            "bank  ",
            "store  ",
            "memories  ",
            "help[",
        ] {
            assert!(out.contains(field), "missing {field:?} in\n{out}");
        }
        assert!(out.contains("(no store yet)"), "{out}");
        assert!(out.contains("unknown  ·  no store"), "{out}");
        // No absolute home path anywhere: the `~` form is the contract.
        assert!(!out.contains(&dir.display().to_string()), "{out}");
        std::fs::remove_dir_all(&dir).ok();
    }

    // The AXI §6 contract, end to end: one call in, the error and the valid
    // flags out, exit 2.
    #[test]
    fn a_bad_flag_should_be_fixable_from_the_error_alone() {
        let (code, out, err) = run(&["connect", "--stat", "foo"], &[]);
        assert_eq!(code, 2, "{out}{err}");
        assert!(out.contains("unexpected argument '--stat'"), "{out}");
        // The whole point: the flags it could have used are in this response.
        for flag in ["--uninstall", "--guidelines", "[AGENT]"] {
            assert!(out.contains(flag), "missing {flag} in\n{out}");
        }
        // Pasted back verbatim, the usage line has to be the command that exists.
        assert!(
            out.contains("Usage: memory-wire connect [OPTIONS] [AGENT]"),
            "{out}"
        );
        assert!(err.is_empty(), "the agent reads stdout: {err}");
    }

    #[test]
    fn a_bad_flag_on_a_nested_subcommand_should_get_that_subcommands_help() {
        let (code, out, _err) = run(&["daemon", "start", "--nope"], &[]);
        assert_eq!(code, 2, "{out}");
        assert!(out.contains("--addr"), "{out}");
        assert!(out.contains("--db"), "{out}");
    }

    #[test]
    fn an_unknown_subcommand_should_get_the_top_level_surface() {
        let (code, out, _err) = run(&["bogus"], &[]);
        assert_eq!(code, 2, "{out}");
        for cmd in ["doctor", "connect", "daemon", "sweep"] {
            assert!(out.contains(cmd), "missing {cmd} in\n{out}");
        }
    }

    // A help request is not a failure, and the route map that used to print on a
    // bare invocation has to still exist somewhere — on `--help`.
    #[test]
    fn help_should_still_work_and_still_carry_the_route_map() {
        for args in [vec!["--help"], vec!["-h"]] {
            let (code, out, _err) = run(&args, &[]);
            assert_eq!(code, 0, "{args:?}: {out}");
            if args == ["--help"] {
                for route in ["/health", "/banks/:id/stats", "/memories", "MCP"] {
                    assert!(out.contains(route), "missing {route} in\n{out}");
                }
            }
        }
        let (code, out, _err) = run(&["connect", "--help"], &[]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("--uninstall"), "{out}");
    }

    #[test]
    fn a_near_miss_flag_should_be_answered_in_the_same_response() {
        let (code, out, _err) = run(&["hook", "--endpoint", "http://x"], &[]);
        assert_eq!(code, 2, "{out}");
        assert!(out.contains("hint: "), "{out}");
        assert!(out.contains("MEMORY_WIRE_URL"), "{out}");
    }
}
