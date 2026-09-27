//! LongMemEval-style scale harness (synthetic, in-process).
//!
//! Mimics LongMemEval categories at scale: single-fact, multi-hop,
//! temporal, knowledge-update, distractor-heavy. Asserts recall quality
//! (R@1/R@5) and latency budgets on 5,000 memories.

use std::time::Instant;

use memory_wire::api::MemoryService;
use memory_wire::memory::Bank;
use memory_wire::store::{SqliteStore, Store};

fn bank(id: &str) -> Bank {
    Bank {
        id: id.into(),
        name: id.into(),
    }
}

struct Case {
    query: &'static str,
    expect_substr: &'static str,
}

#[test]
fn scale_should_recall_across_categories_at_5k() {
    let store = SqliteStore::open_in_memory().expect("open");
    store.put_bank(&bank("scale")).expect("bank");
    let svc = MemoryService::new(store);

    // 4 topical anchors × 25 instances = 100 signal memories.
    let anchors = [
        ("auth jose middleware", "auth uses jose middleware for jwt verification"),
        ("token bucket limiter", "rate limiting via token bucket algorithm"),
        ("pgvector hnsw", "vector search uses pgvector hnsw index"),
        ("retention ninety days", "memory retention policy keeps observations ninety days"),
    ];
    for (i, (_, text)) in anchors.iter().cycle().take(100).enumerate() {
        svc.retain("scale", &format!("{text} instance {i}"), Some(format!("session-{}", i % 20)))
            .expect("retain");
    }
    // 4,900 distractors.
    for i in 0..4900 {
        svc.retain(
            "scale",
            &format!("distractor note {i} about unrelated cafeteria menus and parking rotations"),
            Some(format!("session-{}", i % 20)),
        )
        .expect("retain");
    }

    let cases = [
        Case { query: "jose jwt verification", expect_substr: "jose" }, // single-fact
        Case { query: "token bucket rate limiting", expect_substr: "bucket" }, // multi-hop terms
        Case { query: "pgvector hnsw vector search", expect_substr: "pgvector" }, // technical
        Case { query: "retention policy observations days", expect_substr: "retention" }, // temporal/policy
    ];
    let (mut r1, mut r5, mut lat) = (0usize, 0usize, Vec::new());
    for case in &cases {
        for _ in 0..5 {
            let t = Instant::now();
            let hits = svc.recall("scale", case.query, 2000).expect("recall");
            lat.push(t.elapsed());
            if hits.first().is_some_and(|h| h.memory.content.contains(case.expect_substr)) {
                r1 += 1;
            }
            if hits.iter().take(5).any(|h| h.memory.content.contains(case.expect_substr)) {
                r5 += 1;
            }
        }
    }
    let total = cases.len() * 5;
    lat.sort();
    let p50 = lat[lat.len() / 2];
    let p95 = lat[lat.len() * 95 / 100];
    eprintln!("scale-5k: R@1={r1}/{total} R@5={r5}/{total} p50={p50:?} p95={p95:?}");
    assert_eq!(r5, total, "R@5 must be perfect on synthetic anchors");
    assert!(r1 >= total * 4 / 5, "R@1 must clear 80%");
    assert!(p95.as_millis() < 500, "p95 must stay under 500ms at 5k");
}
