//! Concurrency vs throughput: N clients against ONE store, swept.
//!
//! `examples/soak.rs` runs a mixed workload at a fixed client count and asserts
//! nothing about throughput — it counts operations and never divides by wall
//! time, so this repository has no throughput number at all. This harness adds
//! the missing axis: the same store, the same operation, the same fixed work per
//! client, at 1 / 2 / 4 / 8 / 16 / 32 / 64 clients.
//!
//! Run: `cargo run --release --example bench_concurrency -- [--memories 2000]
//!        [--clients 1,2,4,8,16,32,64] [--ops-per-client 200] [--repeats 3]
//!        [--op recall|retain] [--scaling-floor 1.5] [--out-md PATH]`
//!
//! Method, and why each choice is the one that isolates the store:
//! - **Fixed ops per client, not a time box.** A time box makes ops/s a function
//!   of the scheduler as much as of the store, and a `--seconds` run that got
//!   descheduled mid-step reports a throughput nobody can reproduce. Fixed work
//!   makes aggregate ops/s exactly `clients x ops / wall`, and wall is the thing
//!   under test.
//! - **`std::thread`, not the HTTP surface.** The service's own handlers are
//!   `async fn` over a blocking store on a runtime whose worker count is its own
//!   tunable; going through a socket would measure axum, tokio and the client
//!   alongside the store. Threads over the library call isolate what is actually
//!   in question: whether one `Mutex<Connection>` serializes the work.
//! - **Recall is the default op.** A read leaves the bank the same size at every
//!   client count, so the candidate pool, the FTS index and the page cache are
//!   held constant across the sweep and the only variable left is concurrency.
//!   `--op retain` measures the write path instead, which is the one the pragma
//!   change is aimed at, at the cost of a bank that grows by the run's work.
//! - **The store is built and seeded once, outside every timed region.** Index
//!   construction charged to the first client would flatter every later step.
//!
//! Reported, nonzero exit on a serialization signature:
//! - **Aggregate ops/s must grow with the client count.** This is the whole
//!   point. A store behind one mutex, or one connection, cannot overlap work, so
//!   N clients deliver the same ops/s as one while every client's latency grows
//!   linearly instead. `SCALING_FLOOR` is deliberately low (1.5x from the 1-client
//!   step to the largest): the check is for the *absence* of any gain, not for
//!   good scaling. A working read pool clears it by a wide margin; a store that
//!   cannot overlap work cannot, and that is the finding, not a broken harness.
//! - **RSS must not grow per client without bound**, bounded by the store's own
//!   page cache plus per-thread allocator arenas. See `soak.rs::RSS_PER_CLIENT_MB`
//!   for why the per-thread part is real and why one megabyte per client is
//!   headroom rather than slack a leak hides in.
//!
//! What this does NOT measure: HTTP, MCP or JSON cost; request redaction; any
//! multi-process case; any store other than `SqliteStore`; and any read/write mix
//! other than the two named ops. Throughput here is a store property — a
//! deployment with a real network in front will not see these numbers, and one
//! running several server processes against the same file will see something
//! neither this nor `soak.rs` describes.
//!
//! Limitations. Latency percentiles pool every op of every client in the step, so
//! they describe the contention a step created, not one client's experience;
//! across repeats the *median* of each percentile is reported, because a tail
//! percentile on a shared machine is scheduler noise before it is anything else.
//! `--op retain` grows the bank by `clients x ops` rows over the sweep, so later
//! steps run against a larger bank than earlier ones — bounded-pool recall means
//! the effect is small, but the step column is not then a constant-size
//! measurement and the artifact says so rather than implying otherwise. One step
//! bounds a leak, it cannot prove the absence of one: rerun at a different
//! `--ops-per-client` and check RSS does not track the work done.

mod bench_common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{bail, Result};
use bench_common::{arg, db_bytes, pct, profile, rss_kb, rm_db, shuffle};
use memory_wire::api::MemoryService;
use memory_wire::memory::{Bank, Memory};
use memory_wire::store::{SqliteStore, Store};

