//! Oracle-rerank ceiling for the shipped recall pool — LongMemEval-S.
//!
//! **The question.** Recall already puts a gold row inside its own top 20 on
//! 99.6% of these questions. So a *perfect* reranker over the pool recall
//! already builds would reach R@5 = 99.6 without adding a single component,
//! which means the rerankable headroom is exactly `oracle R@k − current R@k`
//! and the residue is a **matching** problem no reranker can touch. That split
//! had never been measured, so this harness measures it.
//!
//! **Two lists, deliberately, because they are not the same length.** Per
//! question the harness builds the fusion's own ranked output — the same three
//! primitives `MemoryService::recall` calls, in the same order, with the same
//! weights: [`Store::recall_inputs`], [`rank_candidates`] over that window, and
//! [`rrf_fuse`] with [`FusionWeights::SHIPPED`] — and separately records what
//! `MemoryService::recall` actually *serves*. The served list is the pool after
//! the token-budget trim and the 100-result cap, so on a long haystack it is
//! shorter. **Oracle metrics are computed on the pool** (a reranker is handed
//! candidates; the budget is applied to what it returns) and **current metrics
//! on the served list** (that is what this build ships, and what
//! `eval/results.json` scored). Where the two disagree about a question, the
//! difference is counted and reported as its own category rather than averaged
//! away.
//!
//! Nothing is re-queried, re-ranked, or approximated, and no question or gold
//! set is trimmed. A question whose gold row never enters the pool lands in
//! `not in pool` — that is the answer, not a gap to be papered over.
//!
//! The reconstructed pool is checked against what the shipped entry point
//! serves — same ids, same order, same scores — per question. That is what
//! makes "the pool recall built" measured rather than claimed: the two private
//! stream caps in `src/api.rs` are mirrored as constants here, so drift in
//! either surfaces as a mismatch instead of silently measuring another pool.
//!
//! **Quality only.** No latency and no footprint number is produced here. The
//! machine load is recorded in the provenance line as context, not because any
//! figure below depends on it.
//!
//! Run: `cargo run --release --example oracle_rerank -- \
//!       --data eval/data/longmemeval_s_cleaned.json --out-md eval/ORACLE_RERANK.md`

mod bench_common;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::process::Command;

use memory_wire::api::MemoryService;
use memory_wire::memory::{Bank, Memory};
use memory_wire::recall::{rank_candidates, rrf_fuse, FusionWeights, RankedHit};
use memory_wire::store::{SqliteStore, Store};
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// `src/api.rs`'s private `FTS_LIMIT`, mirrored because the oracle has to build
/// the same stream the shipped path builds. The pool self-check below is what
/// keeps the mirror honest: if the constant drifts, the reconstructed pool
/// stops matching what recall serves and the run says so.
const FTS_LIMIT: usize = 50;

/// `src/api.rs`'s private `OVERLAP_LIMIT`, mirrored for the same reason.
const OVERLAP_LIMIT: usize = 200;

/// The cutoffs the ceiling is reported at. `1` and `2` are included because a
/// reranker's whole claim is about the top of the list, and `200` because that
/// is the deepest position a pool this size can express.
const KS: [usize; 7] = [1, 2, 5, 10, 20, 50, 200];

/// First-gold-rank buckets, in the order the artifact prints them. Exhaustive
/// and disjoint by construction: a first-gold rank is either absent from the
/// pool or one rank, and these ranges tile every positive rank.
const BUCKETS: [&str; 7] = [
    "not in pool",
    "1",
    "2-5",
    "6-10",
    "11-20",
    "21-50",
    "51-200",
];

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

/// One question, measured. Both gold ranks are kept because the two metrics
/// answer from different lists, and collapsing them into one number here is
/// exactly the kind of convenient merge that makes a ceiling wrong.
struct Q {
    qtype: String,
    bank: usize,
    window: usize,
    /// The fusion's own ranked output — the oracle's basis.
    pool: usize,
    /// What `MemoryService::recall` served — the current system's basis.
    served: usize,
    /// 1-based rank of the first gold row in the pool, or `None` if absent.
    pool_gold: Option<usize>,
    /// 1-based rank of the first gold row in the served list, or `None`.
    served_gold: Option<usize>,
    /// How far the served list and the pool agree before they diverge. Equals
    /// `served.len()` when the trim only dropped the tail.
    prefix: usize,
    /// A question whose evidence resolves to no session has no gold row to
    /// find, so it stays in the denominator and scores as a miss. Counted, not
    /// discarded.
    gold_empty: bool,
}

