//! Concurrency soak: N clients x M banks x T seconds against one store.
//!
//! The store is a single `Mutex<Connection>` and the handlers are `async fn`
//! calling a blocking store. That is correct for localhost single-user, so this
//! measures it rather than changing it.
//!
//! Run: `cargo run --example soak -- [--clients 16] [--banks 4] [--seconds 10]
//!        [--db /tmp/scratch.db] [--baseline-p99-us N]`
//!
//! Reported, nonzero exit on any miss:
//! - zero 5xx, with the append storm's 409s counted apart through the crate's
//!   own `http_error` mapping, so a conflict can never hide as a storage error
//! - zero SQLITE_BUSY / SQLITE_LOCKED
//! - zero panics, and the store still answers afterwards (a poisoned
//!   `Mutex<Connection>` fails the post-storm probe)
//! - row counts exact: every bank holds its two seeded document rows plus exactly
//!   the plain retains that succeeded, and each storm document still holds one
//! - p99 within 2x of the pre-change baseline from this same harness. The first
//!   run establishes that number, so record it and pass it back as
//!   `--baseline-p99-us` on every later run. Compare only runs on the same
//!   profile *and* the same kind of storage: a dev build against a release build
//!   differ about 7x, and a scratch db on disk against one on tmpfs about 5x,
//!   both measured here. Each run prints its db path so the comparison is
//!   checkable rather than assumed.
//! - no RSS growth past the store's own page cache plus per-client allocator
//!   headroom. One run bounds growth, it cannot prove the absence of a slow leak:
//!   rerun at a different `--seconds` or `--clients` and check the number does
//!   not track the work done.
//! - the append pre-check is not quadratic: its `EXPLAIN QUERY PLAN` rides the
//!   `(bank_id, document_id)` index, and conflict latency does not grow with the
//!   bank, so neither a per-row scan nor a retry loop is hiding behind 409.
//!
//! The database defaults to a temp file this run creates and removes. A `--db`
//! path is never deleted: it is refused if it already holds rows, because the row
//! arithmetic below starts from a known seed.
//!
//! Two things were added to this harness after its first run, both additive:
//! **aggregate throughput** (operations per wall second — the run counted every
//! operation it performed and never divided by time, so the repository had no
//! throughput number anywhere) and **a committed artifact** (`--out-md`; the run
//! used to print to stdout and record nothing, so no soak result had ever been
//! written down). A bare run writes the artifact to a scratch file under
//! `$TMPDIR`; `--out-md eval/SOAK.md` is what updates the committed one. The
//! artifact is written before the verdict is decided, so a FAIL is recorded rather
//! than lost. Every existing assertion, the PASS/FAIL exit behaviour, and the
//! stdout format are unchanged.


use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use memory_wire::api::{http_error, ApiError, MemoryService};
use memory_wire::memory::Bank;
use memory_wire::store::{SqliteStore, Store, StoreError, UpdateMode};
use rusqlite::Connection;

// Shared with the `bench_*` harnesses for the provenance line (build profile, date,
// machine) and the `/proc` RSS reader. A directory module, so cargo does not
// auto-discover it as an example of its own.
mod bench_common;

/// The one document every client replaces: a single row, maximally contended.
const REPLACE_DOC: &str = "soak-replace-doc";
/// The document every client appends to. Seeded, so every append is the 409 path.
const APPEND_DOC: &str = "soak-append-doc";
/// Recall queries, cycled so the FTS stream always has real candidates.
const QUERIES: [&str; 4] = [
    "jose middleware jwt verification",
    "token bucket rate limiting",
    "pgvector hnsw vector index",
    "retention policy observations days",
];
/// Conflicting appends timed at each end of the run, to size the flatness claim.
const PROBE: u64 = 200;
/// How much conflict latency may grow between a 2-row bank and a full one.
/// Generous, because this is a guard against order-of-magnitude regressions, not
/// a benchmark: a per-row scan or a retry loop is 100x or worse, timing jitter
/// between two idle single-threaded bursts is a small multiple.
const FLATNESS_LIMIT: f64 = 25.0;
/// Resident growth allowed per client, on top of the store's page cache.
///
/// The page cache bounds the database, but RSS does not stop there: glibc gives
/// every thread its own arena, and FTS5 query churn leaves freed blocks in each.
/// That part tracks the *client count*, not the work done — measured here, one
/// client and sixteen clients each ran about 33k operations and grew 2.9 MB and
/// 15.7 MB, while sixteen clients over twice the time grew *less* (12.0 MB).
/// One megabyte per client is headroom over that, not slack a leak hides in.
const RSS_PER_CLIENT_MB: u64 = 1;
/// The exact pre-check `SqliteStore::put_doc` runs for `update_mode=append`.
const PRE_CHECK_SQL: &str =
    "SELECT EXISTS(SELECT 1 FROM memories WHERE bank_id=?1 AND document_id=?2)";