/// The sweep, unless `--clients` says otherwise: a doubling ladder spanning
/// "one thread" to "past the core count", which is where a serialization
/// signature is unmistakable.
const DEFAULT_SWEEP: &str = "1,2,4,8,16,32,64";
/// How much aggregate throughput the largest step must add over one client.
///
/// 1.5x, not "linear". A store that cannot overlap work scores ~1.0x and fails; a
/// store with a working read pool scores far higher and passes. Nothing between
/// those two outcomes is the interesting case, and demanding near-ideal scaling
/// would make this fail on a busy machine for reasons that have nothing to do
/// with the store.
const SCALING_FLOOR: f64 = 1.5;
/// Resident growth allowed per client on top of the store's page cache, in MB.
/// `soak.rs` measured 2.9 MB for one client and 15.7 MB for sixteen — glibc gives
/// every thread its own arena and FTS5 churn leaves freed blocks in each — so one
/// megabyte per client is headroom over a per-thread effect, not a leak budget.
const RSS_PER_CLIENT_MB: u64 = 1;
/// Queries cycled so the FTS stream always has real candidates.
const QUERIES: [&str; 4] = [
    "jose middleware jwt verification",
    "token bucket rate limiting",
    "pgvector hnsw vector index",
    "retention policy observations days",
];

/// Which store operation every client runs. Kept to two so the sweep has exactly
/// one variable; a mix would report a number belonging to neither op.
#[derive(Clone, Copy, PartialEq)]
enum Op {
    Recall,
    Retain,
}

impl Op {
    fn name(self) -> &'static str {
        match self {
            Op::Recall => "recall",
            Op::Retain => "retain",
        }
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let memories: usize = arg(&args, "--memories", "2000")
        .parse()
        .expect("--memories N");
    let ops: u64 = arg(&args, "--ops-per-client", "200")
        .parse()
        .expect("--ops-per-client N");
    let repeats: usize = arg(&args, "--repeats", "3")
        .parse()
        .expect("--repeats N");
    let out_md = bench_common::out_md(&args, "BENCH_CONCURRENCY.md");
    let op = match arg(&args, "--op", "recall").as_str() {
        "recall" => Op::Recall,
        "retain" => Op::Retain,
        other => bail!("--op must be recall or retain, got {other:?}"),
    };
    let floor: f64 = arg(&args, "--scaling-floor", &format!("{SCALING_FLOOR}"))
        .parse()
        .expect("--scaling-floor F");
    let sweep: Vec<usize> = arg(&args, "--clients", DEFAULT_SWEEP)
        .split(',')
        .map(|s| s.trim().parse::<usize>().expect("--clients 1,2,4"))
        .collect();
    if sweep.is_empty() || repeats == 0 || ops == 0 || memories == 0 {
        bail!("--clients, --repeats, --ops-per-client and --memories must all be non-zero");
    }

    let db: PathBuf = std::env::temp_dir().join(format!("mw-conc-{}.db", std::process::id()));
    rm_db(&db);

    // Built and seeded once, outside every timed region.
    let seeded = seed(&db, memories)?;
    let biggest = *sweep.last().expect("non-empty sweep");
    let ceiling =
        page_cache_bytes(&db)? + biggest as u64 * RSS_PER_CLIENT_MB * 1_048_576;
    let rss_before = rss_kb();
    let store = SqliteStore::open(&db)?;
    let svc = MemoryService::new(store);

    let mut rows: BTreeMap<usize, Vec<Step>> = BTreeMap::new();
    for &clients in &sweep {
        for _ in 0..repeats {
            rows.entry(clients)
                .or_default()
                .push(step(&svc, clients, ops, op));
        }
    }
    let rss_after = rss_kb();
    let rss_here = rss_now();

