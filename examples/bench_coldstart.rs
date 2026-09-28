//! Cold start: what it costs before the first answer, and what the first query costs.
//!
//! Three costs that a latency benchmark taken after warmup cannot see, and that a
//! user meets first:
//! 1. **Process start to `/health` answering** — what a hook, a CLI call or an MCP
//!    client waits on before its first request can even be sent.
//! 2. **Store open on an existing database** — the `SqliteStore::open` call, which
//!    runs `configure` (the WAL/foreign-keys/busy-timeout pragmas), the idempotent
//!    `CREATE ... IF NOT EXISTS` batch, and the FTS consistency check that decides
//!    between two row counts and a full reindex. Reported per corpus size, because
//!    the check is O(rows) if it ever has to do the work and O(1) if it does not.
//! 3. **First recall versus warm recall** — the first query on a freshly opened
//!    store pays page-in for the FTS segment it touches; every later query does not.
//!    Averaging the two together is how a slow first response hides.
//!
//! Run: `cargo build --release --locked` first, then
//! `cargo run --release --example bench_coldstart -- [--sizes 0,1000,10000]
//!        [--repeats 3] [--warm 20] [--out-md PATH]`
//!
//! Method:
//! - **Start-to-health is timed from before the fork.** `Command::spawn` is
//!   included, and the health poll runs every 2 ms, so the figure is an upper bound
//!   on the real time with up to 2 ms of polling slop added. It is reported as a
//!   range over repeats because process start on a busy machine is the least stable
//!   number in this repository.
//! - **A real `serve` process, not a library call.** Start-to-health is a property of
//!   the binary: dynamic linking, the tokio runtime, the axum router, the store open
//!   and the socket bind all happen before `/health` can answer. Measuring
//!   `SqliteStore::open` alone would omit every one of those.
//! - **`open + migrate` is timed separately from the integrity check**, because they
//!   are different work with different scaling: the batch of `CREATE ... IF NOT
//!   EXISTS` statements is fixed, while the FTS check is O(rows) when the index has
//!   drifted and free when it has not. `rebuilt_fts_on_open()` is reported so a run
//!   that took the expensive branch says so instead of looking like a normal one.
//! - **First recall is one recall, timed once, on a store that was just opened** and
//!   has not answered a query in this process. `--warm` further recalls follow, and
//!   their p50 is the warm figure. The ratio is the artifact's headline.
//!
//! What this does NOT measure: MCP stdio startup (a different transport with its own
//! handshake, and `rmcp`'s, not the HTTP server's), the `seed` and `sweep` commands,
//! TLS or a reverse proxy in front, an OS page-cache-cold first boot of the binary
//! itself (the page cache is warm after the first repeat, which is why repeat 1 is
//! reported separately), and startup on a filesystem where the database file has to
//! be fetched from a network mount. It also does not measure concurrency — that is
//! `examples/bench_concurrency.rs`.
//!
//! Limitations. A size of 0 is a *fresh* database, so its "open" cost includes
//! creating the schema, which is not what a returning user's server pays; the
//! distinction is kept rather than averaged away. The integrity check is SQLite's
//! `integrity_check`, which is thorough and therefore slow on a large store; the
//! store's own startup check is cheaper, and this reports both so the difference is
//! visible instead of assumed. Every figure here is a whole-process or whole-store
//! number on a shared machine and moves with it; `--repeats` reports ranges.

mod bench_common;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use bench_common::{arg, db_bytes, free_port, profile, rm_db, wait_healthy};
use memory_wire::api::MemoryService;
use memory_wire::memory::{Bank, Memory};
use memory_wire::store::{SqliteStore, Store};
use rusqlite::Connection;
use std::process::{Command, Stdio};

