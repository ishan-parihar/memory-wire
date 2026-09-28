//! Shared core for the two LoCoMo **dev-set** harnesses — `examples/locomo.rs`
//! (the replication check on `overlap: 0.25`) and `examples/select_fusion.rs`
//! (the coordinate-descent weight sweep).
//!
//! **Why this file exists.** Both harnesses answer different questions and produce
//! different artifacts, but they must answer them about the *same* corpus, through
//! the *same* ingestion, with the *same* metric definitions — otherwise a number in
//! one of them cannot be compared with a number in the other, and a weight grid is
//! exactly the artifact whose value is cross-row comparison. A second copy of the
//! ingestion and the metrics is the failure mode `AGENTS.md` §2 exists to prevent
//! ("when a doc and an artifact disagree, the artifact wins" presupposes there is
//! one artifact per measurement, not two code paths pretending to be one).
//!
//! **What is deliberately NOT here.** Anything one harness needs alone. `locomo.rs`
//! keeps its grid, its replication verdict and its data-shape audit; `select_fusion.rs`
//! keeps its coordinate descent, its objective and its round bookkeeping. This file
//! is the part that would have to be identical in both, and no more.
//!
//! ## The three-set discipline
//!
//! LoCoMo is the **dev** set (`AGENTS.md` §1, `docs/EVALUATION_HYGIENE.md` §3.1).
//! Selection happens here and only here. Nothing in this file may be pointed at
//! LongMemEval: the ingestion takes a `--docs`/`--data` pair, and both harnesses
//! default those to `eval/data/locomo/`.
//!
//! ## Reuse
//!
//! Included by path, the way `mod bench_common;` picks up
//! `examples/bench_common/mod.rs`:
//!
//! ```ignore
//! #[path = "../eval/locomo_dev.rs"]
//! mod dev;
//! ```
//!
//! Cargo builds each `examples/*.rs` as its own crate root, so a helper only one
//! harness uses is dead code from the other's compiler's point of view. That is the
//! same trade `bench_common` already makes, and it is cheaper than the drift two
//! copies would carry.
#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;

use anyhow::Context as _;
use serde::Deserialize;

use memory_wire::api::MemoryService;
use memory_wire::memory::{Bank, Memory};
use memory_wire::store::{SqliteStore, Store};

/// Upstream commit `eval/download.sh` pins its sha256s to, quoted in every artifact
/// so a reader can find the exact bytes these numbers came from.
pub const UPSTREAM_REF: &str = "decbb07f4f9899deac28a76293564cf263872652";

/// A LoCoMo document. `content` is a JSON **string** holding a list of dialogue
/// turns, so it is parsed a second time in [`Document::text`].
#[derive(Deserialize)]
pub struct Document {
    /// Document id, used as the memory id and as the `gold_ids` reference.
    pub id: String,
    /// The JSON-encoded turn list.
    pub content: String,
    /// The conversation this document belongs to, and the bank-isolation unit.
    pub user_id: String,
}

/// One turn of a LoCoMo document. Every turn has `speaker` and `text`; the
/// optional field is the image-caption side-channel of the conversations that
/// included photos.
#[derive(Deserialize)]
pub struct Turn {
    /// `CARL` or `MARY` in the `locomo10` conversations; kept as a string because
    /// the harness asserts nothing about its value.
    #[allow(dead_code)]
    pub speaker: String,
    /// The utterance, which may be empty for a photo turn.
    #[serde(default)]
    pub text: String,
    /// Caption for an attached photo, if any.
    #[serde(default)]
    pub blip_caption: Option<String>,
}

/// A LoCoMo query. `meta` and `gold_ids` are **already-decoded** JSON values in this
/// distribution — only `content` is double-encoded — and `gold_answers` is an array
/// whose elements are not uniformly strings (6 of 1,540 carry a bare JSON number, a
/// year or a count), so it is held as `serde_json::Value` and only ever read for
/// shape.
#[derive(Deserialize)]
pub struct Query {
    /// Query id, carried into the per-question JSON dump so any cell is re-derivable.
    pub id: String,
    /// The question text, used verbatim as the recall query.
    pub query: String,
    /// Gold *answer* prose. Never scored: this is a retrieval harness. Held as
    /// `Value` because a numeric answer is emitted unquoted in this distribution.
    pub gold_answers: Vec<serde_json::Value>,
    /// The gold document ids. One for most queries, 2–15 for some, empty for two —
    /// and an empty list is dropped by [`plan`], because a question with no gold
    /// document has no document-level answer and every `recall_any@K` would be 0 by
    /// construction.
    pub gold_ids: Vec<String>,
    /// The conversation this query is answered against, and nothing else.
    pub user_id: String,
    /// Per-category labelling, the only field a breakdown is taken on.
    pub meta: Meta,
}

