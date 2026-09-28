//! Write path: what one retain costs, and what one *commit* costs.
//!
//! Nothing in the repository measures this. `scale_sweep` reports total index build
//! time, `soak` counts retains as one op among six, and neither divides by wall
//! time — so the write path had a number quoted about it (505 commits/s) and no
//! committed script that produced it. This harness makes the cost visible and
//! separates it from the two other things a retain does.
//!
//! The store sets `journal_mode=wal` **and** `synchronous=NORMAL`, so its
//! connection does not fsync per commit. What is left is everything else a commit
//! costs, and this artifact prices that; the D1 before/after (the `FULL` side of
//! the same comparison) is in `docs/CONSISTENCY.md` §11.1.
//!
//! Run: `cargo run --release --example bench_write -- [--sizes 1000,10000]
//!        [--repeats 1] [--out-md PATH]`
//!
//! Three paths, so the numbers decompose instead of blending. Each starts from a
//! fresh database with the same schema and bank and writes the *same* generated
//! content:
//! 1. **`service retain`** — `MemoryService::retain`, the real per-retain cost an
//!    HTTP or MCP caller pays: PII redaction, a bank-config read, content-hash
//!    dedup, one transaction, one commit. Creates the bank on its first call, so
//!    no row is seeded into the timed region.
//! 2. **`store put`** — `Store::put` directly: one row, one commit, no service
//!    work. The gap to (1) prices redaction plus the config read.
//! 3. **`batched insert`** — N rows inside ONE `rusqlite` transaction, so it pays
//!    one commit instead of N. The gap to (2) is the per-commit cost: N-1 fsyncs
//!    the batch does not pay. That ratio is the point of the harness.
//!
//! Path 3 is a deliberate limitation, not an oversight. **`Store` has no batch
//! write**, so no multi-row commit is reachable through the public API, and
//! `src/` is owned elsewhere. The batch therefore goes through `rusqlite`
//! directly on the same file against the same schema. It is sound for the one
//! thing being compared — commit count against row count — because the FTS index
//! is kept in step by the `memories_ai` trigger, so a batched `INSERT` populates
//! `memories_fts` exactly as (2) does. It is *not* a claim that the service could
//! do this: it skips redaction, dedup and tag handling, which is why it is its own
//! row in the table and never described as a faster way to retain.
//!
//! Also recorded: `journal_mode` and `synchronous` as an **audit** connection reads
//! them. Both are per-connection settings, so that reading is the compiled-in
//! default the store's own connection also carries (it sets the first and not the
//! second) — evidence of what the store does *not* configure, not a live reading
//! of the store's handle.
//!
//! What this does NOT measure: throughput under concurrency (that is
//! `examples/bench_concurrency.rs --op retain`), recall over the written bank,
//! document-scoped upserts, tag writes, deletes, and settled storage after a bulk
//! load (`examples/bench_footprint.rs` reports that). Nor the cost of real
//! content: the generated strings are short and PII-free, so redaction and
//! hashing see the cheap end of their input distribution.
//!
//! Limitations. Every path-1 and path-2 row is its own transaction, so this is the
//! worst case by construction — a caller that batched through a future API would
//! land nearer path 3. At 10k memories the total is dominated by fsync and moves
//! with the storage underneath it, so `--repeats` reports a range rather than a
//! point. `B/memory` is per path and not comparable across paths at equal row
//! counts: path 1 also writes `memory_tags` rows and a `content_hash`. A p50 is
//! reported for the two single-row paths only — the batched path has one duration,
//! so a percentile over it would be a fabricated statistic.

mod bench_common;

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{bail, Result};
use bench_common::{arg, db_bytes, pct, profile, rm_db};
use memory_wire::api::MemoryService;
use memory_wire::memory::{Bank, Memory};
use memory_wire::store::{SqliteStore, Store};
use rusqlite::{params, Connection};

