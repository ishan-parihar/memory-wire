//! Fusion-weight sweep: does reweighting the two recall streams close the
//! measured `single-session-preference` / `single-session-assistant` reordering
//! deficit? (`docs/NEXT_ITERATION.md`, Phase E1.)
//!
//! Run: `cargo run --release --example sweep_fusion --
//!        --data eval/data/longmemeval_s_cleaned.json [--n 500] [--seed 42]`
//!
//! **A separate harness, not a mode on `longmemeval`, on purpose.** Mixing an
//! exploratory grid into the example that owns `eval/RESULTS.md` puts a
//! twenty-config fan-out one flag away from the reviewed artifact — the exact
//! footgun the temp-`--out-md` rule exists to close, and one that has already
//! destroyed real content here twice. This file is measurement-only: it writes
//! nowhere near `eval/` unless both `--out-md` and `--out-json` are named, it
//! never touches the longmemeval index file, and it takes no flag that can change
//! what a recall does outside the fusion.
//!
//! **One index build per question, every configuration scored on it.** The
//! expensive part of this suite is building a fresh in-memory index of 38–62
//! sessions per question; the retrieval arithmetic is microseconds. So the loop
//! is questions-outer, configurations-inner: all rows of the grid see a
//! byte-identical corpus, and a row cannot disagree with another because its
//! index happened to build differently. It also makes the grid cheap enough to
//! run on every change to the fusion.
//!
//! **Per-question data is kept, not just aggregates.** `--out-json` carries all
//! five retrieval values for every question in every configuration, so any claim
//! in the write-up can be re-derived rather than taken on trust. The aggregate
//! can hold while two categories move in opposite directions — that is how the
//! earlier `detail=none` latency claim ended up unsupported — so the artifact
//! prints the per-category table for *every* row, plus how many individual
//! questions each row improved or regressed against the shipped default.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::time::Instant;

use memory_wire::api::MemoryService;
use memory_wire::memory::{Bank, Memory};
use memory_wire::recall::FusionWeights;
use memory_wire::store::{SqliteStore, Store};
use serde::Deserialize;

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

/// The row the shipped binary is measured against, and the row every other row's
/// per-question diff is taken against. `FusionWeights::SHIPPED` is that binary's
/// configuration, so "Δ vs default" means "what changing the fusion did", not
/// "what a second implementation did".
/// The row every other row is diffed against, and row 0 by construction.
///
/// This is **equal weight, k=60** — the configuration every published number in
/// `README.md`, `eval/RESULTS.md` and the parity table in
/// `docs/NEXT_ITERATION.md` was measured under. It is deliberately *not* the
/// shipped default: E1 moved the shipped `overlap` weight off 1.0, and if the
/// control moved with it then a sweep run today would be diffed against
/// something no committed artifact mentions, and the gate would quietly become
/// "is the new default better than itself".
const CONTROL_LABEL: &str = "control: equal weight, k=60 (published parity baseline)";
/// The configuration the binary actually ships, as a row of its own so the
/// artifact always answers "is the shipped default still beating the published
/// baseline, and by how much".
const SHIPPED_LABEL: &str = "shipped: overlap=0.25, k=60";

fn recall_any(retrieved: &[String], gold: &[String], k: usize) -> f64 {
    let top: HashSet<&str> = retrieved.iter().take(k).map(String::as_str).collect();
    f64::from(gold.iter().any(|g| top.contains(g.as_str())))
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

/// Per-question values, kept so a diff against the default is a real diff.
#[derive(Clone)]
struct Row {
    question_id: String,
    question_type: String,
    r5: f64,
    r10: f64,
    r20: f64,
    ndcg10: f64,
    mrr: f64,
}

#[derive(Default)]
struct Agg {
    count: usize,
    r5: f64,
    r10: f64,
    r20: f64,
    ndcg10: f64,
    mrr: f64,
}

impl Agg {
    fn add(&mut self, r: &Row) {
        self.count += 1;
        self.r5 += r.r5;
        self.r10 += r.r10;
        self.r20 += r.r20;
        self.ndcg10 += r.ndcg10;
        self.mrr += r.mrr;
    }
    fn pct(&self, v: f64) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            v / self.count as f64 * 100.0
        }
    }
    fn r5(&self) -> f64 {
        self.pct(self.r5)
    }
    fn r10(&self) -> f64 {
        self.pct(self.r10)
    }
    fn r20(&self) -> f64 {
        self.pct(self.r20)
    }
    fn ndcg10(&self) -> f64 {
        self.pct(self.ndcg10)
    }
    fn mrr(&self) -> f64 {
        self.pct(self.mrr)
    }
}

