//! Scale sweep: index-build, search latency, storage, context tokens vs corpus size.
//!
//! Mirrors agentmemory `benchmark/SCALE.md` (built-in loads ALL memory;
//! agentmemory returns top-k). Sizes: 240 / 1,000 / 5,000 / 10,000 memories
//! at ~8 observations per session.
//!
//! Run: `cargo run --example scale_sweep -- [--out-md eval/SCALE_SWEEP.md]`

use std::fs;
use std::time::Instant;

use memory_wire::api::MemoryService;
use memory_wire::memory::Bank;
use memory_wire::store::{SqliteStore, Store};

const QUERIES: [&str; 4] = [
    "jose middleware jwt verification",
    "token bucket rate limiting",
    "pgvector hnsw vector index",
    "retention policy observations days",
];

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let out_md = args
        .windows(2)
        .find(|w| w[0] == "--out-md")
        .map(|w| w[1].clone())
        .unwrap_or_else(|| "eval/SCALE_SWEEP.md".into());

    let mut md = String::from(
        "# Scale sweep (memory-wire)\n\nMethod mirrors agentmemory `benchmark/SCALE.md`: ~8 obs/session; built-in tokens = full corpus chars/4; agentmemory tokens = top-10 hits chars/4 (constant). DB = file-backed SQLite+FTS5 bytes.\n\n| Memories | Sessions | Index build | Search p50 | DB bytes | Built-in tokens | Top-10 tokens | Savings |\n|---|---|---|---|---|---|---|---|\n",
    );
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
        md.push_str(&format!(
            "| {n} | {} | {build_ms} ms | {} µs | {db_bytes} | {builtin} | {agent} | {:.1}% |\n",
            n / 8,
            lat[lat.len() / 2],
            100.0 * (1.0 - agent as f64 / builtin as f64)
        ));
    }
    fs::write(&out_md, &md)?;
    eprintln!("wrote {out_md}");
    Ok(())
}