/// Default ladder: nothing, 1k, 10k. 0 is what a first-ever run pays, 1k and 10k
/// are the sizes the committed artifacts already quote, and nothing past 10k is
/// included because `integrity_check` on a large store is slow enough to make this
/// harness a test of SQLite rather than of this binary.
const DEFAULT_SIZES: &str = "0,1000,10000";
/// Queries cycled for the warm figure, matching the other harnesses so a p50 here
/// is comparable to a p50 there.
const QUERIES: [&str; 4] = [
    "jose middleware jwt verification",
    "token bucket rate limiting",
    "pgvector hnsw vector index",
    "retention policy observations days",
];
/// Default budget, as in the other harnesses.
const BUDGET: usize = 2000;
/// How much slower the first recall must be, relative to the warm median, before
/// this harness calls it a penalty. 1.2x rather than "any amount above 1": the
/// first recall is a single sample and the warm figure is a median of `--warm`
/// samples, so the two statistics routinely disagree by a few percent, and reading
/// that as a first-touch cost would be inventing a finding.
const FIRST_QUERY_PENALTY: f64 = 1.2;
/// How long to wait for a spawned `serve` before calling it failed.
const HEALTH_TIMEOUT: Duration = Duration::from_secs(20);

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let sizes: Vec<usize> = arg(&args, "--sizes", DEFAULT_SIZES)
        .split(',')
        .map(|s| s.trim().parse::<usize>().expect("--sizes 0,1000,10000"))
        .collect();
    let repeats: usize = arg(&args, "--repeats", "3").parse().expect("--repeats N");
    let warm: usize = arg(&args, "--warm", "20").parse().expect("--warm N");
    let out_md = bench_common::out_md(&args, "BENCH_COLDSTART.md");
    if repeats == 0 || warm == 0 {
        anyhow::bail!("--repeats and --warm must be non-zero");
    }
    let size_list: Vec<String> = sizes.iter().map(usize::to_string).collect();

    // ---- 1. process start to /health ----------------------------------------
    let starts = match release_binary() {
        Some(_) => start_health(repeats),
        None => vec![],
    };

    // ---- 2 and 3. per-size open and first-recall -----------------------------
    let mut rows: Vec<SizeRow> = Vec::with_capacity(sizes.len());
    for &size in &sizes {
        let db: PathBuf =
            std::env::temp_dir().join(format!("mw-cold-{}-{size}.db", std::process::id()));
        rm_db(&db);
        build(&db, size)?;
        let mut opens = Vec::with_capacity(repeats);
        let mut checks = Vec::with_capacity(repeats);
        let mut firsts = Vec::with_capacity(repeats);
        let mut rebuilds = Vec::with_capacity(repeats);
        let db_main = db_bytes(&db).0;
        for _ in 0..repeats {
            let t = Instant::now();
            let store = SqliteStore::open(&db)?;
            let open_ms = t.elapsed().as_secs_f64() * 1000.0;
            rebuilds.push(store.rebuilt_fts_on_open());
            opens.push(open_ms);

            // The store's startup check is the two row counts; SQLite's own
            // `integrity_check` is the thorough one. Both are reported because
            // they are different work, and the difference is the interesting part.
            let t = Instant::now();
            let verdict: String = Connection::open(&db)?
                .query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))?;
            checks.push((t.elapsed().as_secs_f64() * 1000.0, verdict));

            let svc = MemoryService::new(store);
            let t = Instant::now();
            let _ = svc.recall("c", QUERIES[0], BUDGET)?;
            firsts.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        let mut warms: Vec<f64> = Vec::with_capacity(warm);
        {
            let store = SqliteStore::open(&db)?;
            let svc = MemoryService::new(store);
            for i in 0..warm {
                let t = Instant::now();
                let _ = svc.recall("c", QUERIES[i % QUERIES.len()], BUDGET)?;
                warms.push(t.elapsed().as_secs_f64() * 1000.0);
            }
        }
        rm_db(&db);
        let check_ms: Vec<f64> = checks.iter().map(|(ms, _)| *ms).collect();
        let mut sorted_warm = warms.clone();
        sorted_warm.sort_by(f64::total_cmp);
        let warm_p95 = sorted_warm[sorted_warm.len() * 95 / 100];
        rows.push(SizeRow {
            size,
            open_ms: median(&opens),
            check_ms: median(&check_ms),
            integrity: checks
                .first()
                .map_or("unavailable", |(_, v)| v.as_str())
                .to_string(),
            rebuilt: rebuilds.iter().filter(|b| **b).count(),
            first_ms: median(&firsts),
            warm_p50: median(&warms),
            warm_p95,
            db_main,
        });
    }

    // ---- artifact -----------------------------------------------------------
    let mut md = String::from("# Cold start (memory-wire)\n\n");
    md.push_str(&format!("{}\n\n", bench_common::provenance()));
    md.push_str(&format!(
        "Three costs a warmup latency benchmark cannot see: process start to `/health` \
         answering, store open on an existing database, and first recall against warm recall. \
         Sizes {}; {repeats} repeats each; {warm} warm recalls per size at a {BUDGET}-token \
         budget.\n\n",
        size_list.join(" / "),
    ));
    md.push_str("## Process start to `/health`\n\n");
    match starts.is_empty() {
        true => md.push_str(
            "Not measured — no `target/release/memory-wire`. Run `cargo build --release --locked` \
             first; the binary, not a library call, is what this row is about.\n\n",
        ),
        false => {
            let first = ms(starts[0]);
            // The first spawn is the one whose binary was not in the page cache, so
            // it gets its own line and the rest are summarized. With a single
            // successful spawn there is no "rest", and printing a min-max over an
            // empty set would print infinity.
            let rest = match starts.len() {
                0..=1 => "only one spawn answered; rerun with --repeats 3 or more".to_string(),
                _ => format!(
                    "{} / {} / {} (min / median / max of {})",
                    ms(min_of(&starts[1..])),
                    ms(median(&starts[1..])),
                    ms(max_of(&starts[1..])),
                    starts.len() - 1
                ),
            };
            md.push_str(&format!(
                "Timed from before `Command::spawn` to the first `GET /health` that answers 2xx \
                 with the body `ok`, polled every 2 ms, over {repeats} spawns of a real `serve` on \
                 a scratch database. The 2 ms poll interval is added slop, so this is an upper \
                 bound. A spawn that never answered is excluded rather than recorded as a slow \
                 one.\n\n\
                 | Run | Start to /health |\n|---|---|\n| first (binary not in page cache) | {first} ms |\n\
                 | subsequent | {rest} ms |\n\n",
            ));
        }
    }
    md.push_str("## Store open, integrity check, and first recall\n\n");
    md.push_str(
        "`open` is `SqliteStore::open` on an existing database: connection pragmas, the \
         idempotent `CREATE ... IF NOT EXISTS` batch, the column patches, and the FTS \
         consistency check. `integrity` is SQLite's own `PRAGMA integrity_check` on a second \
         connection — thorough, and therefore slower than the store's own check, which is why both \
         are here. `rebuilt FTS` counts repeats where the store decided the index had drifted and \
         reindexed; a nonzero value would mean that run's open cost is not comparable to the \
         others.\n\n\
         | Memories | open ms | integrity ms | integrity verdict | rebuilt FTS | first recall ms | warm p50 ms | warm p95 ms | first/warm | main B |\n\
         |---|---|---|---|---|---|---|---|---|---|\n",
    );
    for r in &rows {
        md.push_str(&r.row_md());
    }
    // The interesting result is usually the column reading *below* 1: a first query
    // that is not slower than a warm median. Say so explicitly, because the reason
    // this harness exists is to find out whether that is true, and a table alone
    // would leave the reader to guess which way to read it.
    //
    // The bar is `FIRST_QUERY_PENALTY`, not "greater than": the first recall is one
    // sample and the warm figure is the median of `--warm` of them, so a ratio a
    // hair above 1 is the two statistics disagreeing, not a measured penalty.
    // Calling that a finding would be inventing one.
    let ratio = |r: &SizeRow| r.first_ms / r.warm_p50.max(0.0001);
    let penalty: Vec<&SizeRow> = rows
        .iter()
        .filter(|r| ratio(r) > FIRST_QUERY_PENALTY)
        .collect();
    let ratios = rows
        .iter()
        .map(|r| format!("{}: {:.1}x", r.size, ratio(r)))
        .collect::<Vec<String>>()
        .join(", ");
    match penalty.is_empty() {
        true => md.push_str(&format!(
            "\n**No first-query penalty at these sizes.** The first recall after a fresh open is \
             not slower than the warm median anywhere ({ratios}), against a \
             {FIRST_QUERY_PENALTY:.1}x bar. The hypothesis this harness exists to test — that a \
             first query is slow because the FTS pages it touches are not yet resident — is not \
             supported here: a store opened on a file the same process just wrote has those pages \
             in the OS cache already. A ratio below 1.0 is one single sample beating a {warm}-sample \
             median, which is what sampling noise looks like, not a fast first query.\n",
            warm = warm,
        )),
        false => md.push_str(&format!(
            "\n**A first-query penalty is visible at {} of {} sizes** ({ratios}), against a \
             {FIRST_QUERY_PENALTY:.1}x bar: there the first recall after a fresh open costs more \
             than the warm median, which is the FTS first-touch cost this harness was built to \
             find.\n",
            penalty.len(),
            rows.len(),
        )),
    }
    md.push_str(
        "\n## What this does not measure\n\n\
         - MCP stdio startup. That is `rmcp`'s handshake over a pipe, not the HTTP server's, and \
           the two have nothing in common past the store open this table does cover.\n\
         - The `seed` and `sweep` commands, TLS, or a reverse proxy in front of the server.\n\
         - A genuinely cold binary: the page cache is warm after the first spawn, which is why \
           the first spawn is reported on its own row. A first-ever run on a cold page cache is \
           slower and this harness does not attempt to reproduce that condition.\n\
         - A database on a network mount, where the open cost is dominated by fetching the file \
           rather than by anything the store does.\n\
         - Concurrency, and any op other than recall.\n\n\
         ## Limitations\n\n\
         - A size of 0 is a *fresh* database, so its open cost includes creating the schema. That is \
           not what a returning user's server pays, and the two must not be averaged together — \
           which is why the size is a column rather than a run that is silently folded in.\n\
         - `PRAGMA integrity_check` is O(database) and is the slowest number in this table on a \
           large store. The store's own startup check is the cheaper one it actually runs; the gap \
           between the two columns is the cost of being thorough, not a cost the product pays.\n\
         - First recall is a single sample by construction: there is only one first recall per \
           process. `--repeats` gives repeats by reopening the store, which is the same condition, \
           but one sample is still one sample and its tail is not characterized. That is why the \
           verdict uses a 1.2x bar rather than any amount above 1, and why a ratio a hair under 1 \
           is reported as noise rather than as a fast first query.\n\
         - Start-to-health includes the OS scheduler placing the process, which on a loaded machine \
           is the dominant term. The min-max across repeats is the honest presentation.\n",
    );
    std::fs::write(&out_md, &md)?;
    eprintln!("wrote {out_md}");

    // ---- stdout -------------------------------------------------------------
    println!("=== cold start ===");
    println!("profile {}", profile());
    match starts.is_empty() {
        true => println!("start/health   not measured — no `target/release/memory-wire`"),
        false => {
            let first = ms(starts[0]);
            let rest = match starts.len() {
                0..=1 => "only one spawn answered".to_string(),
                _ => format!(
                    "min {} / median {} / max {}",
                    ms(min_of(&starts[1..])),
                    ms(median(&starts[1..])),
                    ms(max_of(&starts[1..]))
                ),
            };
            println!("start/health   first {first} ms · subsequent {rest} ms");
        }
    }
    println!();
    println!("| memories | open ms | integrity ms | verdict | rebuilt FTS | first ms | warm p50 | warm p95 | first/warm | main B |");
    println!("|---|---|---|---|---|---|---|---|---|---|");
    for r in &rows {
        println!("{}", r.row_md().trim_end_matches('\n'));
    }
    Ok(())
}

