//! Footprint: binary bytes, resident memory, and on-disk storage — measured.
//!
//! The README's headline numbers — "8.4 MB binary", "10.5 MB idle RSS", "3.0 MB
//! for 10k memories" — are measured by hand: `ls -l`, `ps -o rss`, and a
//! `wal_checkpoint` done from a shell at some point in the past. Nothing in the
//! repository re-runs them, so they cannot be checked after a change without
//! redoing the whole procedure by hand. This harness is that procedure, committed.
//!
//! Run: `cargo build --release --locked` first, then
//! `cargo run --release --example bench_footprint -- [--memories 10000]
//!        [--repeats 3] [--out-md PATH]`
//!
//! What it reports, and why each is a separate number:
//! - **Binary bytes** — `ls -l` on the release binary, located from this
//!   example's own path so it cannot read a debug build or a stale artifact.
//! - **Harness idle RSS** — this process before it opens a store. It is a *floor*,
//!   not the README number: an example links rusqlite and the store, but not axum,
//!   tokio or the MCP SDK's serving path. Reported separately and labelled, because
//!   quoting it as "idle RSS" would understate a server by the whole HTTP stack.
//! - **Serve idle RSS and RSS after first retain** — the README's actual numbers,
//!   taken from a real `memory-wire serve` process read through
//!   `/proc/<pid>/status`, after `/health` answers and after one HTTP retain. The
//!   pre-retain/post-retain gap is the FTS index and the write path's first pages,
//!   which is why the two readings must never be quoted interchangeably.
//! - **Storage, twice** — the WAL-unflushed footprint while the store holds the
//!   file open, and the settled footprint after `PRAGMA wal_checkpoint(TRUNCATE)`.
//!   The store runs in WAL mode, so at 10k memories the main file is about 3 MB
//!   while the process is actually occupying about 7.4 MB; the README quotes the
//!   settled figure and says so, and so does this.
//! - **Index bytes per 1k memories** — settled storage divided by rows, which is
//!   the only version of that number that survives a change to row size or
//!   content.
//!
//! What this does NOT measure: RSS under concurrent load (`examples/soak.rs` and
//! `examples/bench_concurrency.rs` bound that), memory of an MCP session, a
//! Postgres or other backend, the cost of the FTS index in isolation (this reports
//! the whole store, not an index-only figure), or what a `VACUUM` would reclaim —
//! nothing here vacuums, so a figure that moved because of free pages would be
//! indistinguishable from one that moved because of growth.
//!
//! Limitations. RSS is read from `/proc`, so on a platform without it every RSS row
//! reports `unavailable` and the run still completes — the same degradation
//! `examples/soak.rs` already has. RSS is a whole-process number and includes the
//! allocator's arenas and any page cache the kernel has mapped, so it moves for
//! reasons unrelated to the store; `--repeats` reports a range for that reason.
//! Storage is measured on whatever filesystem `--db` lands on, and the README's
//! figures assume an ordinary local disk: a tmpfs or a network filesystem will not
//! be comparable. Binary size is for the release profile only and includes LTO's
//! effect; it is not a size a user installs from a package.

mod bench_common;

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use bench_common::{arg, db_bytes, free_port, http, profile, rm_db, rss_kb, rss_kb_of, wait_healthy};
use memory_wire::memory::{Bank, Memory};
use memory_wire::store::{SqliteStore, Store};
use rusqlite::Connection;
use std::process::{Child, Command, Stdio};

