//! `doctor`: one screen that says whether memory-wire is actually usable, and
//! nothing else. No telemetry, no remote calls beyond the local health probe.

use std::path::{Path, PathBuf};

use crate::paths;
use crate::{http, paths::IO_TIMEOUT};

/// Health of the on-disk store.
#[derive(Debug, PartialEq, Eq)]
pub enum StoreState {
    /// Opened; counts available.
    Ok,
    /// No database file yet — the server has never run.
    Missing,
    /// Present but unreadable or unmigrated.
    Unreadable(String),
}

impl StoreState {
    /// One word plus optional detail.
    pub fn label(&self) -> String {
        match self {
            StoreState::Ok => "ok".to_string(),
            StoreState::Missing => "missing".to_string(),
            StoreState::Unreadable(why) => format!("unreadable: {why}"),
        }
    }
}

/// Health of the HTTP server.
#[derive(Debug, PartialEq, Eq)]
pub enum ServerState {
    /// `/health` answered 2xx.
    Up,
    /// Unreachable or unhealthy; the reason is the transport error or status.
    Down(String),
}

impl ServerState {
    /// One word plus optional detail.
    pub fn label(&self) -> String {
        match self {
            ServerState::Up => "up".to_string(),
            ServerState::Down(why) => format!("down: {why}"),
        }
    }
}

/// Everything `doctor` reports.
#[derive(Debug)]
pub struct Report {
    /// Endpoint probed.
    pub endpoint: String,
    /// Bank a hook in this directory would resolve to.
    pub bank: String,
    /// Database path.
    pub store: PathBuf,
    /// Database size in bytes (0 when missing).
    pub store_bytes: u64,
    /// Store health.
    pub store_state: StoreState,
    /// Bank rows, when readable.
    pub banks: Option<i64>,
    /// Memory rows, when readable.
    pub memories: Option<i64>,
    /// Server health.
    pub server: ServerState,
    /// The bank a pre-0.4.0 build derived here, when it differs from `bank` and
    /// that bank actually **holds memories**. Carries the memories a user
    /// upgrading this build would otherwise appear to have lost.
    pub legacy_bank: Option<String>,
    /// The bank that does hold memories, when the resolved one holds none.
    ///
    /// `doctor` prints `banks N` and `memories M` side by side and never joined
    /// them, so a store with 710 memories in it and a hook resolving to an empty
    /// bank looked exactly like a working install that had nothing to say.
    pub elsewhere: Option<(String, i64)>,
}

impl Report {
    /// The screen.
    pub fn render(&self) -> String {
        let count = |v: Option<i64>| v.map_or_else(|| "-".to_string(), |n| n.to_string());
        let mut out = format!(
            "memory-wire doctor\n  \
endpoint   {}\n  \
bank       {}\n  \
store      {}  ({}, {})\n  \
banks      {}\n  \
memories   {}\n  \
server     {}",
            self.endpoint,
            self.bank,
            self.store.display(),
            paths::human_bytes(self.store_bytes),
            self.store_state.label(),
            count(self.banks),
            count(self.memories),
            self.server.label()
        );
        if let Some((other, n)) = &self.elsewhere {
            out.push_str(&format!(
                "\n  \
finding    bank `{}` holds no memories; `{}` holds {n}.\n  \
           A hook here resolves to `{}` and will recall nothing. Reach them with\n  \
           `--bank {other}`, or set MEMORY_WIRE_BANK={other}.",
                self.bank, other, self.bank
            ));
        }
        if let Some(legacy) = &self.legacy_bank {
            out.push_str(&format!(
                "\n  \
warning    this directory now resolves to bank `{}`, but a bank `{}` already \
exists here\n  \
           and is where memories written before the switch were kept. Nothing \
was moved:\n  \
           keep using `--bank {legacy}` (or set MEMORY_WIRE_BANK={legacy}) to \
reach them,\n  \
           or read them back out of bank `{legacy}` and retain them into `{0}` \
yourself.\n  \
           See docs/BANK_IDENTITY.md.",
                self.bank, legacy
            ));
        }
        out
    }