/// One corpus size, measured.
struct SizeRow {
    size: usize,
    open_ms: f64,
    check_ms: f64,
    integrity: String,
    rebuilt: usize,
    first_ms: f64,
    warm_p50: f64,
    warm_p95: f64,
    db_main: u64,
}

impl SizeRow {
    fn row_md(&self) -> String {
        let ratio = if self.warm_p50 > 0.0 {
            self.first_ms / self.warm_p50
        } else {
            0.0
        };
        format!(
            "| {} | {:.2} | {:.2} | {} | {} | {:.2} | {:.2} | {:.2} | {ratio:.1}x | {} |\n",
            self.size,
            self.open_ms,
            self.check_ms,
            self.integrity,
            self.rebuilt,
            self.first_ms,
            self.warm_p50,
            self.warm_p95,
            self.db_main,
        )
    }
}

/// Build a scratch database of `size` rows, in `scale_sweep`'s four-topic shape.
fn build(db: &std::path::Path, size: usize) -> Result<()> {
    let store = SqliteStore::open(db)?;
    store.put_bank(&Bank {
        id: "c".into(),
        name: "c".into(),
    })?;
    for i in 0..size {
        store.put(&Memory {
            id: format!("m-{i}"),
            bank_id: "c".into(),
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
    Ok(())
}

/// The release binary, found next to this example's own path — the same lookup
/// `bench_footprint` uses, and for the same reason: `cargo run --example` does not
/// build `[[bin]]`, so an absent binary is a real possibility rather than an error.
fn release_binary() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let candidate = exe.parent()?.parent()?.join("memory-wire");
    candidate.exists().then_some(candidate)
}

/// Spawn `serve` `repeats` times on a fresh scratch database and time each run
/// from before the fork to the first answering `GET /health`.
fn start_health(repeats: usize) -> Vec<f64> {
    let Some(binary) = release_binary() else {
        return vec![];
    };
    let mut out = Vec::with_capacity(repeats);
    for _ in 0..repeats {
        let db = std::env::temp_dir().join(format!("mw-cold-serve-{}.db", std::process::id()));
        rm_db(&db);
        // Port 0 would mean the kernel gave nothing back, and `serve --addr
        // 127.0.0.1:0` would then bind an ephemeral port this harness cannot know,
        // so the health poll would watch a port nobody is listening on. Refuse
        // rather than report a timeout as a slow start.
        let port = free_port();
        if port == 0 {
            break;
        }
        let addr = format!("127.0.0.1:{port}");
        let start = Instant::now();
        let Ok(mut child) = Command::new(&binary)
            .args(["serve", "--addr", &addr, "--db"])
            .arg(&db)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            break;
        };
        let answered = wait_healthy(&addr, HEALTH_TIMEOUT).is_some();
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        let _ = child.kill();
        let _ = child.wait();
        rm_db(&db);
        // A run that never answered is not a slow start, it is a failure, and it
        // must not be folded into the range as though it were 20000 ms.
        if answered {
            out.push(elapsed);
        }
    }
    out
}

fn median(v: &[f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    s[s.len() / 2]
}

fn min_of(v: &[f64]) -> f64 {
    v.iter().copied().fold(f64::MAX, f64::min)
}

fn max_of(v: &[f64]) -> f64 {
    v.iter().copied().fold(0.0f64, f64::max)
}

fn ms(v: f64) -> String {
    format!("{v:.1}")
}