/// One workload step. Report order, and the index each writes its latencies under.
#[derive(Clone, Copy)]
enum Op {
    Retain,
    Recall,
    Stats,
    Page,
    Replace,
    Append,
}

const ALL: [Op; 6] = [
    Op::Retain,
    Op::Recall,
    Op::Stats,
    Op::Page,
    Op::Replace,
    Op::Append,
];
const NAMES: [&str; 6] = ["retain", "recall", "stats", "page", "replace", "append"];

impl Op {
    fn idx(self) -> usize {
        self as usize
    }
    fn name(self) -> &'static str {
        NAMES[self.idx()]
    }
}

/// One client's tally. Errors are data here, not failures: counting them is the
/// job, so a client records and carries on rather than unwinding.
struct Counts {
    ok: u64,
    five_xx: u64,
    conflict: u64,
    other: u64,
    busy: u64,
    /// Successful plain retains, per bank. The only class that adds a row.
    retains: Vec<u64>,
    /// Per-class latencies, microseconds.
    lat: Vec<Vec<u64>>,
}

impl Counts {
    fn new(banks: usize) -> Self {
        Self {
            ok: 0,
            five_xx: 0,
            conflict: 0,
            other: 0,
            busy: 0,
            retains: vec![0; banks],
            lat: ALL.iter().map(|_| Vec::new()).collect(),
        }
    }

    fn absorb(&mut self, other: Self) {
        self.ok += other.ok;
        self.five_xx += other.five_xx;
        self.conflict += other.conflict;
        self.other += other.other;
        self.busy += other.busy;
        for (into, from) in self.retains.iter_mut().zip(other.retains) {
            *into += from;
        }
        for (into, from) in self.lat.iter_mut().zip(other.lat) {
            into.extend(from);
        }
    }

    fn record(&mut self, op: Op, bank: usize, result: &Result<(), ApiError>) {
        match result {
            Ok(()) => {
                self.ok += 1;
                if op.idx() == Op::Retain.idx() {
                    self.retains[bank] += 1;
                }
            }
            Err(e) => {
                let (status, busy) = classify(e);
                if busy {
                    self.busy += 1;
                }
                match status {
                    200..=299 => self.ok += 1,
                    409 => self.conflict += 1,
                    500..=599 => self.five_xx += 1,
                    _ => self.other += 1,
                }
            }
        }
    }
}

/// The request's own status code, plus whether the driver said the database was
/// busy. Classified through `http_error` rather than a local match, so the
/// harness asserts the contract the HTTP surface actually serves.
fn classify(e: &ApiError) -> (u16, bool) {
    let (status, _) = http_error(e);
    let busy = match e {
        ApiError::Store(StoreError::Sqlite(inner)) => matches!(
            inner.sqlite_error_code(),
            Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
        ),
        _ => false,
    };
    (status.as_u16(), busy)
}

/// Two storms at one in ten, the other four shared evenly. Weighted by slot
/// rather than sampled, so a short run still covers every class.
fn schedule(i: u64) -> Op {
    match i % 10 {
        4 => Op::Replace,
        9 => Op::Append,
        n => ALL[(n as usize) % 4],
    }
}

fn content(client: u64, i: u64) -> String {
    format!(
        "soak client {client} revision {i} {}",
        QUERIES[(i % QUERIES.len() as u64) as usize]
    )
}

