//! LongMemEval-S retrieval harness (official dataset, retrieval-only).
//!
//! Methodology mirrors `_audit/agentmemory/benchmark/longmemeval-bench.ts` so
//! numbers are comparable: per question, index each haystack session as one
//! memory (id = session id), search with the question text, score
//! session-level recall_any@K / NDCG@10 / MRR against `answer_session_ids`.
//!
//! Dataset (not vendored, 264 MB — see `eval/download.sh`):
//! `https://huggingface.co/datasets/xiaowu0162/longmemeval-cleaned`
//! file `longmemeval_s_cleaned.json` (500 questions, ~48 sessions each).
//!
//! Run: `cargo run --example longmemeval -- --data eval/data/longmemeval_s_cleaned.json [--n 500] [--seed 42]`

use std::collections::{HashMap, HashSet};
use std::fs;
use std::time::Instant;

use memory_wire::api::MemoryService;
use memory_wire::memory::{Bank, Memory};
use memory_wire::store::{SqliteStore, Store};
use serde::Deserialize;

#[derive(Deserialize)]
struct Turn {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct Entry {
    question_id: String,
    question_type: String,
    question: String,
    answer_session_ids: Vec<String>,
    haystack_session_ids: Vec<String>,
    haystack_sessions: Vec<Vec<Turn>>,
}

/// Deterministic LCG shuffle (no extra deps; seed-stable slice like `--seed`).
fn shuffled_indices(len: usize, seed: u64) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..len).collect();
    let mut s = seed.wrapping_add(0x9E3779B97F4A7C15);
    for i in (1..len).rev() {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        idx.swap(i, (s >> 33) as usize % (i + 1));
    }
    idx
}

fn recall_any(retrieved: &[String], gold: &[String], k: usize) -> f64 {
    let top: HashSet<&str> = retrieved.iter().take(k).map(String::as_str).collect();
    if gold.iter().any(|g| top.contains(g.as_str())) {
        1.0
    } else {
        0.0
    }
}

fn dcg(rels: &[bool], k: usize) -> f64 {
    rels.iter()
        .take(k)
        .enumerate()
        .map(|(i, r)| if *r { 1.0 / ((i + 2) as f64).log2() } else { 0.0 })
        .sum()
}

fn ndcg(retrieved: &[String], gold: &HashSet<String>, k: usize) -> f64 {
    let rels: Vec<bool> = retrieved.iter().take(k).map(|id| gold.contains(id)).collect();
    let ideal = dcg(&vec![true; gold.len().min(k)], k);
    if ideal == 0.0 {
        return 0.0;
    }
    dcg(&rels, k) / ideal
}

fn mrr(retrieved: &[String], gold: &HashSet<String>) -> f64 {
    for (i, id) in retrieved.iter().enumerate() {
        if gold.contains(id) {
            return 1.0 / (i + 1) as f64;
        }
    }
    0.0
}