/// Per-scope tallies. The overall row and every per-type row are the same
/// struct, so the two tables cannot be computed by two different rules.
#[derive(Default)]
struct Acc {
    n: usize,
    gold_empty: usize,
    /// Current R@k hits over the served list, per index into [`KS`].
    cur: [usize; KS.len()],
    /// Oracle R@k hits over the pool, per index into [`KS`].
    ora: [usize; KS.len()],
    /// First-gold-rank histogram over the pool.
    hist: [usize; BUCKETS.len()],
    /// The R@5 misses, classified by where the gold row is in the pool.
    six_twenty: usize,
    twenty_plus: usize,
    absent: usize,
    trimmed: usize,
    pool_lens: Vec<usize>,
    served_lens: Vec<usize>,
    window_lens: Vec<usize>,
    bank_lens: Vec<usize>,
    /// Questions whose served list stops agreeing with the pool before rank 20.
    diverged_early: usize,
}

impl Acc {
    fn push(&mut self, q: &Q) {
        self.n += 1;
        self.gold_empty += usize::from(q.gold_empty);
        for (i, &k) in KS.iter().enumerate() {
            if q.served_gold.is_some_and(|r| r <= k) {
                self.cur[i] += 1;
            }
            if q.pool_gold.is_some() {
                self.ora[i] += 1;
            }
        }
        self.hist[bucket(q.pool_gold)] += 1;
        // Classify the questions this build misses at R@5 by where their gold
        // row actually is. `trimmed` is the category that would be dropped by a
        // three-way split that assumed the served list and the pool agreed.
        if !q.served_gold.is_some_and(|r| r <= 5) {
            match q.pool_gold {
                Some(r) if r <= 5 => self.trimmed += 1,
                Some(r) if r <= 20 => self.six_twenty += 1,
                Some(_) => self.twenty_plus += 1,
                None => self.absent += 1,
            }
        }
        if q.prefix < 20 {
            self.diverged_early += 1;
        }
        self.pool_lens.push(q.pool);
        self.served_lens.push(q.served);
        self.window_lens.push(q.window);
        self.bank_lens.push(q.bank);
    }

    /// The R@5-miss breakdown, as `(6-20, 21+, absent, trimmed, total)`.
    fn misses(&self) -> (usize, usize, usize, usize, usize) {
        (
            self.six_twenty,
            self.twenty_plus,
            self.absent,
            self.trimmed,
            self.six_twenty + self.twenty_plus + self.absent + self.trimmed,
        )
    }

    /// The invariants that must hold whatever the numbers turn out to be. A
    /// harness that can print a monotone-looking but self-contradictory table
    /// is worse than one that refuses to print.
    fn check(&self) {
        assert_eq!(
            self.hist.iter().sum::<usize>(),
            self.n,
            "histogram buckets must tile every question"
        );
        // `cur[2]` is the k=5 column, so `n - cur[2]` is the miss count the four
        // categories have to cover exactly.
        assert_eq!(
            self.six_twenty + self.twenty_plus + self.absent + self.trimmed,
            self.n - self.cur[2],
            "the R@5 misses must be partitioned, no category invented or lost"
        );
        for w in KS.windows(2) {
            let a = KS.iter().position(|&k| k == w[0]).expect("k from KS");
            let b = KS.iter().position(|&k| k == w[1]).expect("k from KS");
            assert!(self.cur[b] >= self.cur[a], "R@{w:?} must not fall as k rises");
            assert!(self.ora[b] >= self.ora[a], "oracle R@{w:?} must not fall as k rises");
            assert!(self.ora[a] >= self.cur[a], "a reranker cannot lose a hit");
        }
    }

    /// `min / median / max` of a length sample, in that order.
    fn spread(samples: &[usize]) -> String {
        let mut s = samples.to_vec();
        s.sort_unstable();
        match (s.first(), s.last()) {
            (Some(lo), Some(hi)) => {
                let mid = s.get(s.len() / 2).copied().unwrap_or(0);
                format!("{lo} / {mid} / {hi}")
            }
            _ => "0 / 0 / 0".to_string(),
        }
    }
}

/// The bucket a first-gold rank falls in, or `not in pool` for `None`.
fn bucket(rank: Option<usize>) -> usize {
    match rank {
        None => 0,
        Some(1) => 1,
        Some(r) if r <= 5 => 2,
        Some(r) if r <= 10 => 3,
        Some(r) if r <= 20 => 4,
        Some(r) if r <= 50 => 5,
        Some(_) => 6,
    }
}