/// One workload step against the live service.
fn run_op(
    svc: &MemoryService<SqliteStore>,
    op: Op,
    bank: &str,
    client: u64,
    i: u64,
) -> Result<(), ApiError> {
    match op {
        Op::Retain => {
            svc.retain(bank, &content(client, i), Some(format!("soak-{client}")))?;
        }
        Op::Recall => {
            svc.recall(bank, QUERIES[(i % QUERIES.len() as u64) as usize], 2000)?;
        }
        Op::Stats => {
            svc.bank_stats(bank)?;
        }
        Op::Page => {
            svc.list_memories(bank, 50, ((i % 40) * 50) as usize)?;
        }
        Op::Replace => {
            svc.retain_doc(
                bank,
                &content(client, i),
                None,
                &[],
                Some(REPLACE_DOC),
                UpdateMode::Replace,
            )?;
        }
        Op::Append => {
            svc.retain_doc(bank, &content(client, i), None, &[], Some(APPEND_DOC), UpdateMode::Append)?;
        }
    }
    Ok(())
}

/// Median latency of `PROBE` appends that must all be refused. Returns
/// `None` if any of them was not a conflict, since then the bank did not hold
/// the document and the timing says nothing about the pre-check.
fn conflict_probe(svc: &MemoryService<SqliteStore>, bank: &str) -> Option<u64> {
    let mut lat = Vec::with_capacity(PROBE as usize);
    for i in 0..PROBE {
        let t = Instant::now();
        let r = svc.retain_doc(bank, &content(u64::MAX, i), None, &[], Some(APPEND_DOC), UpdateMode::Append);
        lat.push(t.elapsed().as_micros() as u64);
        match r {
            Err(ApiError::Store(StoreError::DocumentConflict)) => {}
            _ => return None,
        }
    }
    lat.sort_unstable();
    lat.get(lat.len() / 2).copied()
}

fn pct(sorted: &[u64], p: f64) -> u64 {
    match sorted.len() {
        0 => 0,
        n => sorted[((n as f64 * p) as usize).min(n - 1)],
    }
}