/// Default sizes: 1k and 10k. 10k is the size the README's storage figure is
/// quoted at, so a write harness that stopped short of it would leave the number
/// it exists to explain unconnected.
const DEFAULT_SIZES: &str = "1000,10000";

/// The three write paths. A `usize` discriminant keeps the scratch-file name
/// unique per path without a `Hash` of anything.
#[derive(Clone, Copy, PartialEq)]
enum Write3 {
    ServiceRetain,
    StorePut,
    Batched,
}

impl Write3 {
    fn name(self) -> &'static str {
        match self {
            Self::ServiceRetain => "service retain",
            Self::StorePut => "store put",
            Self::Batched => "batched insert",
        }
    }

    const ALL: [Self; 3] = [Self::ServiceRetain, Self::StorePut, Self::Batched];
}

impl Write {
    /// Throughput of one run, from its own wall time.
    fn ops(&self) -> f64 {
        self.memories as f64 / self.wall_s
    }
}

/// One measured write of `n` rows through one path, repeats already collapsed.
struct Write {
    path: Write3,
    memories: usize,
    /// Median wall time across repeats.
    wall_s: f64,
    /// ops/s range across repeats, for the number that actually moves.
    ops_lo: f64,
    ops_hi: f64,
    mean_us: f64,
    /// `None` for the batched path: one duration is not a distribution.
    p50_us: Option<u64>,
    /// Main-file bytes after the store closed, i.e. with the WAL already folded
    /// in. The WAL-*unflushed* figure is a different measurement — see
    /// `examples/bench_footprint.rs`, which reads it while the store holds the
    /// file open, the way the README's storage line does.
    db_main: u64,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let sizes: Vec<usize> = arg(&args, "--sizes", DEFAULT_SIZES)
        .split(',')
        .map(|s| s.trim().parse::<usize>().expect("--sizes 1000,10000"))
        .collect();
    let repeats: usize = arg(&args, "--repeats", "1").parse().expect("--repeats N");
    let out_md = bench_common::out_md(&args, "BENCH_WRITE.md");
    if sizes.contains(&0) || repeats == 0 {
        bail!("--sizes and --repeats must be non-zero");
    }
    let size_list: Vec<String> = sizes.iter().map(usize::to_string).collect();

    // The pragmas come off one scratch database: they are set per open, so one
    // reading describes every open this harness makes.
    let probe: PathBuf =
        std::env::temp_dir().join(format!("mw-write-probe-{}.db", std::process::id()));
    rm_db(&probe);
    SqliteStore::open(&probe)?;
    let (journal, sync) = pragmas(&probe);
    rm_db(&probe);

    let mut results: Vec<Write> = Vec::new();
    for &n in &sizes {
        for path in Write3::ALL {
            let runs: Vec<Write> = (0..repeats).map(|_| time_write(path, n)).collect::<Result<_>>()?;
            let mut walls: Vec<f64> = runs.iter().map(|r| r.wall_s).collect();
            walls.sort_by(f64::total_cmp);
            let mid = walls[walls.len() / 2];
            let mut p50s: Vec<u64> = runs.iter().filter_map(|r| r.p50_us).collect();
            p50s.sort_unstable();
            let first = runs.first();
            results.push(Write {
                path,
                memories: n,
                wall_s: mid,
                ops_lo: runs.iter().map(Write::ops).fold(f64::MAX, f64::min),
                ops_hi: runs.iter().map(Write::ops).fold(0.0f64, f64::max),
                mean_us: mid / n as f64 * 1e6,
                p50_us: match p50s.is_empty() {
                    true => None,
                    false => Some(p50s[p50s.len() / 2]),
                },
                db_main: first.map_or(0, |r| r.db_main),
            });
        }
    }