fn loadavg() -> String {
    fs::read_to_string("/proc/loadavg")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unavailable".to_string())
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let data = bench_common::arg(&args, "--data", "eval/data/longmemeval_s_cleaned.json");
    let n: usize = bench_common::arg(&args, "--n", "0").parse().unwrap_or(0);
    let seed: u64 = bench_common::arg(&args, "--seed", "42").parse().unwrap_or(42);
    // Both artifacts are temp-defaulted, unlike `longmemeval`'s committed
    // `results.json`: a grid's per-question dump is not the reviewable record
    // for the suite, and two harnesses defaulting to the same committed path is
    // the clobber waiting to happen.
    let out_md = bench_common::out_md(&args, "SWEEP_FUSION.md");
    let out_json = match bench_common::flag(&args, "--out-json") {
        Some(p) => p,
        None => {
            let p = std::env::temp_dir().join("sweep_fusion.json");
            eprintln!("no --out-json given: writing per-question data to {}", p.display());
            p.display().to_string()
        }
    };
    if bench_common::profile() != "release" {
        eprintln!("WARNING: not `--release`; retrieval metrics are unaffected but say so anyway");
    }

    // ---- the grid ----------------------------------------------------------------
    // Kept in one list so the artifact and the stdout are the same table, and so
    // the per-category matrix can never omit a row the aggregate table printed.
    let mut grid: Vec<(String, FusionWeights)> = Vec::new();
    // Row 0 is the control, so the diff baseline is index 0 and always exists.
    grid.push((
        CONTROL_LABEL.to_string(),
        FusionWeights { overlap: 1.0, ..FusionWeights::SHIPPED },
    ));
    grid.push((SHIPPED_LABEL.to_string(), FusionWeights::SHIPPED));
    // A: the overlap weight, BM25 pinned at 1.0 and k at 60.
    let mut w = 0.0;
    while w <= 2.0 + 1e-9 {
        grid.push((
            format!("A: overlap={w:.2}"),
            FusionWeights { overlap: w, ..FusionWeights::SHIPPED },
        ));
        w += 0.25;
    }
    // B: the RRF k constant, at *equal* weights — the same weights the control
    // row has, so this row answers "what would k alone have done to the
    // published baseline". k=60 is deliberately absent: it is the control row,
    // and printing the same configuration twice under two labels invites reading
    // the duplicate as a second independent result.
    for k in [10.0, 20.0, 120.0] {
        grid.push((
            format!("B: equal weight, k={k:.0}"),
            FusionWeights { overlap: 1.0, k, ..FusionWeights::SHIPPED },
        ));
    }
    // C: the cross-stream agreement bonus, as its own section rather than folded
    // into the grid above — it is a different lever (an additive term), not a
    // different weight, and mixing the two axes in one sweep is how a result
    // becomes unattributable. Measured on top of the *shipped* weights, which is
    // where a bonus would actually be applied.
    for a in [0.05, 0.10, 0.25, 0.50] {
        grid.push((
            format!("C: shipped + agreement={a:.2}"),
            FusionWeights { agreement: a, ..FusionWeights::SHIPPED },
        ));
    }
    // D: diagnostics — one stream at a time, which A's endpoints already cover
    // except the overlap-only arm. These bound how much the fusion is doing at
    // all, and are reported as such rather than as candidates.
    grid.push((
        "D: bm25 only".to_string(),
        FusionWeights { overlap: 0.0, ..FusionWeights::SHIPPED },
    ));
    grid.push((
        "D: overlap only".to_string(),
        FusionWeights { bm25: 0.0, ..FusionWeights::SHIPPED },
    ));
    // E: the interaction. B swept k at *equal* weights, which is a configuration
    // this sweep has already shown to be the wrong place to ask the question —
    // k damps the overlap stream's vote, so the k that is right at overlap 1.0
    // need not be the k that is right at the weight the grid actually picked.
    // Both axes act on the same thing (how much a low BM25 rank can be outvoted),
    // so the corner is measured rather than assumed additive.
    for (ov, ks) in [
        (0.25, vec![10.0, 20.0, 40.0, 120.0]),
        (0.50, vec![10.0, 20.0]),
        (0.00, vec![10.0]),
    ] {
        for k in ks {
            grid.push((
                format!("E: overlap={ov:.2}, k={k:.0}"),
                FusionWeights { overlap: ov, k, ..FusionWeights::SHIPPED },
            ));
        }
    }

    // ---- one pass, all configurations ---------------------------------------------
    let raw = fs::read_to_string(&data)?;
    let entries: Vec<Entry> = serde_json::from_str(&raw)?;
    let mut order = bench_common::shuffle(entries.len(), seed);
    if n > 0 && n < order.len() {
        order.truncate(n);
    }
    eprintln!(
        "sweep_fusion: {} questions x {} configurations (seed {seed}); loadavg at start {}",
        order.len(),
        grid.len(),
        loadavg()
    );

    let mut per_config: Vec<(String, FusionWeights, Vec<Row>)> = Vec::new();
    for (i, (label, _)) in grid.iter().enumerate() {
        per_config.push((label.clone(), grid[i].1, Vec::with_capacity(order.len())));
    }
    let mut per_type: Vec<BTreeMap<String, Agg>> = (0..grid.len()).map(|_| BTreeMap::new()).collect();
    let index_start = Instant::now();

    for (qi, &ei) in order.iter().enumerate() {
        let e = &entries[ei];
        let store = SqliteStore::open_in_memory()?;
        store.put_bank(&Bank { id: "eval".into(), name: "eval".into() })?;
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
        // One service, one index, every configuration. `recall_with_weights` is
        // the same code path `recall` takes, with the weights as an argument
        // instead of the shipped constant.
        let svc = MemoryService::new(store);
        let gold: HashSet<String> = e.answer_session_ids.iter().cloned().collect();
        for (ci, (_, weights, rows)) in per_config.iter_mut().enumerate() {
            let hits = svc.recall_with_weights("eval", &e.question, 100_000, weights)?;
            let retrieved: Vec<String> = hits.into_iter().map(|h| h.memory.id).collect();
            let row = Row {
                question_id: e.question_id.clone(),
                question_type: e.question_type.clone(),
                r5: recall_any(&retrieved, &e.answer_session_ids, 5),
                r10: recall_any(&retrieved, &e.answer_session_ids, 10),
                r20: recall_any(&retrieved, &e.answer_session_ids, 20),
                ndcg10: ndcg(&retrieved, &gold, 10),
                mrr: mrr(&retrieved, &gold),
            };
            per_type[ci]
                .entry(row.question_type.clone())
                .or_default()
                .add(&row);
            rows.push(row);
        }
        if (qi + 1) % 50 == 0 {
            eprintln!("  {}/{} ...", qi + 1, order.len());
        }
    }
    let index_secs = index_start.elapsed().as_secs_f64();

    // ---- aggregate + per-question diffs -------------------------------------------
    let totals: Vec<Agg> = per_config
        .iter()
        .map(|(_, _, rows)| {
            let mut a = Agg::default();
            for r in rows {
                a.add(r);
            }
            a
        })
        .collect();
    let base = &per_config[0].2;
    // Per-question movement, per metric. Counted, not summarised, so a row that
    // wins on the mean by losing half its questions and winning the other half
    // big is visible instead of hidden.
    let diffs: Vec<(usize, usize, usize)> = per_config
        .iter()
        .map(|(_, _, rows)| {
            let mut up = 0;
            let mut down = 0;
            let mut same = 0;
            for (r, b) in rows.iter().zip(base.iter()) {
                match r.r5.partial_cmp(&b.r5).unwrap_or(std::cmp::Ordering::Equal) {
                    std::cmp::Ordering::Greater => up += 1,
                    std::cmp::Ordering::Less => down += 1,
                    std::cmp::Ordering::Equal => same += 1,
                }
            }
            (up, down, same)
        })
        .collect();

    let b5 = totals[0].r5();
    let bn = totals[0].ndcg10();
    let b20 = totals[0].r20();
    let mut types: Vec<&String> = per_type[0].keys().collect();
    types.sort();

    // ---- artifact ----------------------------------------------------------------
    let mut md = String::new();
    md.push_str("# Fusion-weight sweep (E1)\n\n");
    md.push_str(&format!(
        "Does reweighting the BM25 and token-overlap streams close the measured \
         `single-session-preference` / `single-session-assistant` reordering \
         deficit? {} questions, seed {seed}, every configuration scored on one \
         shared index per question.\n\n\
         Row 1 is the **control**: equal weight at k=60, the configuration every \
         published number in `README.md`, `eval/RESULTS.md` and the parity table in \
         `docs/NEXT_ITERATION.md` was measured under. It is pinned here rather than \
         following `FusionWeights::SHIPPED`, because a control that moved with the \
         default would stop being comparable to any committed artifact and the gate \
         would quietly become \"is the new default better than itself\". Row 2 is what \
         the binary ships. Every `Δ` is against row 1; a row that does not beat the \
         control is not a candidate.\n\n\
         **Gate.** R@5 and NDCG@10 must rise, R@20 must not fall, and the \
         per-category table below must not show a category moving the other way to \
         pay for it.\n\n\
         Every label states its own configuration. Section **A** holds BM25 at 1.0 \
         and k at 60 and moves `overlap`; **B** holds both weights at 1.0 and moves \
         `k`; **C** adds the agreement bonus to the shipped weights; **D** is one \
         stream at a time, a bound on what the fusion is doing rather than a \
         candidate; **E** is the A×B interaction, measured because both axes act on \
         the same thing (how far a low BM25 rank can be outvoted) and an assumed \
         additivity is how a corner goes unmeasured.\n\n\
         {}\n\n\
         Load at start `{}`, at end `{}`; index build {:.1}s for all {} \
         configurations. Retrieval metrics are load-independent; there is no \
         latency column because nothing here measures wall clock per recall.\n\n",
        totals[0].count,
        bench_common::provenance(),
        loadavg(),
        loadavg(),
        index_secs,
        grid.len(),
    ));

    md.push_str("## Aggregate, every configuration\n\n");
    md.push_str(
        "| Configuration | BM25 | overlap | agree | k | R@5 | ΔR@5 | R@10 | R@20 | ΔR@20 | NDCG@10 | ΔNDCG | MRR | R@5 up/down/same | gate |\n\
         |---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for (i, (label, w)) in grid.iter().enumerate() {
        let t = &totals[i];
        let (up, down, same) = diffs[i];
        let passes = t.r5() > b5 && t.ndcg10() > bn && t.r20() >= b20;
        md.push_str(&format!(
            "| {label} | {:.2} | {:.2} | {:.2} | {:.0} | {:.1}% | {:+.1} | {:.1}% | {:.1}% | {:+.1} | {:.1}% | {:+.1} | {:.1}% | {up}/{down}/{same} | {} |\n",
            w.bm25,
            w.overlap,
            w.agreement,
            w.k,
            t.r5(),
            t.r5() - b5,
            t.r10(),
            t.r20(),
            t.r20() - b20,
            t.ndcg10(),
            t.ndcg10() - bn,
            t.mrr(),
            if passes { "**PASS**" } else { "—" }
        ));
    }
    md.push('\n');

    // Per-category, for every row. Two matrices rather than one table per row:
    // the point of this table is the side-by-side, and 20 near-identical tables
    // is how a per-category regression gets missed.
    // `types` is indexed, not iterated with a `&` pattern that would need a
    // borrow live across the closure body, so each category is read through an
    // owned key.
    let keys: Vec<String> = types.iter().map(|t| (*t).clone()).collect();
    let matrix = |title: &str, f: fn(&Agg) -> f64| {
        let mut s = format!("## {title}, per question type\n\n| Configuration |");
        for t in &keys {
            s.push_str(&format!(" {t} |"));
        }
        s.push_str(" overall |\n|---|");
        for _ in 0..=keys.len() {
            s.push_str("---|");
        }
        s.push('\n');
        let empty = Agg::default();
        for (i, (label, _)) in grid.iter().enumerate() {
            s.push_str(&format!("| {label} |"));
            for t in &keys {
                let a = per_type[i].get(t).unwrap_or(&empty);
                let base_a = per_type[0].get(t).unwrap_or(&empty);
                s.push_str(&format!(" {:.1}% ({:+.1}) |", f(a), f(a) - f(base_a)));
            }
            s.push_str(&format!(" {:.1}% |\n", f(&totals[i])));
        }
        s.push('\n');
        s
    };
    md.push_str(&matrix("R@5", Agg::r5));
    md.push_str(&matrix("R@10", Agg::r10));

    md.push_str("## Per-question data\n\n\
        `--out-json` carries `question_id`, `question_type` and all five retrieval \
        values for every question under every configuration, so any cell above can \
        be re-derived. The `R@5 up/down/same` column in the aggregate table counts \
        individual questions against the control.\n\n");
    md.push_str(&format!(
        "Wrote `{out_md}` and `{out_json}`.\n\n{}",
        bench_common::provenance()
    ));
    fs::write(&out_md, &md)?;

    let json: Vec<serde_json::Value> = per_config
        .iter()
        .map(|(label, w, rows)| {
            serde_json::json!({
                "configuration": label,
                "bm25": w.bm25, "overlap": w.overlap,
                "agreement": w.agreement, "k": w.k,
                "questions": rows.iter().map(|r| serde_json::json!({
                    "question_id": r.question_id,
                    "question_type": r.question_type,
                    "recall_any_at_5": r.r5,
                    "recall_any_at_10": r.r10,
                    "recall_any_at_20": r.r20,
                    "ndcg_at_10": r.ndcg10,
                    "mrr": r.mrr,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    fs::write(&out_json, serde_json::to_string(&json)?)?;

    // ---- stdout ------------------------------------------------------------------
    // The full grid on stdout too: an artifact nobody reads is a log, and this
    // is the table the accept/reject decision is made from.
    println!("{md}");
    let best = (1..grid.len())
        .max_by(|&a, &b| {
            totals[a]
                .r5()
                .partial_cmp(&totals[b].r5())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap();
    println!(
        "best R@5 row: {} (R@5 {:+.1}pp, NDCG@10 {:+.1}pp, R@20 {:+.1}pp); control R@5 {:.1}% NDCG@10 {:.1}%",
        grid[best].0,
        totals[best].r5() - b5,
        totals[best].ndcg10() - bn,
        totals[best].r20() - b20,
        b5,
        bn
    );
    let any_pass = (1..grid.len()).any(|i| totals[i].r5() > b5 && totals[i].ndcg10() > bn && totals[i].r20() >= b20);
    if !any_pass {
        println!("NO CONFIGURATION PASSES THE GATE — equal-weight k=60 stands.");
    }
    eprintln!("wrote {out_md} + {out_json}");
    Ok(())
}