fn arg(name: &str, default: String, args: &[String]) -> String {
    args.windows(2)
        .find(|w| w[0] == name)
        .map(|w| w[1].clone())
        .unwrap_or(default)
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let data = arg("--data", "eval/data/longmemeval_s_cleaned.json".into(), &args);
    let n: usize = arg("--n", "0".into(), &args).parse().unwrap_or(0);
    let seed: u64 = arg("--seed", "42".into(), &args).parse().unwrap_or(42);
    let out_json = arg("--out-json", "eval/results.json".into(), &args);
    let out_md = arg("--out-md", "eval/RESULTS.md".into(), &args);

    let raw = fs::read_to_string(&data)?;
    let entries: Vec<Entry> = serde_json::from_str(&raw)?;
    let mut order = shuffled_indices(entries.len(), seed);
    if n > 0 && n < order.len() {
        order.truncate(n);
    }
    eprintln!("longmemeval: {} questions (seed {seed})", order.len());

    #[derive(Default)]
    struct Agg {
        count: usize,
        r5: f64,
        r10: f64,
        r20: f64,
        ndcg10: f64,
        mrr: f64,
        ms: Vec<u128>,
    }
    let mut total = Agg::default();
    let mut by_type: HashMap<String, Agg> = HashMap::new();
    let mut rows: Vec<serde_json::Value> = Vec::new();

    for (qi, &ei) in order.iter().enumerate() {
        let e = &entries[ei];
        let store = SqliteStore::open_in_memory()?;
        store.put_bank(&Bank {
            id: "eval".into(),
            name: "eval".into(),
        })?;
        for (sid, turns) in e.haystack_session_ids.iter().zip(e.haystack_sessions.iter()) {
            let text: Vec<String> =
                turns.iter().map(|t| format!("{}: {}", t.role, t.content)).collect();
            store.put(&Memory {
                id: sid.clone(),
                bank_id: "eval".into(),
                content: text.join("\n"),
                context: None,
                created_at: None,
            })?;
        }
        let svc = MemoryService::new(store);
        let t = Instant::now();
        let hits = svc.recall("eval", &e.question, 100_000)?;
        let ms = t.elapsed().as_millis();
        let retrieved: Vec<String> = hits.into_iter().map(|h| h.memory.id).collect();
        let gold: HashSet<String> = e.answer_session_ids.iter().cloned().collect();
        let r = serde_json::json!({
            "question_id": e.question_id, "question_type": e.question_type,
            "recall_any_at_5": recall_any(&retrieved, &e.answer_session_ids, 5),
            "recall_any_at_10": recall_any(&retrieved, &e.answer_session_ids, 10),
            "recall_any_at_20": recall_any(&retrieved, &e.answer_session_ids, 20),
            "ndcg_at_10": ndcg(&retrieved, &gold, 10),
            "mrr": mrr(&retrieved, &gold),
            "latency_ms": ms,
        });
        for agg in [Some(&mut total), Some(by_type.entry(e.question_type.clone()).or_default())]
            .into_iter()
            .flatten()
        {
            agg.count += 1;
            agg.r5 += r["recall_any_at_5"].as_f64().unwrap_or(0.0);
            agg.r10 += r["recall_any_at_10"].as_f64().unwrap_or(0.0);
            agg.r20 += r["recall_any_at_20"].as_f64().unwrap_or(0.0);
            agg.ndcg10 += r["ndcg_at_10"].as_f64().unwrap_or(0.0);
            agg.mrr += r["mrr"].as_f64().unwrap_or(0.0);
            agg.ms.push(ms);
        }
        rows.push(r);
        if (qi + 1) % 50 == 0 {
            eprintln!("  {}/{} ...", qi + 1, order.len());
        }
    }

    let pct = |x: f64, c: usize| if c == 0 { 0.0 } else { x / c as f64 * 100.0 };
    let mut md = String::from(
        "# LongMemEval-S retrieval results (memory-wire)\n\nMethodology: per-question fresh index, session-as-document, question-text query — same as agentmemory `longmemeval-bench.ts` (retrieval-only, no LLM judge).\n\n| Slice | R@5 | R@10 | R@20 | NDCG@10 | MRR | p50 ms | n |\n|---|---|---|---|---|---|---|---|\n",
    );
    let mut types: Vec<&String> = by_type.keys().collect();
    types.sort();
    for t in types {
        let a = &by_type[t];
        let mut ms = a.ms.clone();
        ms.sort();
        md.push_str(&format!(
            "| {t} | {:.1}% | {:.1}% | {:.1}% | {:.1}% | {:.1}% | {} | {} |\n",
            pct(a.r5, a.count),
            pct(a.r10, a.count),
            pct(a.r20, a.count),
            pct(a.ndcg10, a.count),
            pct(a.mrr, a.count),
            ms[ms.len() / 2],
            a.count
        ));
    }
    let mut ms = total.ms.clone();
    ms.sort();
    md.push_str(&format!(
        "| **overall** | **{:.1}%** | **{:.1}%** | **{:.1}%** | **{:.1}%** | **{:.1}%** | **{}** | **{}** |\n",
        pct(total.r5, total.count),
        pct(total.r10, total.count),
        pct(total.r20, total.count),
        pct(total.ndcg10, total.count),
        pct(total.mrr, total.count),
        ms[ms.len() / 2],
        total.count
    ));
    fs::write(&out_json, serde_json::to_string(&rows)?)?;
    fs::write(&out_md, &md)?;
    eprintln!("wrote {out_json} + {out_md}");
    Ok(())
}
