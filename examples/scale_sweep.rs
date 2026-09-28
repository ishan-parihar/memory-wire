//! Scale sweep: index-build, search latency, storage, context tokens vs corpus size.
//!
//! Mirrors agentmemory `benchmark/SCALE.md` (built-in loads ALL memory;
//! agentmemory returns top-k). Sizes: 240 / 1,000 / 5,000 / 10,000 memories
//! at ~8 observations per session.
//!
//! Run: `cargo run --example scale_sweep -- [--out-md eval/SCALE_SWEEP.md]`
//!
//! `--out-md` has no `eval/` default: a bare run writes to a scratch file under
//! `$TMPDIR`, so this harness cannot overwrite a committed artifact.

use std::fs;
use std::time::Instant;

use memory_wire::api::MemoryService;
use memory_wire::memory::Bank;
use memory_wire::store::{SqliteStore, Store};

// The `--out-md` policy: a bare run must not be able to overwrite a committed
// `eval/` artifact.
mod bench_common;

const QUERIES: [&str; 4] = [
    "jose middleware jwt verification",
    "token bucket rate limiting",
    "pgvector hnsw vector index",
    "retention policy observations days",
];

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let out_md = bench_common::out_md(&args, "SCALE_SWEEP.md");

    // Assembled in order at the end: header, provenance, table, then the notes that
    // belong after it. This note used to exist only in the committed artifact, which
    // meant a bare run deleted it — the same defect `longmemeval.rs` had.
    let mut head = format!(
        "# Scale sweep (memory-wire)\n\nMethod mirrors agentmemory `benchmark/SCALE.md`: ~8 obs/session; built-in tokens = full corpus chars/4; agentmemory tokens = top-10 hits chars/4 (constant). DB = file-backed SQLite+FTS5 bytes.\n\nRun {} from `--profile={}`.\n\n**`DB bytes` is the main SQLite file only.** The store runs in WAL mode, so while the process holds the database open most pages are still in `-wal` and the real on-disk footprint is larger than the column below. Both halves of that measurement belong to `eval/BENCH_FOOTPRINT.md`, which reports the WAL-unflushed and the post-`wal_checkpoint(TRUNCATE)` figures on every run; they are deliberately not repeated here, because a second copy of a byte count is a second thing to go stale.\n\n| Memories | Sessions | Index build | Search p50 | DB bytes | Built-in tokens | Top-10 tokens | Savings |\n|---|---|---|---|---|---|---|---|\n",
        chrono::Utc::now().format("%Y-%m-%d"),
        bench_common::profile(),
    );
    let tail = "Search p50 is 20 recalls (4 queries × 5) per row, so treat single-digit-percent movement between runs as noise. **Index build and search p50 are wall-clock**: both move 2-3x with the machine's load and neither is a property of the build to be pinned. Token-savings is the share of the whole corpus the top-10 window replaces, so it is a property of the corpus shape, not of the index.\n";
    let mut md = String::new();
    for n in [240usize, 1000, 5000, 10000] {
        let dir = std::env::temp_dir().join(format!("mw-sweep-{n}.db"));
        let _ = fs::remove_file(&dir);
        let t = Instant::now();
        let store = SqliteStore::open(&dir)?;
        store.put_bank(&Bank { id: "s".into(), name: "s".into() })?;
        let mut corpus_chars = 0usize;
        for i in 0..n {
            let c = match i % 4 {
                0 => format!("auth uses jose middleware for jwt verification instance {i}"),
                1 => format!("rate limiting via token bucket algorithm instance {i}"),
                2 => format!("vector search uses pgvector hnsw index instance {i}"),
                _ => format!("distractor note {i} about cafeteria menus and parking rotations"),
            };
            corpus_chars += c.len();
            store.put(&memory_wire::memory::Memory {
                id: format!("m-{i}"),
                bank_id: "s".into(),
                content: c,
                context: Some(format!("session-{}", i / 8)),
                created_at: None,
            })?;
        }
        let build_ms = t.elapsed().as_millis();
        let svc = MemoryService::new(store);
        let mut lat = Vec::new();
        let mut top10_chars = 0usize;
        for q in QUERIES {
            for _ in 0..5 {
                let t = Instant::now();
                let hits = svc.recall("s", q, 2000)?;
                lat.push(t.elapsed().as_micros());
                if top10_chars == 0 {
                    top10_chars = hits.iter().take(10).map(|h| h.memory.content.len()).sum();
                }
            }
        }
        lat.sort();
        let db_bytes = fs::metadata(&dir).map(|m| m.len()).unwrap_or(0);
        let _ = fs::remove_file(&dir);
        let builtin = corpus_chars / 4;
        let agent = top10_chars / 4;
        head.push_str(&format!(
            "| {n} | {} | {build_ms} ms | {} µs | {db_bytes} | {builtin} | {agent} | {:.1}% |\n",
            n / 8,
            lat[lat.len() / 2],
            100.0 * (1.0 - agent as f64 / builtin as f64)
        ));
    }
    md.push_str(&head);
    md.push('\n');
    md.push_str(tail);
    fs::write(&out_md, &md)?;
    eprintln!("wrote {out_md}");
    Ok(())
}