    // ---- artifact -----------------------------------------------------------
    let mut md = String::from("# Write path (memory-wire)\n\n");
    md.push_str(&format!("{}\n\n", bench_common::provenance()));
    md.push_str(&format!(
        "Three write paths over the same generated content, each from a fresh database. \
         `service retain` is the full `MemoryService::retain` (redaction, bank-config read, \
         content-hash dedup, one transaction, one commit). `store put` is `Store::put` (one row, \
         one commit, no service work). `batched insert` is N rows in ONE `rusqlite` \
         transaction, paying one commit instead of N. Sizes {}; {repeats} repeat(s) per cell, \
         the median wall time is reported and ops/s carries its range.\n\n",
        size_list.join(" / "),
    ));
    md.push_str(&format!(
        "Store connection configuration: `journal_mode={journal}`, `synchronous={sync}` as an \
         **audit connection** reads them, i.e. SQLite's compiled-in defaults. The store does \
         **not** run with the `synchronous` shown here: it sets `PRAGMA synchronous=NORMAL` in \
         `configure()`, and both pragmas are per-connection so an audit handle cannot see that. \
         Every absolute figure in this artifact was measured under the store's real \
         configuration — `journal_mode=wal`, `synchronous=NORMAL` (no fsync per commit), not the \
         `FULL` an audit handle reports. The D1 before/after is in \
         `docs/CONSISTENCY.md` §11.1; the numbers here are the *after* side, so a `FULL` audit \
         reading must not be read back as the cost of these writes.\n\n"
    ));
    md.push_str(
        "Main-file bytes are read after the store closed, so the WAL is already folded in. The \
         WAL-*unflushed* figure — what a server that has just been fed 10k memories and not yet \
         checkpointed occupies — is a different measurement and belongs to \
         `eval/BENCH_FOOTPRINT.md`.\n\n",
    );
    md.push_str(
        "| Memories | Path | Build time | ops/s (min-max) | mean us/op | p50 us/op | main B | B/memory |\n\
         |---|---|---|---|---|---|---|---|\n",
    );
    for r in &results {
        md.push_str(&row_md(r));
    }
    for &n in &sizes {
        if let (Some(single), Some(batch), Some(svc)) = (
            results.iter().find(|r| r.memories == n && r.path == Write3::StorePut),
            results.iter().find(|r| r.memories == n && r.path == Write3::Batched),
            results.iter().find(|r| r.memories == n && r.path == Write3::ServiceRetain),
        ) {
            md.push_str(&format!(
                "\nAt {n} memories: one-commit-per-row is **{single_x:.1}x** the wall time of one \
                 commit for the whole batch ({single:.2}s vs {batch:.2}s), and the full service \
                 path is **{svc_x:.1}x** it. A `synchronous` change moves the first ratio; only \
                 an API that can batch moves the second.\n",
                single_x = single.wall_s / batch.wall_s,
                single = single.wall_s,
                batch = batch.wall_s,
                svc_x = svc.wall_s / batch.wall_s,
            ));
        }
    }
    // Once, after every size, not once per size: the point is that the ratio moves
    // with the corpus, which is only visible when the per-size lines are read
    // against each other.
    if sizes.len() > 1 {
        md.push_str(
            "\n**The ratio is not a constant, and must not be extrapolated.** It moves with the \
             corpus because the batch stops being commit-bound: at the smaller size the batch is \
             one fsync against a handful of rows, so the ratio is nearly the whole difference; by \
             the larger size the batch is dominated by building the FTS index for every row, which \
             no pragma can avoid. Read the ratio as *the share of single-row write time that is \
             commit overhead at that size*. The ops/s range is the number to quote for the \
             absolute cost.\n",
        );
    }
    md.push_str(&limits());
    std::fs::write(&out_md, &md)?;
    eprintln!("wrote {out_md}");