/// The `meta` object on a LoCoMo query: `category`, `sample_id`, `speaker_a`,
/// `speaker_b`, `query_timestamp`. Only `category` is read.
#[derive(Deserialize)]
pub struct Meta {
    /// `multi-hop` | `open-domain` | `single-hop` | `temporal`.
    pub category: String,
}

impl Document {
    /// The document as one memory body: one line per turn, caption text folded in
    /// where there is any. The same shape `longmemeval` builds from a session, so the
    /// two suites put the same kind of string in front of the same index.
    pub fn text(&self) -> anyhow::Result<String> {
        let turns: Vec<Turn> = serde_json::from_str(&self.content).map_err(|e| {
            anyhow::anyhow!("locomo: document {} has un-decodable content: {e}", self.id)
        })?;
        Ok(turns
            .iter()
            .map(|t| {
                let mut line = format!("{}: {}", t.speaker, t.text);
                // A photo turn can carry no text, in which case its caption is the
                // only content in the bank a question about it could match.
                match t
                    .blip_caption
                    .as_deref()
                    .map(str::trim)
                    .filter(|c| !c.is_empty())
                {
                    Some(caption) => line.push_str(&format!(" [image: {caption}]")),
                    None if t.text.trim().is_empty() => line.push_str(" [image: no caption]"),
                    None => {}
                }
                line
            })
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

/// The dev corpus, ingested: one bank per `user_id`, one memory per document.
pub struct Corpus {
    /// Every document as parsed, for the counts an artifact reports.
    pub documents: Vec<Document>,
    /// Every query as parsed, **including** the ones [`plan`] drops.
    pub queries: Vec<Query>,
    /// One service per conversation. A query is answered against its own `user_id`'s
    /// bank and no other, which is the isolation model `examples/longmemeval.rs` uses.
    pub banks: HashMap<String, MemoryService<SqliteStore>>,
    /// Documents per bank, for the "N–M per bank" line.
    pub bank_sizes: HashMap<String, usize>,
    /// The recall budget, derived from the corpus rather than pinned — see
    /// [`Corpus::load`].
    pub budget_tokens: usize,
    /// Dialogue turns decoded across every document.
    pub turns: usize,
    /// Queries whose `gold_ids` is empty; carried so a harness can report the drop
    /// instead of quietly shrinking its own denominator.
    pub skipped_no_gold: usize,
}

impl Corpus {
    /// Read both JSON files, ingest one in-memory store per conversation, and derive
    /// the recall budget.
    ///
    /// **One index per conversation, every configuration scored on it** — the same
    /// structure as `examples/sweep_fusion.rs`, and for the same reason: a
    /// byte-identical corpus under every row, so two rows cannot disagree because
    /// one's index happened to build differently.
    ///
    /// The budget is a hard token cap and a bank is a subset of the corpus, so half
    /// the corpus in characters is twice the tightest possible bank. Derived from the
    /// corpus rather than pinned, so a differently-sized dataset cannot make the
    /// budget trim a document the candidate pool would otherwise have returned.
    pub fn load(docs_path: &str, queries_path: &str) -> anyhow::Result<Self> {
        let documents: Vec<Document> =
            serde_json::from_str(&fs::read_to_string(docs_path).with_context(|| {
                format!("locomo: cannot read the document corpus at {docs_path}")
            })?)
            .with_context(|| format!("locomo: {docs_path} is not a JSON array of documents"))?;
        let queries: Vec<Query> = serde_json::from_str(&fs::read_to_string(queries_path)?)
            .with_context(|| format!("locomo: {queries_path} is not a JSON array of queries"))?;
        anyhow::ensure!(
            !documents.is_empty() && !queries.is_empty(),
            "locomo: {docs_path} / {queries_path} is empty — run eval/download.sh"
        );

        // Ten conversations, 272 documents. Each bank holds 19-32 documents, under
        // both recall's 200-row candidate pool window and BM25's `LIMIT 50`, so the
        // *store* pool is the whole bank — but the *fused* list is the union of the
        // streams, and every stream drops a document that matches no query token
        // (`fts_match_query` ORs the query tokens; `rank_candidates` retains
        // `score > 0.0`). A gold document sharing no token with its question is
        // therefore unreachable at any K, which is why the harness reports `R@pool`
        // rather than claiming a ceiling it does not have.
        let mut stores: HashMap<String, SqliteStore> = HashMap::new();
        let mut bank_sizes: HashMap<String, usize> = HashMap::new();
        let mut total_chars = 0usize;
        let mut turns = 0usize;
        for doc in &documents {
            let text = doc.text()?;
            total_chars += text.len();
            turns += serde_json::from_str::<Vec<Turn>>(&doc.content)
                .map(|t| t.len())
                .unwrap_or(0);
            let store = stores.entry(doc.user_id.clone()).or_insert_with(|| {
                let s = SqliteStore::open_in_memory()
                    .expect("an in-memory store cannot fail to open");
                s.put_bank(&Bank {
                    id: doc.user_id.clone(),
                    name: doc.user_id.clone(),
                })
                .expect("put_bank on a fresh in-memory store only inserts the bank row");
                s
            });
            store.put(&Memory {
                id: doc.id.clone(),
                bank_id: doc.user_id.clone(),
                content: text,
                context: None,
                created_at: None,
            })?;
            *bank_sizes.entry(doc.user_id.clone()).or_default() += 1;
        }
        let banks: HashMap<String, MemoryService<SqliteStore>> = stores
            .into_iter()
            .map(|(id, s)| (id, MemoryService::new(s)))
            .collect();

        let missing_bank: Vec<&str> = queries
            .iter()
            .map(|q| q.user_id.as_str())
            .filter(|u| !banks.contains_key(*u))
            .collect();
        anyhow::ensure!(
            missing_bank.is_empty(),
            "locomo: {} queries name a user_id with no documents (first: {}) — bank isolation would \
             score them as misses rather than as a broken corpus",
            missing_bank.len(),
            missing_bank.first().copied().unwrap_or("?")
        );

        let skipped_no_gold = queries.iter().filter(|q| q.gold_ids.is_empty()).count();
        Ok(Self {
            documents,
            queries,
            banks,
            bank_sizes,
            budget_tokens: total_chars / 2,
            turns,
            skipped_no_gold,
        })
    }

    /// Documents in the smallest and largest bank, for the provenance line.
    pub fn bank_range(&self) -> (usize, usize) {
        (
            self.bank_sizes.values().copied().min().unwrap_or(0),
            self.bank_sizes.values().copied().max().unwrap_or(0),
        )
    }

    /// The `meta.category` values present, sorted, for a per-category table's header.
    pub fn categories(&self) -> Vec<String> {
        let mut v: Vec<String> = self.queries.iter().map(|q| q.meta.category.clone()).collect();
        v.sort();
        v.dedup();
        v
    }
}

/// One query that will actually be scored, with its gold set resolved once rather
/// than per configuration.
pub struct PlannedQuery<'q> {
    /// Borrowed from the [`Corpus`], which outlives every plan built over it.
    pub query: &'q Query,
    /// `gold_ids` as a set, for the `contains` the metrics do per retrieved id.
    pub gold: HashSet<String>,
}

/// The queries to evaluate, in the caller's order, with the unanswerable ones dropped.
///
/// `order` is a list of indices into `queries` rather than a slice, so a harness can
/// apply `--n` and `--seed` to it first (`bench_common::shuffle` then `truncate`)
/// without this function having to know what a seed is. A query with an empty
/// `gold_ids` is dropped rather than scored: it has a gold *answer* but no gold
/// *document*, so no document could be retrieved and every `recall_any@K` would be 0
/// by construction. Keeping it would understate every row by a flat constant
/// carrying no measurement at all, so it is dropped **and counted**, never silently
/// kept.
pub fn plan<'q>(queries: &'q [Query], order: impl IntoIterator<Item = usize>) -> Vec<PlannedQuery<'q>> {
    order
        .into_iter()
        .filter_map(|i| {
            let query = &queries[i];
            if query.gold_ids.is_empty() {
                None
            } else {
                Some(PlannedQuery {
                    query,
                    gold: query.gold_ids.iter().cloned().collect(),
                })
            }
        })
        .collect()
}

