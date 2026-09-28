//! LongMemEval-S retrieval harness (official dataset, retrieval-only).
//!
//! Methodology mirrors `_audit/agentmemory/benchmark/longmemeval-bench.ts` so
//! numbers are comparable: per question, index each haystack session as one
//! memory (id = session id), search with the question text, score
//! session-level recall_any@K / NDCG@10 / MRR against `answer_session_ids`.
//! R@1 is in that set and leads the table: it is the shipped ranking's own
//! first-hit rate, and the gap between it and R@20 is reordering rather than
//! coverage — which is what `docs/PERFORMANCE_PLAN.md` P0 targets, and what the
//! table therefore has to be able to show.
//!
//! Dataset (not vendored, 264 MB — see `eval/download.sh`):
//! `https://huggingface.co/datasets/xiaowu0162/longmemeval-cleaned`
//! file `longmemeval_s_cleaned.json` (500 questions, ~48 sessions each).
//!
//! Run: `cargo run --example longmemeval -- --data eval/data/longmemeval_s_cleaned.json [--n 500] [--seed 42]`
//!
//! This binary **owns the whole of `eval/RESULTS.md`**, methodology paragraph
//! included. It used to emit only the table, so every run silently deleted the
//! curated note underneath it — the note stating that the 200-row candidate pool
//! never binds on this suite, which `docs/CONSISTENCY.md` §6 depends on. The
//! header below is part of the template, and the session-count range in it is
//! computed from the run rather than hand-pinned, so it cannot go stale. A run
//! from `--release` therefore reproduces the committed file apart from the date
//! and the latency column.
//!
//! It also emits the `provisional:` paragraph above the table, because the
//! artifact's whole job is to be readable on its own: the weights these numbers
//! were measured at were chosen by looking at these questions
//! (`docs/EVALUATION_HYGIENE.md` §2.1), so a reader who opens only this file
//! must not be able to read a fitted value as a generalisation estimate. That
//! paragraph is generated here rather than typed into the artifact, because the
//! next run would delete a hand-placed one.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::time::Instant;

use memory_wire::api::MemoryService;
use memory_wire::memory::{Bank, Memory};
use memory_wire::recall::FusionWeights;
use memory_wire::store::{SqliteStore, Store};
use serde::Deserialize;

// The `--out-md` policy (a bare run must not be able to overwrite a committed
// `eval/` artifact) and the build-profile name the provenance line needs.
mod bench_common;

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

/// `2500` -> `2,500`. The number is read by humans, and a missing separator is
/// the kind of thing a later copy-paste "fixes" into something else.
fn group_digits(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let data = arg("--data", "eval/data/longmemeval_s_cleaned.json".into(), &args);
    let n: usize = arg("--n", "0".into(), &args).parse().unwrap_or(0);
    let seed: u64 = arg("--seed", "42".into(), &args).parse().unwrap_or(42);
    let out_json = arg("--out-json", "eval/results.json".into(), &args);
    let out_md = bench_common::out_md(&args, "RESULTS.md");
    if bench_common::profile() != "release" {
        eprintln!(
            "WARNING: this run is not `--release`; the latency column below is not comparable \
             to the committed artifacts"
        );
    }
    let profile_token = if bench_common::profile() == "release" { "release" } else { "debug" };

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
        r1: f64,
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
    // The methodology note quotes the haystack size per question. Measured here
    // rather than pinned, so the note can never contradict the run that wrote it.
    let (mut sess_min, mut sess_max, mut sess_sum) = (usize::MAX, 0usize, 0usize);

    for (qi, &ei) in order.iter().enumerate() {
        let e = &entries[ei];
        let store = SqliteStore::open_in_memory()?;
        let n_sessions = e.haystack_session_ids.len();
        sess_min = sess_min.min(n_sessions);
        sess_max = sess_max.max(n_sessions);
        sess_sum += n_sessions;
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
            "recall_any_at_1": recall_any(&retrieved, &e.answer_session_ids, 1),
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
            agg.r1 += r["recall_any_at_1"].as_f64().unwrap_or(0.0);
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
    let date = chrono::Utc::now().format("%Y-%m-%d");
    let sess_min = if sess_min == usize::MAX { 0 } else { sess_min };
    let sess_mean = if total.count == 0 { 0.0 } else { sess_sum as f64 / total.count as f64 };
    // The weights this run actually used, not a remembered copy of them: the
    // marker is only worth emitting if it tracks the build. Empty when the run
    // did not use a weight that was chosen by looking at the test set.
    let provisional = match bench_common::fitted_weight_note(&FusionWeights::SHIPPED) {
        Some(note) => format!("{note}\n\n"),
        None => String::new(),
    };
    // What R@1 is and is not, in the artifact itself rather than in a doc the
    // reader has to find: it is a first-hit rate, and the run's own R@20 is what
    // makes the shortfall reordering instead of a coverage miss.
    let r1_note = format!(
        "**R@1 is the column to read first**: it is the share of the {n} questions this build ranks the \
         gold session *first* for, and it is the shipped ranking's own first-hit rate rather than a \
         coverage claim. The distance from R@1 up to R@20 is reordering, not matching — R@20 is \
         {r20:.1}%, so the candidate pool already holds the gold row for {at20} of the {n}. Each \
         row's percentage is over that row's own `n`.",
        n = total.count,
        r20 = pct(total.r20, total.count),
        at20 = total.r20.round() as usize,
    );
    let mut md = format!(
        "# LongMemEval-S retrieval results (memory-wire)\n\n\
         {provisional}\
         Methodology: per-question fresh index, session-as-document, question-text query — \
         same as agentmemory `longmemeval-bench.ts` (retrieval-only, no LLM judge).\n\n\
         Run {date} from `--{profile_token}`, seed {seed}, all {n} questions. Each question \
         indexes\n{sess_min}–{sess_max} haystack sessions (mean {sess_mean:.1}), so the 200-row \
         recall candidate pool never\nbinds on this suite — every session is scored on every \
         query, and the suite cannot\ndetect a pool-bound or overlap-scorer regression on \
         its own. The >200-row path is\ncovered by `tests/scale.rs` (5,000 memories) and \
         `eval/SCALE_SWEEP.md`. The\nretrieval metrics are deterministic: re-running this \
         binary reproduced all {values}\nper-question values bit-for-bit. The \
         `p50 ms` column is the exception: it is wall-clock, so it\nmoves with \
         the machine's load (2-3x between the runs recorded here) and it is a \
         record of *this*\nrun, not a property of the build. Do not pin it.\n\n\
         {r1_note}\n\n\
         | Slice | R@1 | R@5 | R@10 | R@20 | NDCG@10 | MRR | p50 ms | n |\n\
         |---|---|---|---|---|---|---|---|---|\n",
        n = total.count,
        // Six per-question values per question, so "reproduced bit-for-bit"
        // keeps counting the file rather than the columns it used to have.
        values = group_digits(total.count * 6),
        provisional = provisional,
        r1_note = r1_note,
    );
    let mut types: Vec<&String> = by_type.keys().collect();
    types.sort();
    for t in types {
        let a = &by_type[t];
        let mut ms = a.ms.clone();
        ms.sort();
        md.push_str(&format!(
            "| {t} | {:.1}% | {:.1}% | {:.1}% | {:.1}% | {:.1}% | {:.1}% | {} | {} |\n",
            pct(a.r1, a.count),
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
        "| **overall** | **{:.1}%** | **{:.1}%** | **{:.1}%** | **{:.1}%** | **{:.1}%** | **{:.1}%** | **{}** | **{}** |\n",
        pct(total.r1, total.count),
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