    // ---- stdout -------------------------------------------------------------
    println!("=== write path ===");
    println!("profile {} · sizes {} · repeats {repeats}", profile(), size_list.join(","));
    println!("store connection (audit handle): journal_mode={journal} synchronous={sync} (the store sets both: wal + NORMAL)");
    println!();
    for r in &results {
        print!("{}", row_md(r));
    }
    for &n in &sizes {
        if let (Some(single), Some(batch), Some(svc)) = (
            results.iter().find(|r| r.memories == n && r.path == Write3::StorePut),
            results.iter().find(|r| r.memories == n && r.path == Write3::Batched),
            results.iter().find(|r| r.memories == n && r.path == Write3::ServiceRetain),
        ) {
            println!(
                "ratio     {n}: store put is {:.1}x the batch, service retain is {:.1}x the batch",
                single.wall_s / batch.wall_s,
                svc.wall_s / batch.wall_s
            );
        }
    }
    Ok(())
}

fn limits() -> String {
    "\n## What this does not measure\n\n\
     - Concurrency. Nothing here runs two writers at once; that is \
       `examples/bench_concurrency.rs --op retain`.\n\
     - A batch through the public API. `Store` has no batch write, so the batched row goes \
       through `rusqlite` directly against the same schema. It populates `memories_fts` \
       identically (the `memories_ai` trigger does that work), which is what makes the \
       commit-count comparison valid — but it skips redaction, dedup and tag handling, so it is \
       not a faster way to retain, only a way to price one commit.\n\
     - Recall over the written bank, document upserts, tag writes, deletes, or settled storage \
       after a bulk load (`examples/bench_footprint.rs` reports that).\n\
     - Real content cost. Generated strings are short and PII-free, so redaction and content \
       hashing see the cheap end of their input distribution; a corpus with emails and tokens in \
       it redoes more work per row.\n\n\
     ## Limitations\n\n\
     - Every `service retain` and `store put` row is its own transaction, so this is the worst \
       case by construction. A caller that batched through a future API would land nearer the \
       batched row.\n\
     - At 10k memories the total is dominated by fsync and moves with the storage underneath it. \
       That is why `--repeats` exists and why the ops/s range is the number to quote.\n\
     - `B/memory` is per path and not comparable across paths at equal row counts: `service \
       retain` also writes `memory_tags` rows and a `content_hash`.\n\
     - A p50 is reported for the two single-row paths only. The batched path has one duration, \
       so a percentile over it would be invented; its `mean us/op` is the honest figure.\n\
     - The store-put-to-batch ratio is size-dependent, as the prose above the table says. A \
       ratio quoted without its size is not a measurement.\n"
        .to_string()
}

/// One table row, shared by the artifact and stdout so the two cannot disagree.
fn row_md(r: &Write) -> String {
    let ops = match (r.ops_lo - r.ops_hi).abs() < f64::EPSILON {
        true => format!("{:.1}", r.ops_hi),
        false => format!("{:.1} ({:.1}-{:.1})", r.ops_hi, r.ops_lo, r.ops_hi),
    };
    format!(
        "| {} | {} | {:.2}s | {ops} | {:.0} | {} | {} | {:.0} |\n",
        r.memories,
        r.path.name(),
        r.wall_s,
        r.mean_us,
        r.p50_us.map_or_else(|| "n/a".into(), |v| v.to_string()),
        r.db_main,
        r.db_main as f64 / r.memories as f64,
    )
}

/// `(journal_mode, synchronous)` as an audit connection reads them, both as
/// names, because SQLite reports `synchronous` as a bare integer and "2" in a
/// committed artifact is a worse sentence than "2 (FULL)".
fn pragmas(db: &Path) -> (String, String) {
    let Ok(conn) = Connection::open(db) else {
        return ("unavailable".into(), "unavailable".into());
    };
    let journal: String = conn
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap_or_else(|_| "unavailable".into());
    let sync: i64 = conn.query_row("PRAGMA synchronous", [], |r| r.get(0)).unwrap_or(-1);
    let name = match sync {
        0 => "0 (OFF)",
        1 => "1 (NORMAL)",
        2 => "2 (FULL)",
        3 => "3 (EXTRA)",
        _ => "unavailable",
    };
    (journal, name.into())
}