    /// The reason `--strict` should exit nonzero, if any.
    pub fn strict_failure(&self) -> Option<String> {
        match (&self.server, &self.store_state) {
            (ServerState::Down(why), _) => Some(format!("server unreachable: {why}")),
            (_, StoreState::Missing) => Some(format!("no store at {}", self.store.display())),
            (_, StoreState::Unreadable(why)) => Some(format!("store unreadable: {why}")),
            _ => None,
        }
    }
}

/// Collect a report for the ambient environment.
pub fn collect() -> Report {
    collect_at(
        &paths::endpoint(),
        &paths::resolve_bank(),
        &memory_wire::store::default_db_path(),
    )
}

/// Collect a report for explicit inputs.
pub fn collect_at(endpoint: &str, bank: &str, store: &Path) -> Report {
    // Fold the WAL in first so the reported size is the store as it will be
    // copied, not the store plus a sidecar a `cp` would miss.
    fold_wal(store);
    let store_bytes = store_bytes(store);
    let (store_state, banks, memories) = match counts(store) {
        Some(Ok((b, m))) => (StoreState::Ok, Some(b), Some(m)),
        // Opened, but a page does not check out: the row counts below it would
        // be read off a store nobody should trust, so they are withheld.
        Some(Err(why)) => (StoreState::Unreadable(why), None, None),
        None if !store.exists() => (StoreState::Missing, None, None),
        None => (
            StoreState::Unreadable("no banks table (store not migrated?)".to_string()),
            None,
            None,
        ),
    };
    let server = probe_server(endpoint);
    let per_bank = bank_memory_counts(store);
    Report {
        endpoint: endpoint.to_string(),
        bank: bank.to_string(),
        store: store.to_path_buf(),
        store_bytes,
        store_state,
        banks,
        memories,
        server,
        legacy_bank: legacy_bank_at(&per_bank, bank),
        elsewhere: elsewhere_at(&per_bank, bank),
    }
}

/// Ask `endpoint` whether a memory-wire server is answering, at the grade that
/// counts: `/health` must answer **2xx** *and* a body of exactly `ok`.
///
/// A 2xx alone is not proof — any process on this port answers 200 — so the
/// body is what separates our server from a foreign one squatting on the port.
/// `doctor` and `daemon` both call this rather than each carrying a copy, so
/// "is the server up" cannot mean two different things in one binary; that
/// divergence is the bug `a_foreign_two_hundred_should_not_count_as_the_server`
/// is named for.
pub fn probe_server(endpoint: &str) -> ServerState {
    match http::get(&format!("{endpoint}/health"), IO_TIMEOUT) {
        Ok(r) if !r.ok() => ServerState::Down(format!("GET {endpoint}/health -> {}", r.status)),
        Ok(r) if r.body.trim() == "ok" => ServerState::Up,
        Ok(_) => ServerState::Down("unexpected health response".to_string()),
        Err(e) => ServerState::Down(e.to_string()),
    }
}

/// `(bank_id, memory count)` per bank, read-only, largest first.
///
/// One query, because two findings need it and the store may hold many banks.
/// Empty when the file is missing, unreadable, or has no memories at all — every
/// caller treats that as "nothing to report", never as an error.
fn bank_memory_counts(store: &Path) -> Vec<(String, i64)> {
    if !store.exists() {
        return Vec::new();
    }
    let Ok(conn) = rusqlite::Connection::open_with_flags(
        store,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    ) else {
        return Vec::new();
    };
    let Ok(mut stmt) = conn.prepare(
        "SELECT bank_id, COUNT(*) FROM memories GROUP BY bank_id ORDER BY COUNT(*) DESC, bank_id",
    ) else {
        return Vec::new();
    };
    stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
}