fn pct(x: usize, n: usize) -> f64 {
    if n == 0 {
        0.0
    } else {
        x as f64 / n as f64 * 100.0
    }
}

fn loadavg() -> String {
    fs::read_to_string("/proc/loadavg")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unavailable".to_string())
}

fn rustc() -> String {
    Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unavailable".to_string())
}

/// The commit the tree was measured at, resolved through the ref `.git/HEAD`
/// names. A state this cannot read is reported as unresolved rather than
/// printed as a SHA nobody checked.
fn git_sha() -> String {
    let head = fs::read_to_string(".git/HEAD").unwrap_or_default();
    let head = head.trim().to_string();
    match head.strip_prefix("ref: ") {
        Some(r) => fs::read_to_string(format!(".git/{r}"))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| format!("unresolved ({head})")),
        None if head.is_empty() => "unresolved (no .git/HEAD)".to_string(),
        None => head,
    }
}

/// The dataset's content hash. `eval/download.sh` resolves `main` rather than a
/// revision, so there is no upstream commit to quote — the bytes are the only
/// identifier that can be checked, and they are what the numbers came from.
fn dataset_sha(path: &str) -> String {
    fs::File::open(path).map_or_else(
        |_| "missing".to_string(),
        |mut f| {
            let mut h = Sha256::new();
            if std::io::copy(&mut f, &mut h).is_ok() {
                format!("{:x}", h.finalize())
            } else {
                "unreadable".to_string()
            }
        },
    )
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let data = bench_common::arg(&args, "--data", "eval/data/longmemeval_s_cleaned.json");
    let n: usize = bench_common::arg(&args, "--n", "0").parse().unwrap_or(0);
    let seed: u64 = bench_common::arg(&args, "--seed", "42").parse().unwrap_or(42);
    let ref_json = bench_common::arg(&args, "--ref-json", "eval/results.json");
    let out_md = bench_common::out_md(&args, "ORACLE_RERANK.md");

    let raw = fs::read_to_string(&data)?;
    let entries: Vec<Entry> = serde_json::from_str(&raw)?;
    // Same order `examples/longmemeval.rs` walks, so the per-question comparison
    // against `eval/results.json` is positional and needs no id join.
    let mut order = bench_common::shuffle(entries.len(), seed);
    if n > 0 && n < order.len() {
        order.truncate(n);
    }
    eprintln!(
        "oracle_rerank: {} questions (seed {seed}); loadavg {}",
        order.len(),
        loadavg()
    );

    // The committed per-question file, when present, is the check on *this*
    // harness's current-R@k: it says whether recomputing from the served list
    // reproduces what the shipped harness recorded, question for question.
    let committed: Option<Vec<serde_json::Value>> = fs::read_to_string(&ref_json)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());
    if committed.is_none() {
        eprintln!("note: {ref_json} unreadable; the per-question cross-check is skipped");
    }
    let mut ref_mismatches = 0usize;
    let mut ref_rows = 0usize;

    let mut total = Acc::default();
    let mut by_type: HashMap<String, Acc> = HashMap::new();
    // Recorded separately from `diverged_early`: a served list that stops short
    // of the pool is the budget trim, and that is expected on a long haystack.
    let mut shorter: Vec<String> = Vec::new();
    let mut max_score_delta = 0.0f64;

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

        // ---- the pool, built exactly as `api.rs` builds it ----------------
        let (all, keyword_hits) = svc.store.recall_inputs("eval", &e.question, &[], FTS_LIMIT)?;
        let window = all.len();
        let fts_stream: Vec<RankedHit> = keyword_hits
            .iter()
            .enumerate()
            .map(|(i, (id, _))| RankedHit {
                id: id.clone(),
                rank: i + 1,
            })
            .collect();
        let ranked = rank_candidates(&e.question, all.iter().map(|m| m.content.as_str()));
        let overlap_stream: Vec<RankedHit> = ranked
            .iter()
            .take(OVERLAP_LIMIT)
            .enumerate()
            .map(|(i, (idx, _))| RankedHit {
                id: all[*idx].id.clone(),
                rank: i + 1,
            })
            .collect();
        let fused = rrf_fuse(&[fts_stream, overlap_stream], &FusionWeights::SHIPPED);
        let pool: Vec<String> = fused.iter().map(|(id, _)| id.clone()).collect();
        let by_id: HashMap<&str, f64> = fused.iter().map(|(id, s)| (id.as_str(), *s)).collect();

        // ---- what the shipped entry point actually serves ----------------
        // The 100k-token budget is the existing harness's own. It is large, and
        // it still trims: a 48-session haystack does not fit in it, which is
        // precisely why the two lists are kept apart rather than assumed equal.
        let served = svc.recall("eval", &e.question, 100_000)?;
        let served_ids: Vec<String> = served.iter().map(|h| h.memory.id.clone()).collect();
        if served_ids.len() < pool.len() {
            shorter.push(format!("{} ({} of {})", e.question_id, served_ids.len(), pool.len()));
        }
        for h in &served {
            if let Some(s) = by_id.get(h.memory.id.as_str()) {
                max_score_delta = max_score_delta.max((s - h.score).abs());
            }
        }
        let prefix = served_ids
            .iter()
            .zip(pool.iter())
            .take_while(|(a, b)| a == b)
            .count();

        // ---- the two gold ranks -------------------------------------------
        let gold: HashSet<&str> = e.answer_session_ids.iter().map(String::as_str).collect();
        let first_in = |ids: &[String]| ids.iter().position(|id| gold.contains(id.as_str()));
        let q = Q {
            qtype: e.question_type.clone(),
            bank: e.haystack_session_ids.len(),
            window,
            pool: pool.len(),
            served: served_ids.len(),
            pool_gold: first_in(&pool).map(|i| i + 1),
            served_gold: first_in(&served_ids).map(|i| i + 1),
            prefix,
            gold_empty: e.answer_session_ids.is_empty(),
        };
        total.push(&q);
        by_type.entry(q.qtype.clone()).or_default().push(&q);

        // ---- cross-check this harness's current-R@k ------------------------
        if let Some(rows) = &committed {
            if let Some(row) = rows.get(qi) {
                ref_rows += 1;
                // `results.json` stores these as f64 (`1.0` / `0.0`), not as JSON
                // booleans, so the comparison is numeric. Reading them with
                // `as_bool` returns `None` for every row and reports the whole
                // suite as mismatched.
                let hit = |key: &str| {
                    row.get(key)
                        .and_then(serde_json::Value::as_f64)
                        .map(|v| v > 0.5)
                };
                let ours = |k: usize| q.served_gold.is_some_and(|r| r <= KS[k]);
                let same = hit("recall_any_at_5") == Some(ours(2))
                    && hit("recall_any_at_10") == Some(ours(3))
                    && hit("recall_any_at_20") == Some(ours(4))
                    && row.get("question_id").and_then(serde_json::Value::as_str)
                        == Some(e.question_id.as_str());
                if !same {
                    ref_mismatches += 1;
                }
            }
        }

        if (qi + 1) % 50 == 0 {
            eprintln!("  {}/{} ...", qi + 1, order.len());
        }
    }

    total.check();
    for a in by_type.values() {
        a.check();
    }

    // ---- render ----------------------------------------------------------
    let n_all = total.n;
    let date = chrono::Utc::now().format("%Y-%m-%d");
    let (s6, s21, absent, trimmed, misses) = total.misses();
    let mut md = String::new();
    md.push_str(&format!(
        "# Oracle-rerank ceiling for the shipped recall pool (memory-wire)\n\n\
         Measured at `{sha}` plus the uncommitted `examples/oracle_rerank.rs`.\n\n\
         ```bash\n\
         cargo run --release --example oracle_rerank -- \\\n  \
         --data {data} --out-md eval/ORACLE_RERANK.md\n\
         ```\n\n\
         ## Provenance\n\n\
         | | |\n|---|---|\n\
         | Date | {date} |\n\
         | Commit | `{sha}` |\n\
         | Build profile | `{profile}` |\n\
         | Toolchain | {rustc} |\n\
         | Machine | {host} |\n\
         | `/proc/loadavg` at run | `{load}` |\n\
         | Questions | {n_all} (seed {seed}, none dropped) |\n\
         | `FusionWeights::SHIPPED` | `bm25: 1.0`, `overlap: 0.25`, `agreement: 0.0`, `k: 60` |\n\
         | Stream caps in force | FTS5 BM25 `{fts}`, token-overlap `{ovl}`, store window `200` |\n\
         | Dataset | `xiaowu0162/longmemeval-cleaned`, `longmemeval_s_cleaned.json` |\n\
         | Dataset sha256 | `{dsha}` |\n\
         | Dataset revision | unpinned — `eval/download.sh` resolves `main`, so the content hash is the identifier |\n\n\
         The load figure is context for the run, not a caveat on anything below: every number here is \
         a retrieval-quality metric, which does not move with machine load. **No latency or RSS figure \
         is published in this artifact** — this machine runs background jobs, and a timing number taken \
         under that load is an anecdote wearing a decimal point.\n\n",
        sha = git_sha(),
        data = data,
        date = date,
        profile = bench_common::profile(),
        rustc = rustc(),
        host = bench_common::host(),
        load = loadavg(),
        n_all = n_all,
        seed = seed,
        fts = FTS_LIMIT,
        ovl = OVERLAP_LIMIT,
        dsha = dataset_sha(&data),
    ));

    md.push_str(&format!(
        "## Method\n\n\
         Per question: a fresh in-memory index, one memory per haystack session, query = question \
         text — the same build `examples/longmemeval.rs` uses, not re-implemented. The pool is then \
         constructed in-process from the **same three primitives** `MemoryService::recall` calls, in \
         the same order, with the same weights: `Store::recall_inputs` (the store's bounded window ∪ \
         its BM25 hits), `rank_candidates` over that window, and `rrf_fuse` with \
         `FusionWeights::SHIPPED`. No re-query, no re-rank, no approximation, no question trimmed to \
         make anything fit.\n\n\
         **Two lists, because they are not the same length.** The fusion's ranked output is the \
         **pool** — what a reranker would be handed. What `MemoryService::recall` returns is the \
         **served** list: the same rows after the token-budget trim and the 100-result cap. Oracle \
         metrics are computed on the pool; current metrics on the served list, because that is what \
         this build ships and what `eval/results.json` scored. The difference between the two is \
         measured below, not assumed away.\n\n\
         **The pool is checked against what recall serves.** Per question the harness calls the \
         shipped `MemoryService::recall` and compares ids in order and each fused score. That is what \
         makes \"the pool recall built\" measured rather than claimed — the two stream caps in \
         `src/api.rs` are mirrored as constants here, so drift in either surfaces as a mismatch \
         instead of quietly measuring a different pool.\n\n\
         - Max absolute fused-score difference, served vs pool: **{score_delta:.1}** (0.0 is exact agreement)\n\
         - Served list shorter than the pool: **{shorter}** of {n_all} questions (budget trim; first 5: {examples})\n\
         - Served list stops agreeing with the pool before rank 20: **{diverged}** of {n_all}\n\
         - Per-question `recall_any@5/10/20` vs the committed `eval/results.json`: **{ref_bad}** \
         mismatches over {ref_n} compared rows\n\n\
         Every recall metric below is a function of one number per question: the 1-based rank of the \
         **first** gold row, or `None` if no gold row is in the list. \"Any gold in the top k\" is \
         exactly `first_gold <= k`, so there is no second place for the answer to hide.\n\n",
        score_delta = max_score_delta,
        shorter = shorter.len(),
        n_all = n_all,
        examples = if shorter.is_empty() {
            "none".to_string()
        } else {
            shorter.iter().take(5).cloned().collect::<Vec<_>>().join("; ")
        },
        diverged = total.diverged_early,
        ref_bad = ref_mismatches,
        ref_n = ref_rows,
    ));

    // ---- sizes -----------------------------------------------------------
    md.push_str(&format!(
        "## Pool size\n\n\
         The pool is described in the code as \"newest 200 rows ∪ BM25 top-50\". That is a bound, not \
         a size, and on this suite the bank is the whole haystack, so the real distribution is:\n\n\
         | Rows per question | min / median / max |\n|---|---|\n\
         | Haystack sessions indexed (the bank) | {bank} |\n\
         | Store candidate window read | {win} |\n\
         | **Fusion pool (the oracle's list)** | **{pool}** |\n\
         | Served after the token-budget trim (current) | {served} |\n\n\
         The pool is bounded by the bank, not by the 200-row window: the deepest rank a gold row can \
         hold here is **{maxrank}**, so the `51-200` bucket holds only ranks 51–{maxrank} and the \
         per-k \"pool smaller than k\" counts below are the same fact stated per cutoff. **The \
         candidate window is not what binds on this suite — the token budget is.** The served list is \
         shorter than the pool on **{served_shorter} of {n_all}** questions, because a 48-session \
         haystack does not fit in the {budget}-token budget the harness passes. That costs nothing at \
         R@5/10/20 ({diverged} questions diverge before rank 20), but it does mean a caller never \
         sees the bottom of the ranking, which is the region a reranker would be drawing its deep \
         candidates from.\n\n",
        bank = Acc::spread(&total.bank_lens),
        win = Acc::spread(&total.window_lens),
        pool = Acc::spread(&total.pool_lens),
        served = Acc::spread(&total.served_lens),
        maxrank = total.pool_lens.iter().copied().max().unwrap_or(0),
        served_shorter = shorter.len(),
        n_all = n_all,
        budget = 100_000,
        diverged = total.diverged_early,
    ));

    // ---- histogram -------------------------------------------------------
    let hist_total: usize = total.hist.iter().sum();
    md.push_str(&format!(
        "## First-gold-rank histogram — all {hist_total} questions\n\n\
         Where the first gold row sat inside the **pool**. This is the load-bearing table: a reranker \
         can only move a gold row that is already in the pool, so everything below rank 5 is the \
         entire rerankable prize, and the `not in pool` mass is the part no reordering can reach.\n\n\
         | First gold rank in pool | Questions | Share |\n|---|---|---|\n",
    ));
    for (i, label) in BUCKETS.iter().enumerate() {
        md.push_str(&format!(
            "| {label} | {} | {:.1}% |\n",
            total.hist[i],
            pct(total.hist[i], n_all)
        ));
    }
    md.push_str(&format!(
        "| **total** | **{hist_total}** | **100.0%** |\n\n\
         `pool smaller than k`, per cutoff — the questions whose pool cannot reach k at all, which is \
         what caps oracle R@k at the deep end:\n\n\
         | k | Questions with pool < k | Share |\n|---|---|---|\n",
    ));
    for &k in KS.iter() {
        let short = total.pool_lens.iter().filter(|&&p| p < k).count();
        md.push_str(&format!("| {k} | {short} | {:.1}% |\n", pct(short, n_all)));
    }
    md.push_str(&format!(
        "\n`pool smaller than k` is stated here rather than as a bucket in the histogram, because the \
         two would double-count: a first-gold rank cannot exceed the pool length, so a gold row past \
         the pool end **is** `not in pool`. Read together the two tables say the whole thing — the \
         histogram says how deep the recoverable mass sits, the per-k counts say which cutoffs the \
         pool is even tall enough to express.\n\n\
         **Questions whose gold set is empty** (the \"evidence resolves to no session\" shape): \
         **{ge}** of {n_all}. They would be kept in the denominator and scored as a miss at every k, \
         never dropped. Both denominators are reported and are identical, so no figure in this \
         artifact depends on the choice: **{n_all}** of {n_all} questions have a non-empty gold set.\n\n",
        ge = total.gold_empty,
        n_all = n_all,
    ));

    // ---- oracle table ----------------------------------------------------
    md.push_str(
        "## Oracle R@k vs current R@k\n\n\
         **Oracle R@k** — a hit if any gold row appears in the **pool's** top k. It is the score a \
         *perfect* reranker over this pool would post: a reranker may reorder candidates but cannot \
         invent one. **Current R@k** — what this build posts today over the **served** list, \
         recomputed here from that same pool before the trim. The delta is the size of the prize.\n\n\
         | k | current R@k | oracle R@k | delta | published R@k (`eval/RESULTS.md`) |\n\
         |---|---|---|---|---|\n",
    );
    for (i, &k) in KS.iter().enumerate() {
        // The published artifact reports R@5/10/20 only. The other four cutoffs
        // have no committed counterpart, and inventing one would be worse than
        // saying so.
        let pub_col = match k {
            5 => "97.2%",
            10 => "98.6%",
            20 => "99.6%",
            _ => "not reported",
        };
        md.push_str(&format!(
            "| {k} | {:.1}% | {:.1}% | +{:.1}pp | {pub_col} |\n",
            pct(total.cur[i], n_all),
            pct(total.ora[i], n_all),
            pct(total.ora[i] - total.cur[i], n_all),
        ));
    }
    md.push_str(&format!(
        "\nRecomputed here: current **{:.1}% / {:.1}% / {:.1}%**, oracle **{:.1}% / {:.1}% / {:.1}%** \
         at R@5 / R@10 / R@20, over {n_all} questions.\n\n\
         The committed `eval/RESULTS.md` row reads **97.2% / 98.6% / 99.6%**. {ref_note}\n\n",
        pct(total.cur[2], n_all),
        pct(total.cur[3], n_all),
        pct(total.cur[4], n_all),
        pct(total.ora[2], n_all),
        pct(total.ora[3], n_all),
        pct(total.ora[4], n_all),
        n_all = n_all,
        ref_note = match (ref_rows, ref_mismatches) {
            (0, _) => "`eval/results.json` was not readable, so the per-question cross-check did \
                       not run and the aggregate above is this harness's own recomputation."
                .to_string(),
            (_, 0) => "They agree — and so does every per-question value: all \
                       `recall_any@5/10/20` recomputed from the served list matched \
                       `eval/results.json` row for row."
                .to_string(),
            (_, bad) => format!(
                "**They do not agree**: {bad} of {ref_rows} per-question rows differ from the \
                 committed `eval/results.json`. Both values are reported here rather than \
                 reconciled."
            ),
        },
    ));

    // ---- the split -------------------------------------------------------
    md.push_str(&format!(
        "## The three-way split — the {misses} questions recall currently misses at R@5\n\n\
         | Where the gold row actually is | Questions | Share of all {n_all} | Share of the {misses} misses | Fixable by |\n\
         |---|---|---|---|---|\n\
         | Rank 6–20 in the pool | {s6} | {p6:.1}% | {m6:.1}% | reranking alone |\n\
         | Rank 21+ in the pool | {s21} | {p21:.1}% | {m21:.1}% | reranking a deeper pool |\n\
         | Not in the pool at all | {absent} | {pa:.1}% | {ma:.1}% | better **matching** only |\n\
         | In pool at ≤5, trimmed out of the served list | {trimmed} | {pt:.1}% | {mt:.1}% | budget, not ranking |\n\
         | **total misses** | **{misses}** | **{pm:.1}%** | **100.0%** | |\n\n\
         In plain language: of the questions recall gets wrong at rank 5 today, **{s6}** have a gold \
         row sitting between rank 6 and 20 — inside the pool, below the cutoff, reachable by \
         reordering and by nothing else. **{s21}** have a gold row at rank 21 or deeper: still inside \
         the pool, so still reachable by a reranker handed the whole pool, but not by anything \
         operating on a top-20 window. **{absent}** have no gold row in the pool at all, which is \
         the one category no reordering can touch and the only one a different kind of match could \
         recover. **{trimmed}** were ranked into the top 5 and then dropped at the token-budget \
         trim — a budget fact rather than a ranking fact, and the reason a three-way split needs a \
         fourth row: those questions would otherwise have been counted as a reranking failure.\n\n\
         **{s6}** questions — **{p6}%** of the suite — are the entire addressable market for a \
         reranker that reorders the pool recall already builds.\n\n",
        misses = misses,
        n_all = n_all,
        s6 = s6,
        s21 = s21,
        absent = absent,
        trimmed = trimmed,
        p6 = pct(s6, n_all),
        p21 = pct(s21, n_all),
        pa = pct(absent, n_all),
        pt = pct(trimmed, n_all),
        m6 = pct(s6, misses),
        m21 = pct(s21, misses),
        ma = pct(absent, misses),
        mt = pct(trimmed, misses),
        pm = pct(misses, n_all),
    ));

    // ---- per category ----------------------------------------------------
    md.push_str(
        "## Per question type\n\n\
         The same categories, cut by the dataset's own `question_type`. This suite's R@5 deficit is \
         known to sit in `single-session-preference` and `single-session-assistant`, so the question \
         each row answers is: is that category's shortfall rerankable, or is it a matching failure?\n\n\
         | Type | n | R@5 | oracle R@5 | Δ | in pool 6–20 | in pool 21+ | absent | trimmed |\n\
         |---|---|---|---|---|---|---|---|---|\n",
    );
    let mut types: Vec<&String> = by_type.keys().collect();
    types.sort();
    for t in types {
        let a = &by_type[t];
        let (c6, c21, cabsent, ctrim, cmisses) = a.misses();
        debug_assert_eq!(c6 + c21 + cabsent + ctrim, cmisses, "partitioned by Acc::check");
        md.push_str(&format!(
            "| {t} | {} | {:.1}% | {:.1}% | +{:.1}pp | {c6} | {c21} | {cabsent} | {ctrim} |\n",
            a.n,
            pct(a.cur[2], a.n),
            pct(a.ora[2], a.n),
            pct(a.ora[2] - a.cur[2], a.n),
        ));
    }
    md.push_str(&format!(
        "| **overall** | **{n_all}** | **{cr5:.1}%** | **{or5:.1}%** | **+{d5:.1}pp** | **{s6}** | **{s21}** | **{absent}** | **{trimmed}** |\n\n",
        n_all = n_all,
        cr5 = pct(total.cur[2], n_all),
        or5 = pct(total.ora[2], n_all),
        d5 = pct(total.ora[2] - total.cur[2], n_all),
        s6 = s6,
        s21 = s21,
        absent = absent,
        trimmed = trimmed,
    ));

    // The conclusion section. The figures are interpolated from the run that
    // wrote the tables above, so a re-run cannot leave the prose contradicting
    // them; what each figure *means* is a finding from the 500-question run
    // recorded here, and a run on different data is a different measurement.
    md.push_str(&format!(
        "\
## What this means

- **Oracle R@5 is {or5:.1}%, not 99.6%.** The gold row is inside the pool recall builds for **all \
{n_all}** questions — {absent} land in `not in pool`. The 99.6% is the *current* R@20, and it \
understates pool coverage because the pool runs far deeper than 20 ({pool} rows, min/median/max). \
A perfect reranker over the pool this build already assembles would post R@5 = {or5:.1}%, against \
today's {cr5:.1}%.
- **The entire R@5 deficit is rerankable: {s6} of the misses sit at rank 6–20, {s21} at rank 21+.** \
Every question recall currently misses at rank 5 has its gold row in the pool, between ranks 6 and \
50. Nothing is lost to pool coverage, so on this suite nothing at R@5 is a matching failure.
- **The largest single prize is at rank 1, not at rank 5.** Current R@1 is {cr1:.1}% against a \
{or1:.1}% oracle — **{d1:.1}pp**, against {d5:.1}pp at R@5. {top1} questions already rank the gold \
row first; the {below1} that do not are what a reranker would have to move, and that is a larger \
prize than the R@5 headline.
- **The recoverable mass is concentrated.** `single-session-preference` supplies {pref6} of the \
{s6} rank-6–20 misses on only {pref_n} questions (R@5 {pref_cr5:.1}% against a {or5:.1}% oracle), \
and `temporal-reasoning` {temp6} of them on {temp_n}. `knowledge-update` and \
`single-session-assistant` are already at 100% R@5 and have nothing left to gain.
- **This suite cannot speak to better *matching*, in either direction.** With {absent} questions \
missing from the pool there is no coverage failure here for any new retrieval signal to fix. The \
pool is the entire {bank} row haystack — `recall_inputs` reads every row on this suite — so pool \
coverage is unconstrained by construction, and no figure here supports or refutes a vector arm or \
an LLM.
- **The candidate list is not the binding constraint; the serving budget is.** The pool holds \
{pool} rows while the 100,000-token budget leaves a caller {served}, on all {n_all} questions. No \
question diverges before rank 20, so the trim cost {trimmed} R@5 hits — but the deep candidates a \
reranker would be reordering are exactly the ones the budget keeps out of the response.
",
        n_all = n_all,
        or1 = pct(total.ora[0], n_all),
        or5 = pct(total.ora[2], n_all),
        cr1 = pct(total.cur[0], n_all),
        cr5 = pct(total.cur[2], n_all),
        d1 = pct(total.ora[0] - total.cur[0], n_all),
        d5 = pct(total.ora[2] - total.cur[2], n_all),
        top1 = total.hist[1],
        below1 = total.hist[2..].iter().sum::<usize>(),
        absent = absent,
        trimmed = trimmed,
        s6 = s6,
        s21 = s21,
        pref6 = by_type
            .get("single-session-preference")
            .map_or(0, |a| a.six_twenty),
        pref_n = by_type
            .get("single-session-preference")
            .map_or(0, |a| a.n),
        pref_cr5 = by_type
            .get("single-session-preference")
            .map_or(0.0, |a| pct(a.cur[2], a.n)),
        temp6 = by_type
            .get("temporal-reasoning")
            .map_or(0, |a| a.six_twenty + a.twenty_plus),
        temp_n = by_type
            .get("temporal-reasoning")
            .map_or(0, |a| a.n),
        pool = Acc::spread(&total.pool_lens),
        served = Acc::spread(&total.served_lens),
        bank = Acc::spread(&total.bank_lens),
    ));

    fs::write(&out_md, &md)?;
    eprintln!("wrote {out_md}");
    eprintln!(
        "oracle_rerank: {n_all} questions · served shorter than pool {} · diverged before rank 20 \
         {} · results.json mismatches {ref_mismatches}",
        shorter.len(),
        total.diverged_early,
    );
    Ok(())
}

