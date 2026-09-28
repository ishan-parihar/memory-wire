//! coding-life eval: grep baseline vs memory-wire on a labeled coding dataset.
//!
//! Dataset vendored from `_audit/agentmemory/eval/data/coding-agent-life-v1`
//! (15 sessions, 15 queries with `goldSessionIds`; vendored from competitor agentmemory, Apache-2.0).
//! Scoring mirrors their `eval/runner/score.ts`: P@k / R@k / hit-rate / p50,
//! aggregated by adapter and by question type.
//!
//! Run: `cargo run --example coding_life -- [--k 5] [--out-md eval/CODING_LIFE.md]`
//!
//! `--out-md` has no `eval/` default: a bare run writes to a scratch file under
//! `$TMPDIR`, so this harness cannot overwrite a committed artifact.

use std::collections::HashSet;
use std::fs;
use std::time::Instant;

use memory_wire::api::MemoryService;
use memory_wire::memory::{Bank, Memory};
use memory_wire::store::{SqliteStore, Store};
use serde::Deserialize;

mod bench_common;

#[derive(Deserialize)]
struct Session {
    id: String,
    content: String,
}

#[derive(Deserialize)]
struct Query {
    id: String,
    #[serde(rename = "type")]
    qtype: String,
    question: String,
    #[serde(rename = "goldSessionIds")]
    gold_session_ids: Vec<String>,
}

/// Case-insensitive substring-match rank (grep baseline adapter).
fn grep_rank(question: &str, sessions: &[Session]) -> Vec<String> {
    let toks: Vec<String> = question
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 3)
        .map(|t| t.to_lowercase())
        .collect();
    let mut scored: Vec<(usize, &str)> = sessions
        .iter()
        .map(|s| {
            let body = s.content.to_lowercase();
            (toks.iter().filter(|t| body.contains(t.as_str())).count(), s.id.as_str())
        })
        .filter(|(c, _)| *c > 0)
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
    scored.into_iter().map(|(_, id)| id.to_string()).collect()
}

fn arg(name: &str, default: String, args: &[String]) -> String {
    args.windows(2)
        .find(|w| w[0] == name)
        .map(|w| w[1].clone())
        .unwrap_or(default)
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let k: usize = arg("--k", "5".into(), &args).parse().unwrap_or(5);
    let out_md = bench_common::out_md(&args, "CODING_LIFE.md");

    let sessions: Vec<Session> =
        serde_json::from_str(&fs::read_to_string("eval/data/sessions.json")?)?;
    let queries: Vec<Query> =
        serde_json::from_str(&fs::read_to_string("eval/data/queries.json")?)?;

    // Index once for memory-wire.
    let store = SqliteStore::open_in_memory()?;
    store.put_bank(&Bank { id: "life".into(), name: "life".into() })?;
    for s in &sessions {
        store.put(&Memory {
            id: s.id.clone(),
            bank_id: "life".into(),
            content: s.content.clone(),
            context: None,
            created_at: None,
        })?;
    }
    let svc = MemoryService::new(store);

    #[derive(Default)]
    struct Agg {
        n: usize,
        p: f64,
        r: f64,
        hit: usize,
        lat: Vec<u128>,
    }
    let mut adapters: std::collections::HashMap<String, Agg> = Default::default();
    let mut by_type: std::collections::HashMap<(String, String), Agg> = Default::default();

    let mut misses: Vec<&str> = Vec::new();
    for q in &queries {
        let gold: HashSet<&str> = q.gold_session_ids.iter().map(String::as_str).collect();
        // grep adapter
        let t = Instant::now();
        let ranked = grep_rank(&q.question, &sessions);
        let ms = t.elapsed().as_micros();
        let hits = ranked.iter().take(k).filter(|id| gold.contains(id.as_str())).count();
        let a = adapters.entry("grep".into()).or_default();
        a.n += 1;
        a.p += hits as f64 / k as f64;
        a.r += if gold.is_empty() { 0.0 } else { hits as f64 / gold.len() as f64 };
        a.hit += usize::from(hits > 0);
        a.lat.push(ms);
        let bt = by_type.entry(("grep".into(), q.qtype.clone())).or_default();
        bt.n += 1;
        bt.p += hits as f64 / k as f64;
        bt.r += if gold.is_empty() { 0.0 } else { hits as f64 / gold.len() as f64 };
        bt.hit += usize::from(hits > 0);
        // memory-wire adapter
        let t = Instant::now();
        let hits_mw = svc.recall("life", &q.question, 2000)?;
        let ms = t.elapsed().as_micros();
        let ranked_mw: Vec<String> = hits_mw.into_iter().map(|h| h.memory.id).collect();
        let hits = ranked_mw.iter().take(k).filter(|id| gold.contains(id.as_str())).count();
        let a = adapters.entry("memory-wire".into()).or_default();
        a.n += 1;
        a.p += hits as f64 / k as f64;
        a.r += if gold.is_empty() { 0.0 } else { hits as f64 / gold.len() as f64 };
        a.hit += usize::from(hits > 0);
        a.lat.push(ms);
        let bt = by_type.entry(("memory-wire".into(), q.qtype.clone())).or_default();
        bt.n += 1;
        bt.p += hits as f64 / k as f64;
        bt.r += if gold.is_empty() { 0.0 } else { hits as f64 / gold.len() as f64 };
        bt.hit += usize::from(hits > 0);
        if hits == 0 {
            misses.push(q.id.as_str());
        }
    }
    if !misses.is_empty() {
        eprintln!("memory-wire misses: {}", misses.join(", "));
    }

    let mut md = format!(
        "# coding-life eval (memory-wire)\n\nDataset: vendored `eval/data/{{sessions,queries}}.json` (15 sessions, 15 labeled queries, from agentmemory `eval/data/coding-agent-life-v1`). Scoring mirrors their `eval/runner/score.ts`. k={k}.\n\nRun {} from `--profile={}`.\n\nThe corpus is 15 sessions, so the 200-row recall candidate pool cannot bind here and every session is scored on every query. P@{k} / R@{k} / hit rate are deterministic and are what this suite gates on. **The p50 latency column is not.** It is a {k}-sample median over one run of one machine: it has been measured across release runs of this binary between roughly 260 and 620 us, so it moves several-fold with the box's load and must be quoted as a range, never as a regression signal. Re-run it; do not pin it.\n\n| Adapter | P@{k} | R@{k} | Hit rate | p50 latency | n |\n|---|---|---|---|---|---|\n",
        chrono::Utc::now().format("%Y-%m-%d"),
        bench_common::profile(),
    );
    for name in ["memory-wire", "grep"] {
        let a = &adapters[name];
        let mut lat = a.lat.clone();
        lat.sort();
        md.push_str(&format!(
            "| {name} | {:.1}% | {:.1}% | {:.1}% | {} µs | {} |\n",
            a.p / a.n as f64 * 100.0,
            a.r / a.n as f64 * 100.0,
            a.hit as f64 / a.n as f64 * 100.0,
            lat[lat.len() / 2],
            a.n
        ));
    }
    fs::write(&out_md, &md)?;
    eprintln!("wrote {out_md}");
    Ok(())
}