    // ---- table --------------------------------------------------------------
    // The first step in the sweep establishes the baseline and is not itself
    // measured against the floor: a 1x gain over itself is the definition of the
    // baseline, not evidence of anything.
    let first = sweep[0];
    let mut lines = Vec::with_capacity(sweep.len());
    let mut base = 0.0f64;
    let mut thin: Vec<(usize, f64, f64)> = Vec::new();
    for &clients in &sweep {
        let steps = &rows[&clients];
        let mean = mean_ops(steps);
        let lo = steps
            .iter()
            .map(|s| s.ops_per_s)
            .fold(f64::MAX, f64::min);
        let hi = steps.iter().map(|s| s.ops_per_s).fold(0.0f64, f64::max);
        if base == 0.0 {
            base = mean;
        }
        let gain = if base > 0.0 { mean / base } else { 0.0 };
        // `efficiency` is the finer-grained form of the same signature the gate
        // reads: 1.0 is perfect scaling, and a store that can only overlap the
        // non-SQL part of a recall plateaus well below 1. Growth is the gate;
        // efficiency is the number that says how much of the win is left on the
        // table once the gate passes.
        let efficiency = gain / clients as f64;
        if clients > first && gain < floor {
            thin.push((clients, gain, mean));
        }
        lines.push(format!(
            "| {clients} | {mean:.1} ({lo:.1}-{hi:.1}) | {} | {} | {} | {rss_here} | {gain:.2}x | {efficiency:.2} |",
            med(steps.iter().map(|s| s.p50)),
            med(steps.iter().map(|s| s.p95)),
            med(steps.iter().map(|s| s.p99)),
        ));
    }
    let big_gain = mean_ops(&rows[&biggest]) / base;
    let big_eff = big_gain / biggest as f64;
    let ceiling_mb = ceiling as f64 / 1_048_576.0;
    let panics: u64 = rows.values().flatten().map(|s| s.panics).sum();

    // ---- artifact -----------------------------------------------------------
    let (main, wal, shm) = db_bytes(&db);
    let mut md = String::from("# Concurrency vs throughput (memory-wire)\n\n");
    md.push_str(&format!("{}\n\n", bench_common::provenance()));
    md.push_str(&format!(
        "Workload: `std::thread` clients against ONE `SqliteStore` (one writer \
         `Mutex<Connection>` plus a 4-connection WAL read pool, writer-first with \
         spill-on-contention), `{op}` op, {memories} seeded memories, {ops} ops per client, \
         {repeats} repeats per client count. Aggregate ops/s is `clients x {ops} / wall`; \
         latencies are microseconds. `RSS MB` is this process at the end of the step.\n\n",
        op = op.name(),
    ));
    md.push_str(
        "Percentiles pool every op of every client in a step, so they describe the contention \
         that step created rather than one client's experience; across repeats the median of each \
         percentile is shown. `ops/s` carries its min-max range across repeats — thread scheduling \
         is not this harness's to control, so single-digit-percent movement between runs is noise, \
         not a result. `efficiency` is `vs 1 client / clients`: 1.00 is perfect scaling, and a \
         store that can overlap only part of an operation plateaus below it.\n\n\
         | Clients | ops/s (min-max) | p50 us | p95 us | p99 us | RSS MB | vs 1 client | Eff |\n\
         |---|---|---|---|---|---|---|---|\n",
    );
    for l in &lines {
        md.push_str(l);
        md.push('\n');
    }
    md.push_str(&format!(
        "\n**Scaling gate: {big_gain:.2}x aggregate ops/s at {biggest} clients against the \
         1-client step ({big_eff:.2} of ideal), floor {floor:.2}x — {}.**\n\n",
        verdict(big_gain, floor)
    ));
    md.push_str(&format!(
        "RSS across the whole sweep: {}. Ceiling {ceiling_mb:.1} MB (store page cache + \
         {RSS_PER_CLIENT_MB} MB/client). Database at exit: main {main} B, `-wal` {wal} B, `-shm` \
         {shm} B — the WAL is unflushed for as long as the store holds the file open. Panicked \
         clients across the whole sweep: {panics}.\n\n",
        rss_line(rss_before, rss_after)
    ));
    md.push_str(
        "## What this does not measure\n\n\
         - HTTP, MCP and JSON cost. The clients are threads over the library call, so axum, tokio \
           and a socket are deliberately absent.\n\
         - Any store but `SqliteStore`, any second process, and any deployment where the server \
           is not the only reader or writer.\n\
         - Request redaction, non-default budgets, tag filters, and the lifecycle routes.\n\n\
         ## Limitations\n\n\
         - A p99 over 64 threads on a shared machine is scheduler noise before it is store \
           contention. Quote the ops/s range; treat the percentile tail as indicative only.\n\
         - `--op retain` grows the bank by `clients x ops` rows over the sweep, so later steps run \
           against a larger bank than earlier ones. Recall cost is pool-bounded so the effect is \
           small, but the step column is not a constant-size measurement.\n\
         - One step bounds a leak, it cannot prove the absence of one: rerun at a different \
           `--ops-per-client` and check the RSS column does not track the work done.\n\
         - `SCALING_FLOOR` is an absence-of-gain check, not a performance target. Passing it means \
           the store overlaps *some* work, not that it scales well — read the `Eff` column for how \
           much of the ideal gain is actually realized, since a store that releases the lock during \
           the non-SQL part of a recall can grow aggregate throughput while still serializing every \
           query. A regression that halves efficiency can pass this gate.\n",
    );
    std::fs::write(&out_md, &md)?;
    eprintln!("wrote {out_md}");