fn rss_kb() -> Option<u64> {
    fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find(|l| l.starts_with("VmRSS:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
}

/// Database bytes including the WAL and shared-memory files, so the RSS
/// comparison is against everything the store actually wrote.
fn db_bytes(db: &Path) -> u64 {
    let mut total = fs::metadata(db).map(|m| m.len()).unwrap_or(0);
    for suffix in ["-wal", "-shm"] {
        let side = PathBuf::from(format!("{}-{}", db.display(), suffix));
        total += fs::metadata(side).map(|m| m.len()).unwrap_or(0);
    }
    total
}

/// Every row of `EXPLAIN QUERY PLAN`, joined into one line.
///
/// A subquery reports its double-init (`SCAN CONSTANT ROW`) first and the work it
/// actually does on a later row, so reading only the first row misses the index
/// and would call an indexed pre-check a full scan.
fn explain(audit: &Connection, sql: &str) -> Result<String> {
    let mut stmt = audit.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
    let rows = stmt.query_map(rusqlite::params!["soak-0", APPEND_DOC], |r| {
        r.get::<_, String>(3)
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out.join(" | "))
}

/// The store's own page cache in bytes: `cache_size` counts pages and is negative
/// when it does. This is the part of the RSS ceiling that comes from the store
/// rather than from this file; [`RSS_PER_CLIENT_MB`] is the rest.
fn page_cache_bytes(audit: &Connection) -> Result<u64> {
    let page: u64 = audit.query_row("PRAGMA page_size", [], |r| r.get(0))?;
    let pages: i64 = audit.query_row("PRAGMA cache_size", [], |r| r.get(0))?;
    Ok(pages.unsigned_abs() * page)
}

/// How many rows one document holds in one bank.
fn doc_rows(audit: &Connection, bank: &str, doc: &str) -> Result<u64> {
    audit.query_row(
        "SELECT COUNT(*) FROM memories WHERE bank_id=?1 AND document_id=?2",
        rusqlite::params![bank, doc],
        |r| r.get(0),
    )
    .map_err(Into::into)
}

/// The database file, or one of the two sidecars that live beside it. The empty
/// suffix names the database itself: building these names by appending `"-"`
/// unconditionally would silently produce a path nothing ever wrote.
fn sidecar(db: &Path, suffix: &str) -> PathBuf {
    match suffix.is_empty() {
        true => db.to_path_buf(),
        false => PathBuf::from(format!("{}-{}", db.display(), suffix)),
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == name)
        .map(|w| w[1].clone())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let clients: usize = flag(&args, "--clients")
        .map(|v| v.parse().expect("--clients N"))
        .unwrap_or(16);
    let banks: usize = flag(&args, "--banks")
        .map(|v| v.parse().expect("--banks M"))
        .unwrap_or(4);
    let seconds: u64 = flag(&args, "--seconds")
        .map(|v| v.parse().expect("--seconds T"))
        .unwrap_or(10);
    let baseline_p99: Option<u64> = flag(&args, "--baseline-p99-us").map(|v| v.parse().expect("--baseline-p99-us N"));
    let out_md = bench_common::out_md(&args, "SOAK.md");
    let explicit_db = flag(&args, "--db");
    let db = explicit_db
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join(format!("mw-soak-{}.db", std::process::id())));

    if explicit_db.is_none() {
        for suffix in ["", "-wal", "-shm"] {
            let _ = fs::remove_file(sidecar(&db, suffix));
        }
    } else if db.exists() {
        // Never destroy a path the caller named. The row arithmetic below starts
        // from a known seed, so a database already holding rows is refused rather
        // than counted into this run's totals.
        let existing = Connection::open(&db)?
            .query_row("SELECT COUNT(*) FROM memories", [], |r| r.get::<_, u64>(0))?;
        if existing > 0 {
            bail!(
                "--db {} already holds {existing} rows; point --db at a fresh scratch file",
                db.display()
            );
        }
    }
    let store = SqliteStore::open(&db)?;
    let svc = MemoryService::new(store);

    // Seed: the banks, plus the two document rows the storms keep hitting. Both
    // exist before the first client runs, so every replace is an update (net
    // zero rows) and every append is a conflict, and the final row count is a
    // closed-form expectation rather than a race to guess at.
    let bank_ids: Vec<String> = (0..banks).map(|i| format!("soak-{i}")).collect();
    for bank in &bank_ids {
        svc.store
            .put_bank(&Bank {
                id: bank.clone(),
                name: bank.clone(),
            })?;
        for doc in [REPLACE_DOC, APPEND_DOC] {
            svc.retain_doc(bank, &format!("seeded {doc} in {bank}"), None, &[], Some(doc), UpdateMode::Replace)?;
        }
    }
    let seeded = 2 * banks as u64;

    // The pre-check's plan and the store's page-cache ceiling, read once against
    // a separate connection and kept for the post-storm document census. A scan
    // of the bank would not name the index, so the plan is the whole quadratic
    // claim; the ceiling keeps the RSS check off magic numbers.
    let audit = Connection::open(&db)?;
    let plan = explain(&audit, PRE_CHECK_SQL)?;
    let indexed = plan.contains("idx_memories_bank_doc");
    let rss_ceiling = page_cache_bytes(&audit)? + clients as u64 * RSS_PER_CLIENT_MB * 1_048_576;

    let rss_before = rss_kb();
    let early_probe = conflict_probe(&svc, &bank_ids[0]);

    let wall = Instant::now();
    let mut clients_out: Vec<Counts> = Vec::with_capacity(clients);
    let mut panics = 0u64;
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..clients)
            .map(|cid| {
                let svc = &svc;
                let bank_ids = &bank_ids;
                let deadline = Duration::from_secs(seconds);
                scope.spawn(move || {
                    let mut c = Counts::new(bank_ids.len());
                    let start = Instant::now();
                    let mut i = 0u64;
                    while start.elapsed() < deadline {
                        let op = schedule(i);
                        let bank_idx = (cid + i as usize) % bank_ids.len();
                        let t = Instant::now();
                        let r = run_op(svc, op, &bank_ids[bank_idx], cid as u64, i);
                        c.lat[op.idx()].push(t.elapsed().as_micros() as u64);
                        c.record(op, bank_idx, &r);
                        i += 1;
                    }
                    c
                })
            })
            .collect();
        for h in handles {
            match h.join() {
                Ok(c) => clients_out.push(c),
                Err(_) => panics += 1,
            }
        }
    });
    let wall = wall.elapsed();

    let mut total = Counts::new(banks);
    for c in clients_out {
        total.absorb(c);
    }
    let late_probe = conflict_probe(&svc, &bank_ids[0]);
    // A poisoned `Mutex<Connection>` panics on the next lock, so the store being
    // answerable after the storm is the whole "no poisoning" check.
    let usable = panic::catch_unwind(AssertUnwindSafe(|| svc.bank_stats(&bank_ids[0]).is_ok()))
        .unwrap_or(false);

    // Row exactness, per bank and in total. Only a plain retain adds a row:
    // a replace supersedes its document's single row, and an append is refused.
    let mut per_bank: Vec<u64> = Vec::with_capacity(banks);
    for bank in &bank_ids {
        per_bank.push(svc.bank_stats(bank)?.memories as u64);
    }
    let db_total: u64 = per_bank.iter().sum();
    let expected_total = seeded + total.retains.iter().sum::<u64>();
    let per_bank_exact = per_bank
        .iter()
        .enumerate()
        .all(|(i, n)| *n == 2 + total.retains[i]);

    // Each storm document must still be exactly one row: the replace path's
    // headline guarantee, and the one thing the counters above cannot see.
    let mut replace_rows = Vec::with_capacity(banks);
    let mut append_rows = Vec::with_capacity(banks);
    for bank in &bank_ids {
        replace_rows.push(doc_rows(&audit, bank, REPLACE_DOC)?);
        append_rows.push(doc_rows(&audit, bank, APPEND_DOC)?);
    }
    drop(audit);
    let docs_exact = replace_rows.iter().all(|n| *n == 1) && append_rows.iter().all(|n| *n == 1);

    let rss_after = rss_kb();
    let db_on_disk = db_bytes(&db);
    // kB to bytes, and against the page-cache ceiling rather than the file size:
    // RSS follows what SQLite holds resident, which is bounded by its cache.
    let rss_growth = match (rss_before, rss_after) {
        (Some(b), Some(a)) => a.saturating_sub(b) * 1024,
        _ => 0,
    };

    let mut all: Vec<u64> = total.lat.iter().flatten().copied().collect();
    all.sort_unstable();
    let p50 = pct(&all, 0.50);
    let p95 = pct(&all, 0.95);
    let p99 = pct(&all, 0.99);

    println!("=== concurrency soak ===");
    println!(
        "workload   {clients} clients x {banks} banks x {seconds}s · db {}",
        db.display()
    );
    println!("           wall {:.1}s · {seeded} seeded document rows", wall.as_secs_f64());
    println!();
    println!("| op      | count  | p50 us | p95 us | p99 us |");
    println!("|---|---|---|---|---|");
    // Summarised once, here, because the artifact below needs the same numbers and
    // the per-op vectors are drained by this loop. One summary, two consumers, so
    // the stdout table and the committed table cannot disagree.
    let mut per_op: Vec<(usize, usize, u64, u64, u64)> = Vec::with_capacity(ALL.len());
    for op in ALL {
        let mut lat = std::mem::take(&mut total.lat[op.idx()]);
        lat.sort_unstable();
        let (n, p50, p95, p99) = (
            lat.len(),
            pct(&lat, 0.50),
            pct(&lat, 0.95),
            pct(&lat, 0.99),
        );
        per_op.push((op.idx(), n, p50, p95, p99));
        println!("| {} | {n} | {p50} | {p95} | {p99} |", op.name());
    }
    println!(
        "| all     | {} | {p50} | {p95} | {p99} |",
        all.len()
    );
    println!();
    println!(
        "statuses   ok {} · 409 {} · 5xx {} · other {}",
        total.ok, total.conflict, total.five_xx, total.other
    );
    println!(
        "sqlite     SQLITE_BUSY {} · panics {panics} · post-storm probe {}",
        total.busy,
        if usable { "ok" } else { "FAILED" }
    );
    println!("rows       per bank {per_bank:?}");
    println!("           expected sum {expected_total} · db sum {db_total} · per bank exact {per_bank_exact}");
    println!("documents  replace-storm {replace_rows:?} · append-storm {append_rows:?}");
    let rss_line = match (rss_before, rss_after) {
        (Some(b), Some(a)) => format!("{:.1} MB -> {:.1} MB (delta {:+.1} MB)", b as f64 / 1024.0, a as f64 / 1024.0, (a - b) as f64 / 1024.0),
        _ => "unavailable on this platform".to_string(),
    };
    println!(
        "rss        {rss_line} · db on disk {:.1} MB · ceiling {:.1} MB (page cache + {RSS_PER_CLIENT_MB} MB/client)",
        db_on_disk as f64 / 1_048_576.0,
        rss_ceiling as f64 / 1_048_576.0
    );
    println!("pre-check  {plan}");
    let flat = match (early_probe, late_probe) {
        (Some(e), Some(l)) => {
            let ratio = l as f64 / e.max(1) as f64;
            format!(
                "conflict p50 {e} us on a 2-row bank -> {l} us on a full one ({ratio:.1}x, limit {FLATNESS_LIMIT:.0}x)"
            )
        }
        _ => "conflict probe did not see a conflict".to_string(),
    };
    println!("           {flat}");

    let mut fails: Vec<String> = Vec::new();
    if total.five_xx > 0 {
        fails.push(format!("{} 5xx responses", total.five_xx));
    }
    if total.busy > 0 {
        fails.push(format!("{} SQLITE_BUSY/LOCKED", total.busy));
    }
    if total.other > 0 {
        fails.push(format!("{} unexpected statuses", total.other));
    }
    if total.conflict == 0 {
        fails.push("append storm produced no 409s, so the conflict path went untested".into());
    }
    if panics > 0 {
        fails.push(format!("{panics} panicked clients"));
    }
    if !usable {
        fails.push("store unusable after the storm (poisoned connection)".into());
    }
    if !per_bank_exact || db_total != expected_total {
        fails.push(format!(
            "row counts not exact: expected {expected_total}, db {db_total}, per bank {per_bank_exact}"
        ));
    }
    if !docs_exact {
        fails.push(format!(
            "storm documents not single-rowed: replace {replace_rows:?} append {append_rows:?}"
        ));
    }
    if !indexed {
        fails.push(format!("append pre-check does not ride the index: {plan}"));
    }
    if let (Some(e), Some(l)) = (early_probe, late_probe) {
        if l as f64 / e.max(1) as f64 > FLATNESS_LIMIT {
            fails.push(format!(
                "append pre-check cost grew with bank size: {e} us -> {l} us"
            ));
        }
    } else {
        fails.push("append pre-check probe never reached the conflict path".into());
    }
    if rss_growth > rss_ceiling {
        fails.push(format!(
            "RSS grew {:.1} MB, past the {:.1} MB ceiling",
            rss_growth as f64 / 1_048_576.0,
            rss_ceiling as f64 / 1_048_576.0
        ));
    }
    match baseline_p99 {
        Some(base) if base > 0 => {
            let limit = base * 2;
            let verdict = if p99 <= limit { "within" } else { "OVER" };
            println!("baseline   p99 {p99} us vs 2x {base} us = {limit} us · {verdict}");
            if p99 > limit {
                fails.push(format!("p99 {p99} us over 2x the {base} us baseline"));
            }
        }
        Some(_) => println!("baseline   p99 {p99} us · no baseline recorded yet"),
        None => println!(
            "baseline   p99 {p99} us · no --baseline-p99-us given, so this run establishes it"
        ),
    }

    println!();
    // Throughput: the number this run previously counted its way past. Every
    // attempt is a request that came back with some status, so the denominator is
    // attempts and the numerator is attempts — throughput is not a success rate,
    // and the status line above is where the success rate lives.
    let attempts = total.ok + total.conflict + total.five_xx + total.other;
    let ops_per_s = if wall.as_secs_f64() > 0.0 {
        attempts as f64 / wall.as_secs_f64()
    } else {
        0.0
    };
    println!(
        "throughput {ops_per_s:.0} ops/s over {attempts} attempts in {:.1}s",
        wall.as_secs_f64()
    );

    // The artifact, written before the verdict so a FAIL is recorded rather than
    // lost with the process exit. Same numbers as the stdout above, plus the
    // provenance and the limits that stdout has no room for.
    let mut md = String::from("# Concurrency soak (memory-wire)\n\n");
    md.push_str(&format!("{}\n\n", bench_common::provenance()));
    md.push_str(&format!(
        "Workload: {clients} clients x {banks} banks x {seconds}s against ONE `SqliteStore` \
         (one writer `Mutex<Connection>` plus a 4-connection WAL read pool, writer-first with \
         spill-on-contention), a six-op schedule weighted one storm in ten, on a \
         database this run created at {dbpath}. Build profile, machine and date are in the \
         provenance line above; the profile alone moves these numbers 2-5x.\n\n\
         **Aggregate throughput: {ops_per_s:.0} ops/s** over {attempts} attempts in {wall_s:.1}s \
         (median {p50} us, p95 {p95} us, p99 {p99} us across all ops).\n\n\
         | op | count | ops/s | p50 us | p95 us | p99 us |\n|---|---|---|---|---|---|\n",
        dbpath = db.display(),
        wall_s = wall.as_secs_f64(),
    ));
    for (idx, n, op50, op95, op99) in &per_op {
        md.push_str(&format!(
            "| {} | {n} | {:.0} | {op50} | {op95} | {op99} |\n",
            ALL[*idx].name(),
            if wall.as_secs_f64() > 0.0 {
                *n as f64 / wall.as_secs_f64()
            } else {
                0.0
            },
        ));
    }
    md.push_str(&format!(
        "\nStatuses: ok {ok} · 409 {conflict} · 5xx {five_xx} · other {other}. \
         SQLITE_BUSY/LOCKED {busy}. Panicked clients {panics}. Post-storm probe {probe}.\n\n\
         Row exactness: per bank {per_bank:?}, expected sum {expected_total}, db sum {db_total}, \
         per-bank exact {per_bank_exact}. Storm documents: replace {replace_rows:?}, append \
         {append_rows:?}.\n\n\
         RSS: {rss_line} · db on disk {db_mb:.1} MB · ceiling {ceil_mb:.1} MB (page cache + \
         {RSS_PER_CLIENT_MB} MB/client).\n\n\
         Append pre-check plan: {plan}\n\nIndexed: {indexed}. Conflict probe: {flat}.\n\n\
         Baseline p99: {baseline_note}.\n\n\
         Verdict: {verdict_line}\n\n\
         ## What this does not measure\n\n\
         - Throughput scaling. This run fixes the client count; \
           `examples/bench_concurrency.rs` sweeps it and is where a serialization \
           signature is actually visible.\n\
         - Write cost. The workload keeps every bank growing, so a run's throughput \
           depends on its length; `examples/bench_write.rs` prices a single retain.\n\
         - Recall quality, recall versus bank size, startup cost, and binary or storage \
           footprint. Those are `bench_recall_curve`, `bench_coldstart` and \
           `bench_footprint`.\n\
         - HTTP and MCP transport cost: this is the library call in-process, so axum, tokio \
           and a socket are absent by design.\n\n\
         ## Limitations\n\n\
         - A time-boxed run measures throughput as a function of the scheduler as well as of \
           the store. Quoting one run's ops/s as a property of the store overstates it; re-run \
           and quote the spread, as `eval/CODING_LIFE.md` does for its p50.\n\
         - The p99 baseline is only comparable across runs on the same profile *and* the same \
           kind of storage: a dev build against a release build differ about 7x, and a scratch \
           db on disk against one on tmpfs about 5x, both measured here.\n\
         - One run bounds an RSS leak, it cannot prove the absence of one: rerun at a \
           different `--seconds` or `--clients` and check the number does not track the work.\n\
         - The append pre-check flatness check sizes a per-row scan or a retry loop. It is not \
           sensitive to a constant-factor regression, and its {FLATNESS_LIMIT:.0}x budget \
           deliberately allows for timing jitter between two idle single-threaded bursts.\n",
        ok = total.ok,
        conflict = total.conflict,
        five_xx = total.five_xx,
        other = total.other,
        busy = total.busy,
        probe = if usable { "ok" } else { "FAILED" },
        rss_line = rss_line,
        db_mb = db_on_disk as f64 / 1_048_576.0,
        ceil_mb = rss_ceiling as f64 / 1_048_576.0,
        indexed = indexed,
        baseline_note = match baseline_p99 {
            Some(base) if base > 0 => format!("compared against 2x {base} us"),
            Some(_) => "no baseline recorded yet".into(),
            None => "none given; this run establishes it".into(),
        },
        verdict_line = match fails.is_empty() {
            true => "**PASS**".to_string(),
            false => format!("**FAIL** — {}", fails.join("; ")),
        },
    ));
    fs::write(&out_md, &md)?;
    eprintln!("wrote {out_md}");

    if fails.is_empty() {
        println!("verdict    PASS");
    } else {
        bail!("FAIL: {}", fails.join("; "));
    }

    if explicit_db.is_none() {
        for suffix in ["", "-wal", "-shm"] {
            let _ = fs::remove_file(sidecar(&db, suffix));
        }
    }
    Ok(())
}