/// Write `n` rows through one path, timed. A fresh database per call: the
/// measurement is of a growing store, so it cannot share one.
fn time_write(path: Write3, n: usize) -> Result<Write> {
    // Per-path scratch name: the three paths are timed sequentially against
    // fresh databases, so the slug only has to be unique, not clever.
    let db: PathBuf = std::env::temp_dir().join(format!(
        "mw-write-{}-{}-{n}.db",
        std::process::id(),
        path.name().replace(' ', "_")
    ));
    rm_db(&db);
    let bank = "w";
    let mut lat: Vec<u64> = Vec::with_capacity(n);
    let start = Instant::now();
    // `p50` stays `None` for the batched path, which is the honest answer for one
    // duration; every other path records one latency per row.
    let mut p50 = None;

    match path {
        // The service creates the bank its first retain names, so nothing is
        // seeded here: a seeded row would sit in the timed path where a real
        // first call does not pay for it.
        Write3::ServiceRetain => {
            let svc = MemoryService::new(SqliteStore::open(&db)?);
            for i in 0..n {
                let t = Instant::now();
                svc.retain(bank, &content(i), None)?;
                lat.push(t.elapsed().as_micros() as u64);
            }
            lat.sort_unstable();
            p50 = Some(pct(&lat, 0.50));
        }
        Write3::StorePut => {
            let store = SqliteStore::open(&db)?;
            store.put_bank(&Bank {
                id: bank.into(),
                name: bank.into(),
            })?;
            for i in 0..n {
                let t = Instant::now();
                store.put(&Memory {
                    id: format!("m-{i}"),
                    bank_id: bank.into(),
                    content: content(i),
                    context: Some(format!("session-{}", i / 8)),
                    created_at: None,
                })?;
                lat.push(t.elapsed().as_micros() as u64);
            }
            lat.sort_unstable();
            p50 = Some(pct(&lat, 0.50));
        }
        // One transaction, N rows. The `memories_ai` trigger indexes each row into
        // `memories_fts` inside the same transaction, so this leaves a store in the
        // same shape as the single-row paths rather than an unindexed one.
        Write3::Batched => {
            SqliteStore::open(&db)?.put_bank(&Bank {
                id: bank.into(),
                name: bank.into(),
            })?;
            let conn = Connection::open(&db)?;
            let tx = conn.unchecked_transaction()?;
            {
                let mut stmt = tx.prepare_cached(
                    "INSERT INTO memories (id, bank_id, content, context) VALUES (?1, ?2, ?3, ?4)",
                )?;
                for i in 0..n {
                    stmt.execute(params![
                        format!("m-{i}"),
                        bank,
                        content(i),
                        format!("session-{}", i / 8),
                    ])?;
                }
            }
            tx.commit()?;
        }
    }

    let wall = start.elapsed().as_secs_f64();
    let (db_main, _, _) = db_bytes(&db);
    rm_db(&db);
    Ok(Write {
        path,
        memories: n,
        wall_s: wall,
        ops_lo: n as f64 / wall,
        ops_hi: n as f64 / wall,
        mean_us: wall / n as f64 * 1e6,
        p50_us: p50,
        db_main,
    })
}

/// The same generated content every path writes: short, unique, PII-free. Short
/// so the comparison is about commits rather than bytes, unique so the store's
/// content-hash dedup never short-circuits a write into a no-op.
fn content(i: usize) -> String {
    match i % 4 {
        0 => format!("auth uses jose middleware for jwt verification instance {i}"),
        1 => format!("rate limiting via token bucket algorithm instance {i}"),
        2 => format!("vector search uses pgvector hnsw index instance {i}"),
        _ => format!("distractor note {i} about cafeteria menus and parking rotations"),
    }
}