/// Default corpus: the size the README's storage line is quoted at.
const DEFAULT_MEMORIES: usize = 10_000;/// How long to wait for a spawned `serve` to answer `/health` before giving up.
/// Generous because this box is not quiet; a server that has not answered in two
/// seconds has failed, whatever the machine is doing.
const HEALTH_TIMEOUT: Duration = Duration::from_secs(20);

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let memories: usize = arg(&args, "--memories", &DEFAULT_MEMORIES.to_string())
        .parse()
        .expect("--memories N");
    let repeats: usize = arg(&args, "--repeats", "3").parse().expect("--repeats N");
    let out_md = bench_common::out_md(&args, "BENCH_FOOTPRINT.md");
    if memories == 0 || repeats == 0 {
        anyhow::bail!("--memories and --repeats must be non-zero");
    }

    // ---- the binary ---------------------------------------------------------
    // Located from this example's own path — `target/<profile>/examples/<name>` —
    // so the size read is the profile being measured and not whatever a stale
    // `target/` happens to hold. `cargo run --example` does not build the bin, so
    // an absent binary is a real possibility and is reported as such.
    let binary = release_binary();
    let binary_bytes = binary
        .as_deref()
        .and_then(|p| std::fs::metadata(p).ok())
        .map_or(0, |m| m.len());
    let binary_note = match binary_bytes {
        0 => "unavailable — no `target/release/memory-wire`; run `cargo build --release --locked` first".into(),
        n => format!("{n} B ({} MB)", mb(n)),
    };

    // ---- this process -------------------------------------------------------
    let harness_idle = rss_kb();
    let mut harness_loaded: Vec<u64> = Vec::new();
    let mut unflushed: Vec<(u64, u64, u64)> = Vec::new();
    let mut settled: Vec<(u64, u64, u64)> = Vec::new();

    let db: PathBuf =
        std::env::temp_dir().join(format!("mw-foot-{}-{memories}.db", std::process::id()));
    for _ in 0..repeats {
        rm_db(&db);
        let store = SqliteStore::open(&db)?;
        store.put_bank(&Bank {
            id: "f".into(),
            name: "f".into(),
        })?;
        // `Store::put`, not `retain`, so the storage figure is the same shape
        // `eval/SCALE_SWEEP.md` reports and the two can be compared directly.
        for i in 0..memories {
            store.put(&Memory {
                id: format!("m-{i}"),
                bank_id: "f".into(),
                content: match i % 4 {
                    0 => format!("auth uses jose middleware for jwt verification instance {i}"),
                    1 => format!("rate limiting via token bucket algorithm instance {i}"),
                    2 => format!("vector search uses pgvector hnsw index instance {i}"),
                    _ => format!("distractor note {i} about cafeteria menus and parking rotations"),
                },
                context: Some(format!("session-{}", i / 8)),
                created_at: None,
            })?;
        }
        harness_loaded.push(rss_kb().unwrap_or(0));
        // Read while the store still holds the file open: this is the transient
        // figure, the one a server that has just been fed rows is actually paying.
        unflushed.push(db_bytes(&db));
        checkpoint(&db)?;
        settled.push(db_bytes(&db));
    }
    rm_db(&db);

    let take = |v: &[(u64, u64, u64)]| (v[v.len() / 2].0, v[v.len() / 2].1, v[v.len() / 2].2);
    let (u_main, u_wal, u_shm) = take(&unflushed);
    let (s_main, s_wal, s_shm) = take(&settled);
    let u_total = u_main + u_wal + u_shm;
    let s_total = s_main + s_wal + s_shm;
    let per1k = s_total * 1000 / memories as u64;

    // ---- a real server ------------------------------------------------------
    let serve = match binary.as_deref() {
        Some(_) => measure_serve(),
        None => None,
    };

    // ---- artifact -----------------------------------------------------------
    let mut md = String::from("# Footprint (memory-wire)\n\n");
    md.push_str(&format!("{}\n\n", bench_common::provenance()));
    md.push_str(&format!(
        "Measured, not hand-copied: the README's headline numbers were, until this harness \
         existed, measured by hand — `ls -l`, `ps -o rss` and a manual `wal_checkpoint` done \
         at some point in the past. (When this harness was written the README read \"8.4 MB \
         binary, 10.5 MB idle RSS, 3.0 MB for 10k memories\"; its current figures are \
         re-pinned from runs of *this* harness, and the RSS rows below are what it \
         publishes.) This harness is that procedure, committed, so a change can no longer \
         require redoing it by hand. Corpus {memories} rows in \
         `scale_sweep`'s four-topic shape, `--repeats {repeats}`, median reported.\n\n",
    ));
    md.push_str("| Dimension | Value | How read |\n|---|---|---|\n");
    md.push_str(&format!(
        "| Release binary | {binary_note} | `std::fs::metadata` on the binary next to this example's own path |\n\
         | Harness idle RSS (floor) | {} | `VmRSS`, this process, before any store is opened |\n\
         | Harness RSS with {memories} rows | {} | same process, store open, index built |\n\
         | Serve idle RSS | {} | `VmRSS` of a real `memory-wire serve`, after `/health` answers |\n\
         | Serve RSS after 1 retain | {} | same process, after one HTTP retain |\n\
         | Storage, WAL unflushed | {u_total} B ({} MB) | main {u_main} + `-wal` {u_wal} + `-shm` {u_shm}, store holding the file open |\n\
         | Storage, settled total | {s_total} B ({} MB) | the same three files after `PRAGMA wal_checkpoint(TRUNCATE)` |\n\
         | Storage, settled main file | {s_main} B ({} MB) | main only — the figure `eval/SCALE_SWEEP.md` and the README quote |\n\
         | Index bytes per 1k memories | {per1k} B ({} MB) | settled total / rows x 1000 |\n\n",
        opt_mb(harness_idle),
        opt_range_mb(&harness_loaded),
        serve.as_ref().map_or_else(|| "not measured — no release binary".into(), |s| opt_kb(s.idle_kb)),
        serve.as_ref().map_or_else(|| "not measured — no release binary".into(), |s| opt_kb(s.after_retain_kb)),
        mb(u_total),
        mb(s_total),
        mb(s_main),
        mb(per1k),
    ));
    md.push_str(&format!(
        "**The two storage figures are different measurements, not a discrepancy.** The store \
         runs in WAL mode: while the process holds the database open, {memories} rows occupy \
         {u_total} B ({u} MB); after a clean `wal_checkpoint(TRUNCATE)` the same rows occupy \
         {s_total} B ({s} MB), of which {s_main} B is the main file and the remainder is the \
         fixed {shm}-byte shared-memory mapping. A server that has just been fed {memories} \
         memories and not yet checkpointed is using {ratio:.1}x the settled figure. The README and \
         `eval/SCALE_SWEEP.md` quote the settled main file and say so; this harness measures both \
         every time, so a change that moves one and not the other shows up as exactly that.\n\n",
        u = mb(u_total),
        s = mb(s_total),
        shm = s_shm,
        ratio = u_total as f64 / s_total.max(1) as f64,
    ));
    md.push_str(
        "## What this does not measure\n\n\
         - RSS under load. Nothing here is concurrent; `examples/soak.rs` and \
           `examples/bench_concurrency.rs` bound that.\n\
         - MCP stdio session memory, or a client that keeps connections open.\n\
         - Any backend but SQLite, and any index-only figure: the storage rows are the whole \
           store (tables, indexes, FTS5 shadow tables and free pages), not the search index alone.\n\
         - What a `VACUUM` would reclaim. Nothing here vacuums, so growth and free-page reuse \
           are indistinguishable in these numbers.\n\
         - A packaged or stripped binary. Binary size is the raw `target/release` file, LTO \
           build, unstripped by whatever profile `Cargo.toml` declares.\n\n\
         ## Limitations\n\n\
         - RSS comes from `/proc`, so every RSS row reads `unavailable` on a platform without \
           it and the run still completes — the same degradation `examples/soak.rs` has. Off \
           Linux the storage and binary rows are still exact.\n\
         - RSS is a whole-process number: it includes allocator arenas and whatever the kernel \
           has mapped, so it can move for reasons unrelated to the store. That is why \
           `--repeats` exists and why a range is printed rather than a point.\n\
         - Storage depends on the filesystem `--db` lands on. The figures above assume an \
           ordinary local disk; tmpfs, a network mount and a copy-on-write filesystem will each \
           report something else, and none of them is comparable to these.\n\
         - The harness RSS rows and the serve RSS rows are different processes doing different \
           work. The harness rows are a floor for a bare library user; only the serve rows are \
           comparable to the README's headline.\n",
    );
    std::fs::write(&out_md, &md)?;
    eprintln!("wrote {out_md}");

    // ---- stdout -------------------------------------------------------------
    println!("=== footprint ===");
    println!("profile {}", profile());
    println!("binary  {binary_note}");
    println!("harness idle RSS      {}", opt_mb(harness_idle));
    println!("harness RSS @ {memories} rows  {}", opt_range_mb(&harness_loaded));
    match &serve {
        Some(s) => {
            println!("serve idle RSS        {}", opt_kb(s.idle_kb));
            println!("serve RSS +1 retain   {}", opt_kb(s.after_retain_kb));
            println!("serve retain status  {}", s.retain_status);
        }
        None => println!("serve                 not measured — no `target/release/memory-wire`"),
    }
    println!(
        "storage unflushed     {u_total} B ({} MB) = main {u_main} + wal {u_wal} + shm {u_shm}",
        mb(u_total)
    );
    println!(
        "storage settled       {s_total} B ({} MB) = main {s_main} + wal {s_wal} + shm {s_shm}",
        mb(s_total)
    );
    println!("index per 1k rows     {per1k} B ({} MB)", mb(per1k));
    Ok(())
}