    // ---- stdout -------------------------------------------------------------
    println!("=== concurrency vs throughput ===");
    println!(
        "{} client counts (1..{biggest}) · op {op} · {memories} memories · {ops} ops/client · {repeats} repeats · profile {prof}",
        sweep.len(),
        op = op.name(),
        prof = profile()
    );
    println!("db {} · seeded {seeded} rows", db.display());
    println!("rss ceiling {ceiling_mb:.1} MB (page cache + {RSS_PER_CLIENT_MB} MB/client)");
    println!();
    println!("| clients | ops/s (min-max) | p50 us | p95 us | p99 us | rss MB | vs 1 client | Eff |");
    println!("|---|---|---|---|---|---|---|---|");
    for l in &lines {
        println!("{l}");
    }
    println!();
    for (clients, gain, ops_s) in &thin {
        println!(
            "scaling   {clients} clients: {gain:.2}x aggregate ops/s ({ops_s:.1}/s), under the {floor:.2}x floor"
        );
    }
    println!(
        "scaling   gate at {biggest} clients: {big_gain:.2}x vs floor {floor:.2}x · {}",
        verdict(big_gain, floor)
    );
    println!("rss       {} (ceiling {ceiling_mb:.1} MB)", rss_line(rss_before, rss_after));
    println!("storage   main {main} B · wal {wal} B · shm {shm} B (WAL unflushed while the store holds the file open)");
    println!("panics    {panics}");
    println!();
    let rss_ok = match (rss_before, rss_after) {
        (Some(b), Some(a)) => (a.saturating_sub(b) * 1024) <= ceiling,
        // Degrade as `soak.rs` does: no `/proc`, no RSS gate, run still passes.
        _ => true,
    };
    if !rss_ok {
        bail!("FAIL: RSS grew past the page cache + {RSS_PER_CLIENT_MB} MB/client ceiling ({} against {ceiling_mb:.1} MB)", rss_line(rss_before, rss_after));
    }
    if panics > 0 {
        bail!("FAIL: {panics} panicked clients — a poisoned store is not a throughput result");
    }
    if !thin.is_empty() {
        let detail: Vec<String> = thin
            .iter()
            .map(|(c, g, o)| format!("{c} clients {g:.2}x ({o:.1} ops/s)"))
            .collect();
        bail!(
            "FAIL: aggregate ops/s does not grow with client count — a serialization signature. \
             floor {floor:.2}x, observed {}",
            detail.join(", ")
        );
    }
    println!("verdict    PASS — ops/s grew {big_gain:.2}x from 1 to {biggest} clients, floor {floor:.2}x");
    rm_db(&db);
    Ok(())
}

/// One timed step: `clients` threads, `ops` operations each, pooled latencies.
struct Step {
    ops_per_s: f64,
    p50: u64,
    p95: u64,
    p99: u64,
    panics: u64,
}