/// A hit means **any** id in `gold` inside the top `k` — the same `recall_any`
/// definition `longmemeval` and `sweep_fusion` use, so the R@5/R@10 columns are on
/// one scale with `eval/RESULTS.md`.
pub fn recall_any(retrieved: &[String], gold: &[String], k: usize) -> f64 {
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

/// NDCG@k with the binary relevance `recall_any` implies, ideal DCG over
/// `min(|gold|, k)` relevant rows. Zero when the gold set is empty, which
/// [`plan`] has already made unreachable.
pub fn ndcg(retrieved: &[String], gold: &HashSet<String>, k: usize) -> f64 {
    let rels: Vec<bool> = retrieved.iter().take(k).map(|id| gold.contains(id)).collect();
    let ideal = dcg(&vec![true; gold.len().min(k)], k);
    if ideal == 0.0 {
        return 0.0;
    }
    dcg(&rels, k) / ideal
}

/// Reciprocal rank of the **first** gold id, or 0.0 if none was returned.
pub fn mrr(retrieved: &[String], gold: &HashSet<String>) -> f64 {
    for (i, id) in retrieved.iter().enumerate() {
        if gold.contains(id) {
            return 1.0 / (i + 1) as f64;
        }
    }
    0.0
}

/// Per-question values, kept so a diff between two configurations is a real
/// per-question diff and any aggregate in an artifact can be re-derived from the JSON.
#[derive(Clone)]
pub struct Row {
    /// LoCoMo query id.
    pub query_id: String,
    /// `meta.category`, the breakdown key.
    pub category: String,
    /// `recall_any@1`.
    pub r1: f64,
    /// `recall_any@5`.
    pub r5: f64,
    /// `recall_any@10`.
    pub r10: f64,
    /// `recall_any@20`.
    pub r20: f64,
    /// NDCG@10.
    pub ndcg10: f64,
    /// Reciprocal rank of the first gold id.
    pub mrr: f64,
    /// Did the gold document survive the candidate pool at all? Measured over the
    /// *whole* returned list, not a top-k, so it answers "is retrieval or coverage
    /// the binding constraint" — the question R@20 cannot answer on a corpus whose
    /// banks are smaller than 20 documents.
    pub pool: f64,
}

impl Row {
    /// Score one recall's returned id list against the query's gold.
    pub fn new(query: &Query, retrieved: &[String], gold: &HashSet<String>) -> Self {
        Self {
            query_id: query.id.clone(),
            category: query.meta.category.clone(),
            r1: recall_any(retrieved, &query.gold_ids, 1),
            r5: recall_any(retrieved, &query.gold_ids, 5),
            r10: recall_any(retrieved, &query.gold_ids, 10),
            r20: recall_any(retrieved, &query.gold_ids, 20),
            ndcg10: ndcg(retrieved, gold, 10),
            mrr: mrr(retrieved, gold),
            pool: recall_any(retrieved, &query.gold_ids, usize::MAX),
        }
    }
}

/// A running sum of [`Row`]s, with the percentage accessors every table needs.
///
/// Percentages are multiplied by 100 here so that a table cell and its `Δ` are the
/// same units the whole project quotes; the denominator is `count` and is printed
/// next to every percentage, per `AGENTS.md` §3.
#[derive(Default, Clone)]
pub struct Agg {
    /// The n behind every percentage in this aggregate.
    pub count: usize,
    /// Summed `recall_any@1`.
    pub r1: f64,
    /// Summed `recall_any@5`.
    pub r5: f64,
    /// Summed `recall_any@10`.
    pub r10: f64,
    /// Summed `recall_any@20`.
    pub r20: f64,
    /// Summed NDCG@10.
    pub ndcg10: f64,
    /// Summed MRR.
    pub mrr: f64,
    /// Summed `R@pool`.
    pub pool: f64,
}

impl Agg {
    /// Fold one row in.
    pub fn add(&mut self, r: &Row) {
        self.count += 1;
        self.r1 += r.r1;
        self.r5 += r.r5;
        self.r10 += r.r10;
        self.r20 += r.r20;
        self.ndcg10 += r.ndcg10;
        self.mrr += r.mrr;
        self.pool += r.pool;
    }
    /// The percentage form of a sum.
    pub fn pct(&self, v: f64) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            v / self.count as f64 * 100.0
        }
    }
    /// `recall_any@1` as a percentage.
    pub fn r1(&self) -> f64 {
        self.pct(self.r1)
    }
    /// `recall_any@5` as a percentage.
    pub fn r5(&self) -> f64 {
        self.pct(self.r5)
    }
    /// `recall_any@10` as a percentage.
    pub fn r10(&self) -> f64 {
        self.pct(self.r10)
    }
    /// `recall_any@20` as a percentage.
    pub fn r20(&self) -> f64 {
        self.pct(self.r20)
    }
    /// NDCG@10 as a percentage.
    pub fn ndcg10(&self) -> f64 {
        self.pct(self.ndcg10)
    }
    /// MRR as a percentage.
    pub fn mrr(&self) -> f64 {
        self.pct(self.mrr)
    }
    /// `R@pool` as a percentage.
    pub fn pool(&self) -> f64 {
        self.pct(self.pool)
    }
    /// Every metric, in the order the tables print them, as percentages.
    pub fn all(&self) -> [f64; 6] {
        [self.r1(), self.r5(), self.r10(), self.r20(), self.ndcg10(), self.mrr()]
    }
    /// The metric set under the given column names, for a table header and a `Δ`.
    pub fn labelled(&self) -> [(&'static str, f64); 6] {
        [
            ("R@1", self.r1()),
            ("R@5", self.r5()),
            ("R@10", self.r10()),
            ("R@20", self.r20()),
            ("NDCG@10", self.ndcg10()),
            ("MRR", self.mrr()),
        ]
    }
}