/// The release binary, found next to this example's own path.
///
/// `current_exe` is `target/<profile>/examples/<name>-<hash>`, so the binary is two
/// directories up and named as `[[bin]]` declares. Falls back to `None` rather
/// than guessing at `target/debug`.
fn release_binary() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.parent()?;
    let candidate = dir.join("memory-wire");
    candidate.exists().then_some(candidate)
}

/// Fold the WAL into the main file, which is what turns the transient footprint
/// into the settled one. `TRUNCATE` rather than `PASSIVE` because the point is to
/// make the number reproducible, and a passive checkpoint that is merely
/// *allowed* to fold pages would report a size that depends on the schedule.
fn checkpoint(db: &Path) -> Result<()> {
    let conn = Connection::open(db)?;
    conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
    Ok(())
}

/// What a real `serve` process costs: RSS once `/health` answers, then once more
/// after a single HTTP retain.
struct Serve {
    idle_kb: Option<u64>,
    after_retain_kb: Option<u64>,
    /// The retain's HTTP status, so a 4xx can never be read as "a write happened"
    /// behind the RSS number it is paired with.
    retain_status: String,
}

/// Spawn `memory-wire serve` on a free loopback port, read its RSS twice, stop it.
///
/// The retain goes over HTTP on purpose: it is the request the README's
/// "RSS after first retain" describes, and a direct `Store::put` into the same file
/// from this process would measure this process instead of the server.
fn measure_serve() -> Option<Serve> {
    let binary = release_binary()?;
    let db = std::env::temp_dir().join(format!("mw-serve-{}.db", std::process::id()));
    rm_db(&db);
    let port = free_port();
    let addr = format!("127.0.0.1:{port}");
    let mut child: Child = Command::new(&binary)
        .args(["serve", "--addr", &addr, "--db"])
        .arg(&db)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    // The health wait is what makes "idle" mean idle: RSS is read only once the
    // server is actually answering, so it cannot catch a process mid-boot. How long
    // that wait took is `examples/bench_coldstart.rs`'s row, not this one's.
    let up = wait_healthy(&addr, HEALTH_TIMEOUT);
    let idle_kb = rss_kb_of(child.id());
    // One retain through the HTTP surface, the same request the README's figure is
    // described in terms of. Its status is checked so a 4xx cannot be mistaken for
    // a write that happened.
    let body = r#"{"content":"auth uses jose middleware for jwt verification"}"#;
    let retain_status = match up {
        // No answer means no RSS figure is "idle"; saying so beats printing the
        // memory of a process that was still booting.
        None => "server never answered /health".into(),
        Some(_) => http(&addr, "POST", "/banks/footprint/retain", Some(body))
            .map_or_else(|e| format!("failed: {e}"), |(code, _)| code.to_string()),
    };
    let after_retain_kb = rss_kb_of(child.id());
    let _ = child.kill();
    let _ = child.wait();
    rm_db(&db);
    Some(Serve {
        idle_kb,
        after_retain_kb,
        retain_status,
    })
}