/// One client count, one repeat.
///
/// Threads over the library call, a fixed op count each, every latency pooled. A
/// panic is counted and skipped rather than unwound into the numbers: a poisoned
/// store is a finding, and dropping the sample silently would count it as a
/// success. Each client's op order is an LCG permutation, so its work is
/// identical run to run and differs between clients.
fn step<S: memory_wire::store::Store>(svc: &MemoryService<S>, clients: usize, ops: u64, op: Op) -> Step {
    let mut lat: Vec<u64> = Vec::with_capacity(clients * ops as usize);
    let mut panics = 0u64;
    let start = Instant::now();
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..clients)
            .map(|c| {
                scope.spawn(move || {
                    let mut local: Vec<u64> = Vec::with_capacity(ops as usize);
                    for i in shuffle(ops as usize, 0xC0FFEE ^ c as u64) {
                        let q = QUERIES[i % QUERIES.len()];
                        let t = Instant::now();
                        match op {
                            Op::Recall => {
                                let _ = svc.recall("c", q, 2000);
                            }
                            Op::Retain => {
                                let _ = svc.retain(
                                    "c",
                                    &format!("conc client {c} op {i} about {q}"),
                                    Some(format!("c{c}")),
                                );
                            }
                        }
                        local.push(t.elapsed().as_micros() as u64);
                    }
                    local
                })
            })
            .collect();
        for h in handles {
            match h.join() {
                Ok(v) => lat.extend(v),
                Err(_) => panics += 1,
            }
        }
    });
    let wall = start.elapsed();
    lat.sort_unstable();
    Step {
        ops_per_s: if wall.as_secs_f64() > 0.0 {
            lat.len() as f64 / wall.as_secs_f64()
        } else {
            0.0
        },
        p50: pct(&lat, 0.50),
        p95: pct(&lat, 0.95),
        p99: pct(&lat, 0.99),
        panics,
    }
}

/// Fill the store with `memories` rows in `scale_sweep`'s 4-topic shape (three
/// signal topics plus a distractor), so a recall here sees the same corpus shape
/// the committed `eval/SCALE_SWEEP.md` describes.
fn seed(db: &Path, memories: usize) -> Result<u64> {
    let store = SqliteStore::open(db)?;
    store.put_bank(&Bank {
        id: "c".into(),
        name: "c".into(),
    })?;
    for i in 0..memories {
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
    Ok(memories as u64)
}

/// The store's own page cache in bytes: `cache_size` counts pages and is negative
/// when it does not. This is the part of the RSS ceiling that comes from the
/// store; [`RSS_PER_CLIENT_MB`] is the rest.
fn page_cache_bytes(db: &Path) -> Result<u64> {
    let audit = rusqlite::Connection::open(db)?;
    let page: u64 = audit.query_row("PRAGMA page_size", [], |r| r.get(0))?;
    let pages: i64 = audit.query_row("PRAGMA cache_size", [], |r| r.get(0))?;
    Ok(pages.unsigned_abs() * page)
}

fn mean_ops(steps: &[Step]) -> f64 {
    steps.iter().map(|s| s.ops_per_s).sum::<f64>() / steps.len().max(1) as f64
}

/// Median of an iterator of per-repeat percentiles, so a tail percentile is not
/// decided by whichever repeat the machine happened to deschedule.
fn med(vals: impl Iterator<Item = u64>) -> u64 {
    let mut v: Vec<u64> = vals.collect();
    v.sort_unstable();
    match v.len() {
        0 => 0,
        n => v[n / 2],
    }
}

fn rss_now() -> String {
    match rss_kb() {
        Some(kb) => format!("{:.1}", kb as f64 / 1024.0),
        None => "n/a".into(),
    }
}

fn rss_line(before: Option<u64>, after: Option<u64>) -> String {
    match (before, after) {
        (Some(b), Some(a)) => format!(
            "{:.1} MB -> {:.1} MB (delta {:+.1} MB)",
            b as f64 / 1024.0,
            a as f64 / 1024.0,
            (a as f64 - b as f64) / 1024.0
        ),
        _ => "unavailable on this platform".into(),
    }
}

fn verdict(gain: f64, floor: f64) -> &'static str {
    if gain >= floor {
        "PASS, throughput grows with client count"
    } else {
        "FAIL, throughput does not grow with client count — a serialization signature"
    }
}