/// The bank this directory used to resolve to, when that differs from the bank it
/// resolves to now **and** that bank still holds memories.
///
/// Read-only, like everything else here: it answers a question rather than
/// repairing anything. Both conditions matter. A different name with no memories
/// behind it is not an orphaned memory, it is just a fresh project; and an old
/// bank that happens to still be the name this directory resolves to is not a
/// migration at all.
///
/// **Memories, not a `banks` row.** The row is the bug this replaces. A `banks`
/// row is created on connect and by `retain`, so its presence says the bank was
/// named at some point — not that anything was ever written. Counting rows in
/// `banks` made this fire on a bank holding nothing and then tell the user their
/// memories "were kept" there. Measured against a real store: the legacy-named
/// bank had 0 rows while 710 memories sat in a third bank, so the warning named
/// the wrong bank and promised a history that had not happened.
fn legacy_bank_at(per_bank: &[(String, i64)], bank: &str) -> Option<String> {
    let legacy = paths::legacy_bank_in(&std::env::current_dir().ok()?);
    if legacy == bank {
        return None;
    }
    per_bank
        .iter()
        .any(|(id, n)| id == &legacy && *n > 0)
        .then_some(legacy)
}

/// The bank that actually holds memories, when the resolved one holds none.
///
/// The finding that answers "where is my corpus?" — a store with 710 memories in
/// it and a hook resolving to an empty bank is a working install that recalls
/// nothing, and nothing on the screen said so. Silent when the resolved bank has
/// memories, and silent when no bank has any, because then there is nothing to
/// point at.
fn elsewhere_at(per_bank: &[(String, i64)], bank: &str) -> Option<(String, i64)> {
    if per_bank.iter().any(|(id, n)| id == bank && *n > 0) {
        return None;
    }
    per_bank.iter().find(|(_, n)| *n > 0).cloned()
}

/// On-disk size of the store, SQLite sidecars included.
///
/// A WAL database keeps uncommitted pages in `memory.db-wal` and a shared-memory
/// index in `memory.db-shm`, so the main file alone under-reports what the store
/// actually occupies — the larger the bank, the larger the gap. Every file is
/// measured, never created: `doctor` reports on a store, it does not bring one
/// into existence.
fn store_bytes(store: &Path) -> u64 {
    [None, Some("wal"), Some("shm")]
        .iter()
        .map(|suffix| {
            let mut name = store.as_os_str().to_os_string();
            if let Some(suffix) = suffix {
                name.push(format!("-{suffix}"));
            }
            std::fs::metadata(PathBuf::from(name)).map(|m| m.len()).unwrap_or(0)
        })
        .sum()
}

/// `(banks, memories)` read-only; `None` when the file is missing or unusable.
///
/// Read-only on purpose: `doctor` must not create the database it is reporting
/// on, and a WAL database that is mid-write still reads. `PRAGMA integrity_check`
/// rides the same handle — it writes nothing, and a store with a damaged page
/// passes a `COUNT(*)` while handing back nonsense, so the check runs before the
/// counts and its verdict is the report. `Err` means "opened, but not sound".
fn counts(store: &Path) -> Option<Result<(i64, i64), String>> {
    if !store.exists() {
        return None;
    }
    let conn = rusqlite::Connection::open_with_flags(
        store,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .ok()?;
    if let Err(why) = integrity(&conn) {
        return Some(Err(why));
    }
    let banks = conn
        .query_row("SELECT COUNT(*) FROM banks", [], |r| r.get::<_, i64>(0))
        .ok()?;
    let memories = conn
        .query_row("SELECT COUNT(*) FROM memories", [], |r| r.get::<_, i64>(0))
        .ok()?;
    Some(Ok((banks, memories)))
}

/// `PRAGMA integrity_check`, verbatim, as `Ok(())` or the driver's own complaint.
///
/// The first row is `ok` on a sound store and the offending page on a damaged
/// one, so nothing is summarised away: the text `doctor` prints is the text
/// SQLite produced. Costs one pass over the file, which is why it lives here and
/// not on every request.
fn integrity(conn: &rusqlite::Connection) -> Result<(), String> {
    let verdict: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if verdict == "ok" {
        Ok(())
    } else {
        Err(format!("integrity_check: {}", first_line(&verdict)))
    }
}

/// The first line of a multi-line SQLite complaint, so the screen stays a screen.
fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or(text).trim()
}