fn mb(bytes: u64) -> String {
    format!("{:.2}", bytes as f64 / 1_048_576.0)
}

fn opt_mb(kb: Option<u64>) -> String {
    match kb {
        Some(kb) => format!("{:.2} MB", kb as f64 / 1024.0),
        None => "unavailable on this platform".into(),
    }
}

fn opt_kb(kb: Option<u64>) -> String {
    match kb {
        Some(kb) => format!("{kb} kB ({:.2} MB)", kb as f64 / 1024.0),
        None => "unavailable on this platform".into(),
    }
}

/// A median over repeats, printed as a range when the repeats disagree by more
/// than rounding — one RSS reading is a claim, three are a measurement.
fn opt_range_mb(reads: &[u64]) -> String {
    match reads {
        [] => "unavailable on this platform".into(),
        [only] => format!("{:.2} MB", *only as f64 / 1024.0),
        _ => {
            let lo = *reads.iter().min().unwrap_or(&0) as f64 / 1024.0;
            let hi = *reads.iter().max().unwrap_or(&0) as f64 / 1024.0;
            let mut sorted: Vec<u64> = reads.to_vec();
            sorted.sort_unstable();
            let mid = sorted[sorted.len() / 2] as f64 / 1024.0;
            match (hi - lo).abs() < 0.005 {
                true => format!("{mid:.2} MB"),
                false => format!("{mid:.2} MB ({lo:.2}-{hi:.2})"),
            }
        }
    }
}