/// Per-category aggregates, in sorted key order so a table's rows are stable across
/// runs and across configurations.
pub type PerCategory = BTreeMap<String, Agg>;

/// `/proc/loadavg`, trimmed. `"unavailable"` where there is no `/proc`, so a
/// non-Linux box still produces an artifact that says it does not know rather than
/// one that omits the line.
pub fn loadavg() -> String {
    fs::read_to_string("/proc/loadavg")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unavailable".to_string())
}

/// The commit this artifact describes, read out of the working tree rather than
/// passed in. `docs/EVALUATION_HYGIENE.md` §1 is about numbers that look like
/// generalisation estimates; a benchmark artifact that cannot name the build it
/// measured is the same failure in a different place. Prints a placeholder instead
/// of failing: a source tree with no `.git` still produces a usable artifact, as long
/// as it says so.
pub fn git_head() -> String {
    let found = std::env::current_dir().ok().and_then(|start| {
        let mut up = Some(start.as_path());
        while let Some(cur) = up {
            let g = cur.join(".git");
            if g.is_dir() || g.is_file() {
                return Some(g);
            }
            up = cur.parent();
        }
        None
    });
    let read = found.and_then(|git| {
        let head = fs::read_to_string(git.join("HEAD")).ok()?.trim().to_string();
        // A detached HEAD is the commit; a branch is a ref file to read.
        match head.strip_prefix("ref: ") {
            Some(r) => fs::read_to_string(git.join(r.trim())).ok().map(|s| s.trim().to_string()),
            None => Some(head),
        }
    });
    match read {
        Some(sha) if sha.len() == 40 => sha,
        Some(other) => format!("(unresolved: {other})"),
        None => "(unavailable: no .git found above the working directory)".to_string(),
    }
}

/// How many of `sample`'s entries differ from `base`, as `(better, worse, same)`.
/// Counted rather than summarised, so a configuration that wins on the mean by
/// losing half its questions and winning the other half big is visible instead of
/// hidden behind the mean.
pub fn movement(base: &[f64], sample: &[f64]) -> (usize, usize, usize) {
    let (mut up, mut down) = (0, 0);
    for (s, b) in sample.iter().zip(base) {
        match s.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal) {
            std::cmp::Ordering::Greater => up += 1,
            std::cmp::Ordering::Less => down += 1,
            std::cmp::Ordering::Equal => {}
        }
    }
    (up, down, sample.len().saturating_sub(up + down))
}