/// Fold the WAL into the main file, best effort.
///
/// The one write `doctor` performs, and it is why a plain `cp` of the store
/// taken right after a `doctor` run is a usable backup. `PASSIVE` never blocks:
/// if a live server holds the write lock, or the file is not a WAL database at
/// all, the statement is a no-op and the error is dropped — a checkpoint that
/// did not run is not a health finding. Skipped entirely when the file is
/// absent, so pointing `--db` at a nonexistent path still creates nothing.
fn fold_wal(store: &Path) {
    if !store.exists() {
        return;
    }
    if let Ok(conn) = rusqlite::Connection::open(store) {
        let _ = conn.busy_timeout(std::time::Duration::from_millis(500));
        let _ = conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE)");
    }
}

/// Print the report; return the process exit code.
pub fn run(strict: bool) -> i32 {
    let report = collect();
    println!("{}", report.render());
    match (strict, report.strict_failure()) {
        (true, Some(why)) => {
            eprintln!("memory-wire doctor: {why}");
            1
        }
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory_wire::api::MemoryService;
    use memory_wire::memory::Bank;
    use memory_wire::store::{SqliteStore, Store};

    fn bank(id: &str) -> Bank {
        Bank { id: id.into(), name: id.into() }
    }

    /// A loopback server that answers one request with a fixed status and body —
    /// the shape of a foreign process squatting on the memory-wire port.
    fn foreign(status: &'static str, body: &'static str) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        std::thread::spawn(move || {
            let Ok((mut sock, _)) = listener.accept() else { return };
            let mut raw = [0u8; 1024];
            let _ = sock.read(&mut raw);
            let _ = sock.write_all(
                format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\n\r\n{body}", body.len())
                    .as_bytes(),
            );
        });
        format!("http://{addr}")
    }

    // The bug this fixes: a 2xx is not a health check. Anything else listening
    // on 8888 answers 200, and `doctor` used to call that "up".
    #[test]
    fn a_foreign_two_hundred_should_not_count_as_the_server() {
        let ep = foreign("200 OK", "<html>some other service</html>");
        let dir = std::env::temp_dir().join(format!("mw-doctor-foreign-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("tmp");
        let store = dir.join("memory.db");
        let svc = MemoryService::new(SqliteStore::open(&store).expect("open"));
        svc.store.put_bank(&bank("demo")).expect("bank");
        svc.retain("demo", "auth uses jose", None).expect("retain");

        let r = collect_at(&ep, "demo", &store);
        assert_eq!(
            r.server,
            ServerState::Down("unexpected health response".to_string()),
            "{r:?}"
        );
        assert!(r.strict_failure().unwrap().contains("unexpected health response"));
        std::fs::remove_dir_all(&dir).ok();
    }

    // The real server's body is `ok`, so it still reports up.
    #[test]
    fn a_body_of_ok_should_still_count_as_the_server() {
        let ep = foreign("200 OK", "ok");
        let r = collect_at(&ep, "demo", Path::new("/nonexistent/memory.db"));
        assert_eq!(r.server, ServerState::Up, "{r:?}");
    }

    // A WAL store spends a good part of its bytes outside the main file, so the
    // report adds the `-wal` and `-shm` sidecars in — and creates none of them.
    #[test]
    fn store_bytes_should_include_the_sqlite_sidecars() {
        let dir = std::env::temp_dir().join(format!("mw-doctor-bytes-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("tmp");
        let store = dir.join("memory.db");
        let sidecars = ["memory.db", "memory.db-wal", "memory.db-shm"];
        let present = |names: &[&str; 3]| names.iter().map(|n| dir.join(n).exists()).collect::<Vec<_>>();
        assert_eq!(store_bytes(&store), 0, "a missing store measures zero");

        let svc = MemoryService::new(SqliteStore::open(&store).expect("open"));
        svc.store.put_bank(&bank("b")).expect("bank");
        svc.retain("b", "auth uses jose", None).expect("retain");

        let before = present(&sidecars);
        let total = store_bytes(&store);
        let files: u64 = sidecars
            .iter()
            .map(|n| std::fs::metadata(dir.join(n)).map(|m| m.len()).unwrap_or(0))
            .sum();
        assert!(total >= files, "{total} vs {files} across {sidecars:?}");
        assert!(total > 0, "{total}");
        assert_eq!(before, present(&sidecars), "measuring must not touch the file set");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn collect_at_should_count_a_real_store() {
        let dir = std::env::temp_dir().join(format!("mw-doctor-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("tmp");
        let store = dir.join("memory.db");
        let svc = MemoryService::new(SqliteStore::open(&store).expect("open"));
        for id in ["proj", "other"] {
            svc.store.put_bank(&bank(id)).expect("bank");
        }
        svc.retain("proj", "auth uses jose", None).expect("retain");
        svc.retain("proj", "rate limiting via token bucket", None).expect("retain");

        let r = collect_at("http://127.0.0.1:1", "proj", &store);
        assert_eq!(r.banks, Some(2));
        assert_eq!(r.memories, Some(2));
        assert_eq!(r.store_state, StoreState::Ok);
        assert!(r.store_bytes > 0);
        assert!(matches!(r.server, ServerState::Down(_)));
        assert!(r.strict_failure().is_some(), "server down fails strict");

        let text = r.render();
        assert!(text.starts_with("memory-wire doctor\n"), "{text}");
        for line in [
            "  endpoint   http://127.0.0.1:1",
            "  bank       proj",
            "  banks      2",
            "  memories   2",
        ] {
            assert!(text.contains(line), "missing {line:?} in\n{text}");
        }
        assert!(text.contains("  server     down: "), "{text}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn collect_at_should_report_a_missing_store_without_creating_it() {
        let dir = std::env::temp_dir().join(format!("mw-doctor-missing-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("tmp");
        let store = dir.join("memory.db");
        let r = collect_at("http://127.0.0.1:1", "demo", &store);
        assert_eq!(r.store_state, StoreState::Missing);
        assert_eq!(r.banks, None);
        assert_eq!(r.memories, None);
        assert_eq!(r.store_bytes, 0);
        assert!(r.render().contains("banks      -"), "{}", r.render());
        assert!(r.strict_failure().is_some());
        assert!(!store.exists(), "doctor must not create the store");
        std::fs::remove_dir_all(&dir).ok();
    }

    // The failure this is for: a store whose pages do not check out still
    // answers `COUNT(*)`, so a healthy-looking report on a rotten bank is worse
    // than no report. The header is left intact — it has to open for the
    // complaint to be anything other than "unreadable" — and a page past it is
    // overwritten, which is what a bad sector looks like from SQLite's side.
    #[test]
    fn a_store_with_a_damaged_page_should_fail_strict() {
        let dir = std::env::temp_dir().join(format!("mw-doctor-corrupt-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("tmp");
        let store = dir.join("memory.db");
        {
            let svc = MemoryService::new(SqliteStore::open(&store).expect("open"));
            svc.store.put_bank(&bank("demo")).expect("bank");
            for n in 0..40 {
                svc.retain("demo", &format!("memory number {n} of the corrupt fixture"), None)
                    .expect("retain");
            }
        } // closed, so the WAL is folded and the file is a single file to damage

        let len = std::fs::metadata(&store).expect("stat").len();
        assert!(len > 4096 * 2, "fixture too small to damage: {len}");
        let mut raw = std::fs::read(&store).expect("read");
        raw[4096..4608].fill(0xFF);
        std::fs::write(&store, &raw).expect("write");

        // A healthy server, so the report reaches the store branch: a down server
        // is reported first and would mask what this test is about.
        let r = collect_at(&foreign("200 OK", "ok"), "demo", &store);
        let StoreState::Unreadable(why) = &r.store_state else {
            panic!("a damaged page must not read as healthy: {r:?}");
        };
        assert!(why.contains("integrity_check"), "{why}");
        let strict = r.strict_failure().expect("a damaged store must fail --strict");
        assert!(strict.contains("store unreadable"), "{strict}");
        assert!(r.render().contains("unreadable: integrity_check"), "{}", r.render());
        // Counts off a store nobody should trust are withheld, not reported.
        assert_eq!(r.banks, None);
        assert_eq!(r.memories, None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn render_should_align_every_field() {
        let r = Report {
            endpoint: "http://127.0.0.1:8888".to_string(),
            bank: "demo".to_string(),
            store: PathBuf::from("/tmp/memory.db"),
            store_bytes: 2048,
            store_state: StoreState::Ok,
            banks: Some(3),
            memories: Some(42),
            server: ServerState::Up,
            legacy_bank: None,
            elsewhere: None,
        };
        assert_eq!(
            r.render(),
            "memory-wire doctor\n  \
endpoint   http://127.0.0.1:8888\n  \
bank       demo\n  \
store      /tmp/memory.db  (2.0 KiB, ok)\n  \
banks      3\n  \
memories   42\n  \
server     up"
        );
        assert!(r.strict_failure().is_none());

        // Each failure mode on its own: the server is reported first, so the
        // store branch needs a healthy server to be reachable in the report.
        let mut only_store = r;
        only_store.server = ServerState::Up;
        only_store.store_state = StoreState::Missing;
        assert!(only_store.strict_failure().unwrap().contains("no store at"));
        let mut unreadable = only_store;
        unreadable.store_state = StoreState::Unreadable("no banks table".to_string());
        assert!(unreadable.strict_failure().unwrap().contains("unreadable"));
    }

    /// A directory whose bank is now derived from its git remote, where the
    /// basename bank still exists: exactly the upgrade case, and the only one
    /// worth a warning. `collect_at` is pointed at a store that holds the old
    /// bank, and the ambient cwd is this repository — whose remote derives
    /// `memory-wire` while its basename is also `memory-wire`, so the two
    /// collide. A temp directory is used for the *store* and the legacy name is
    /// checked against the real cwd, which is why this asserts on the helper
    /// rather than trying to fake a repository.
    #[test]
    fn a_legacy_bank_under_a_different_name_should_warn_and_change_nothing() {
        let dir = std::env::temp_dir().join(format!("mw-doctor-legacy-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("tmp");
        let store = dir.join("memory.db");
        let legacy = paths::legacy_bank_in(&std::env::current_dir().expect("cwd"));
        let svc = MemoryService::new(SqliteStore::open(&store).expect("open"));
        svc.store.put_bank(&bank(&legacy)).expect("bank");
        svc.retain(&legacy, "auth uses jose", None).expect("retain");

        // A different current bank, so the rename is the thing being reported.
        let r = collect_at("http://127.0.0.1:1", "derived-name", &store);
        assert_eq!(r.legacy_bank.as_deref(), Some(legacy.as_str()), "{r:?}");
        let text = r.render();
        assert!(text.contains("warning"), "{text}");
        assert!(text.contains("derived-name"), "{text}");
        assert!(text.contains(&format!("`{legacy}`")), "{text}");
        assert!(text.contains("--bank"), "must name the way out: {text}");
        assert!(text.contains("MEMORY_WIRE_BANK"), "{text}");

        // Read-only: the warning is a report, not a repair. Both banks stand.
        let after = MemoryService::new(SqliteStore::open(&store).expect("open"));
        assert_eq!(
            after
                .bank_stats("derived-name")
                .expect("new bank")
                .memories,
            0,
            "doctor must not move memories"
        );
        assert_eq!(after.bank_stats(&legacy).expect("legacy bank").memories, 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// No legacy bank in the store, and the resolved name unchanged: nothing to
    /// warn about, so the screen stays a screen.
    #[test]
    fn no_legacy_bank_should_leave_the_screen_quiet() {
        let dir = std::env::temp_dir().join(format!("mw-doctor-nowarn-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("tmp");
        let store = dir.join("memory.db");
        let svc = MemoryService::new(SqliteStore::open(&store).expect("open"));
        svc.store.put_bank(&bank("unrelated")).expect("bank");

        let r = collect_at("http://127.0.0.1:1", "derived-name", &store);
        assert_eq!(r.legacy_bank, None, "{r:?}");
        assert!(!r.render().contains("warning"), "{}", r.render());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The regression for the bug this finding was measured against.
    ///
    /// A `banks` row is created on connect and by `retain`, so the old check
    /// fired on the row's existence and then told the user their memories "were
    /// kept" in a bank that held none. Measured on a real store: the
    /// legacy-named bank had 0 rows while 710 memories sat in a third bank, so
    /// the warning named the wrong bank and promised a history that never
    /// happened.
    #[test]
    fn a_legacy_bank_row_with_no_memories_should_stay_quiet() {
        let dir = std::env::temp_dir().join(format!("mw-doctor-emptylegacy-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("tmp");
        let store = dir.join("memory.db");
        let legacy = paths::legacy_bank_in(&std::env::current_dir().expect("cwd"));
        let svc = MemoryService::new(SqliteStore::open(&store).expect("open"));
        // The bank exists as a row, and holds nothing.
        svc.store.put_bank(&bank(&legacy)).expect("bank");

        let r = collect_at("http://127.0.0.1:1", "derived-name", &store);
        assert_eq!(r.legacy_bank, None, "an empty bank is not an orphan: {r:?}");
        assert!(!r.render().contains("warning"), "{}", r.render());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A store holding memories and a hook resolving to an empty bank is a working
    /// install that recalls nothing, and nothing on the screen said so. This is
    /// the finding that answers "where is my corpus?".
    #[test]
    fn an_empty_resolved_bank_should_point_at_the_bank_that_has_the_memories() {
        let dir = std::env::temp_dir().join(format!("mw-doctor-elsewhere-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("tmp");
        let store = dir.join("memory.db");
        let svc = MemoryService::new(SqliteStore::open(&store).expect("open"));
        svc.store.put_bank(&bank("omp")).expect("bank");
        for i in 0..3 {
            svc.retain("omp", &format!("corpus memory {i}"), None).expect("retain");
        }

        let r = collect_at("http://127.0.0.1:1", "ishan-parihar-memory-wire", &store);
        assert_eq!(r.elsewhere, Some(("omp".to_string(), 3)), "{r:?}");
        let text = r.render();
        assert!(text.contains("holds no memories"), "{text}");
        assert!(text.contains("`omp` holds 3"), "{text}");
        // Must name both ways out, or the finding is a complaint with no exit.
        assert!(text.contains("--bank omp"), "{text}");
        assert!(text.contains("MEMORY_WIRE_BANK=omp"), "{text}");

        // Read-only, like every other finding here.
        let after = MemoryService::new(SqliteStore::open(&store).expect("open"));
        assert_eq!(
            after.bank_stats("ishan-parihar-memory-wire").expect("new bank").memories,
            0,
            "doctor must not move memories"
        );
        assert_eq!(after.bank_stats("omp").expect("omp").memories, 3);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The finding is only for a bank that is actually empty. A resolved bank with
    /// memories in it is the normal case and must not be second-guessed.
    #[test]
    fn a_resolved_bank_with_memories_should_not_be_second_guessed() {
        let dir = std::env::temp_dir().join(format!("mw-doctor-notempty-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("tmp");
        let store = dir.join("memory.db");
        let svc = MemoryService::new(SqliteStore::open(&store).expect("open"));
        svc.retain("this-repo", "auth uses jose", None).expect("retain");
        svc.retain("other-repo", "something else entirely", None).expect("retain");

        let r = collect_at("http://127.0.0.1:1", "this-repo", &store);
        assert_eq!(r.elsewhere, None, "{r:?}");
        assert!(!r.render().contains("finding"), "{}", r.render());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// With no memories anywhere there is nothing to point at, so a fresh install
    /// is not greeted by a finding about a corpus it does not have.
    #[test]
    fn an_empty_store_should_not_report_a_missing_corpus() {
        let dir = std::env::temp_dir().join(format!("mw-doctor-nostore-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("tmp");
        let store = dir.join("memory.db");
        let svc = MemoryService::new(SqliteStore::open(&store).expect("open"));
        svc.store.put_bank(&bank("fresh")).expect("bank");

        let r = collect_at("http://127.0.0.1:1", "fresh", &store);
        assert_eq!(r.elsewhere, None, "{r:?}");
        assert!(!r.render().contains("finding"), "{}", r.render());
        std::fs::remove_dir_all(&dir).ok();
    }
}
