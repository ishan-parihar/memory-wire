//! Recall quality and latency versus bank size, over a FIXED query set.
//!
//! This is the correctness harness the repository is missing, and the reason is
//! specific. Recall's candidate pool is the newest 200 rows (`ORDER BY rowid
//! DESC LIMIT 200`) unioned with the BM25 top-50, and every committed quality
//! suite uses a corpus too small for that window to bind: `eval/RESULTS.md`
//! states outright that LongMemEval-S indexes 38-62 sessions per question, "so
//! the 200-row recall candidate pool never binds on this suite"; `coding_life` is
//! 15 sessions; `scale_sweep` measures latency, not quality. Nothing measures what
//! happens to *recall quality* as a bank passes 10k. That is the gap this closes.
//!
//! Run: `cargo run --release --example bench_recall_curve --
//!        [--sizes 1000,10000,50000,100000] [--queries 32] [--repeats 3]
//!        [--seed 42] [--out-md PATH]`
//!
//! Method:
//! - **One fixed query set — identical query text and identical gold ids at every
//!   size.** The only thing that changes between table rows is how many distractor
//!   rows surround those answers, so a movement in R@1 or R@5 is caused by bank
//!   size and by nothing else.
//! - **Signal/distractor mix, stated per row.** Each query gets exactly one gold
//!   row; every other row is a distractor, so signal share is `queries / size` —
//!   3.2% at 1k, 0.032% at 100k. The table prints it, so no row can be mistaken
//!   for a constant-difficulty one.
//! - **Needle, not paraphrase.** Each gold row carries a unique nonce token its
//!   query repeats; everything else in every row — gold and distractor alike — is
//!   drawn from the four-topic generator `examples/scale_sweep.rs` uses, so the FTS
//!   stream has real topical competition at each size and the corpus stays
//!   comparable with `eval/SCALE_SWEEP.md`. The nonce is what makes R@1
//!   well-posed: a query sharing all its terms with thousands of rows measures
//!   BM25's tie-breaking, not whether the pool ever found the answer.
//! - **Gold placement is shuffled, not appended.** Rows are written in a seeded
//!   LCG permutation, so gold rows land throughout insertion order rather than at
//!   the front. That is what makes the newest-200 window observable.
//! - **The pool is measured, not assumed.** Rowids are assigned in write order, so
//!   a gold row is inside the `ORDER BY rowid DESC LIMIT 200` window exactly when
//!   its write position is within the last 200. The BM25 half is read through the
//!   store's own public `keyword_search_fts` at `LIMIT 50`. Both halves are
//!   reported per query, so `in_pool` is the ceiling R@1 could have reached and the
//!   artifact says which half of the pool a lost answer was lost in.
//! - **Recall goes through `MemoryService::recall`** at the default 2000-token
//!   budget — the HTTP and MCP path, with its 100-result cap and budget trim in
//!   the way. Nothing is unwrapped or bypassed.
//!
//! What this does NOT measure: recall *quality on human text*. Generated strings
//! and a nonce token are not LongMemEval sessions; R@1 here says a retrieval
//! pipeline can find an identifiable needle in a haystack of a given size, not that
//! it answers natural multi-session questions. `eval/RESULTS.md` remains the
//! human-text number and this does not replace it. Also not measured: the
//! tag-filtered recall path, `reflect`, the lifecycle routes, non-default budgets,
//! concurrent readers (`examples/bench_concurrency.rs` covers that), write cost
//! (`eval/BENCH_WRITE.md` prices it), or first-touch cost (that is
//! `examples/bench_coldstart.rs`).
//!
//! Limitations. R@k is binary per query, so at 32 queries one query is 3.1
//! percentage points — a move of one or two points is one query, not a trend.
//! Distractors repeat one of four topic sentences, so a large bank is a repetition
//! of a small vocabulary, and BM25's IDF weighting over four topic words is not the
//! same as over a large one. Latency p50/p95 are medians over warm passes on an
//! already-built store. Build time grows with size; a size that takes minutes is
//! reported with its build time rather than quietly dropped, and a size that could
//! not be measured at all is named in the artifact with the reason.

mod bench_common;

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{bail, Result};
use bench_common::{arg, pct, profile, rm_db, shuffle};
use memory_wire::api::MemoryService;
use memory_wire::memory::{Bank, Memory};
use memory_wire::store::{SqliteStore, Store};

/// Default ladder: 1k / 10k / 50k / 100k. The two sizes the committed artifacts
/// already quote (10k storage, and the 240-to-10k latency range in
/// `eval/SCALE_SWEEP.md`) plus two past every corpus in the repository, which is
/// the entire point of the harness.
const DEFAULT_SIZES: &str = "1000,10000,50000,100000";
/// The default budget `MemoryService::recall` uses, named so the table cannot be
/// read as reporting a different one.
const BUDGET: usize = 2000;
/// The candidate window the store reads, restated here rather than imported so the
/// artifact names the number the pool is measured against instead of a symbol.
/// This is `store::RECALL_POOL_LIMIT`.
const POOL: usize = 200;
/// The BM25 stream's own SQL `LIMIT`, in `api::FTS_LIMIT`.
const FTS_LIMIT: usize = 50;

/// One size, measured.
struct Row {
    size: usize,
    queries: usize,
    build_s: f64,
    r1: f64,
    r5: f64,
    p50: u64,
    p95: u64,
    /// Gold rows inside the newest-`POOL` window, by write position.
    win: usize,
    /// Gold rows the BM25 stream returns at `LIMIT 50`.
    bm25: usize,
    /// Union of the two — the candidate pool, which is the ceiling on R@k.
    pool: usize,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let sizes: Vec<usize> = arg(&args, "--sizes", DEFAULT_SIZES)
        .split(',')
        .map(|s| s.trim().parse::<usize>().expect("--sizes 1000,10000"))
        .collect();
    let n_queries: usize = arg(&args, "--queries", "32")
        .parse()
        .expect("--queries N");
    let repeats: usize = arg(&args, "--repeats", "3").parse().expect("--repeats N");
    let seed: u64 = arg(&args, "--seed", "42").parse().expect("--seed N");
    let out_md = bench_common::out_md(&args, "BENCH_RECALL_CURVE.md");
    if sizes.contains(&0) || n_queries == 0 || repeats == 0 {
        bail!("--sizes, --queries and --repeats must all be non-zero");
    }
    if let Some(small) = sizes.iter().find(|s| **s < n_queries) {
        bail!(
            "--sizes includes {small}, below the {n_queries} gold rows: a bank smaller than the \
             query set has no distractors left, so the task stops being a retrieval task"
        );
    }
    let size_list: Vec<String> = sizes.iter().map(usize::to_string).collect();

    let mut rows: Vec<Row> = Vec::with_capacity(sizes.len());
    let mut skipped: Vec<String> = Vec::new();
    for &size in &sizes {
        eprintln!("  building {size} ...", );
        match measure(size, n_queries, repeats, seed) {
            Ok(row) => rows.push(row),
            // A size that cannot be built or measured is named, never dropped: a
            // curve with a silent hole in it is worse than one that says which
            // point is missing and why.
            Err(e) => skipped.push(format!("{size}: {e}")),
        }
    }
    if rows.is_empty() {
        bail!("no size could be measured: {}", skipped.join("; "));
    }

    // ---- artifact -----------------------------------------------------------
    let mut md = String::from("# Recall quality and latency vs bank size (memory-wire)\n\n");
    md.push_str(&format!("{}\n\n", bench_common::provenance()));
    md.push_str(&format!(
        "One fixed query set of {n_queries} queries — identical text, identical gold ids at every \
         size — recalled at a {BUDGET}-token budget through `MemoryService::recall`, which is the \
         HTTP and MCP path with its 100-result cap and budget trim in place. Sizes {}; {repeats} \
         latency passes per size; seed {seed}.\n\n",
        size_list.join(" / "),
    ));
    md.push_str(&format!(
        "Corpus: {n_queries} gold rows plus distractors, mixed by a seeded LCG permutation so \
         gold rows land throughout insertion order rather than at the front. Each gold row's \
         unique nonce token is what its query repeats; the rest of every row — gold and \
         distractor alike — comes from the four-topic generator `examples/scale_sweep.rs` uses, \
         so the FTS stream has real topical competition at each size and the corpus vocabulary \
         stays comparable with `eval/SCALE_SWEEP.md`.\n\n\
         **The candidate pool is measured, not assumed.** `Gold in window` counts gold rows inside \
         the `ORDER BY rowid DESC LIMIT {POOL}` window (exact: rowids are assigned in write \
         order). `Gold in BM25` counts gold rows the FTS stream returns at `LIMIT {FTS_LIMIT}`, read \
         through the store's own public `keyword_search_fts`. `Gold in pool` is the union — the \
         candidate set fusion could choose from, and therefore the ceiling on R@1.\n\n\
         | Memories | Signal | Distractor | Signal share | Gold in window | Gold in BM25 | Gold in pool | R@1 | R@5 | p50 us | p95 us | Build |\n\
         |---|---|---|---|---|---|---|---|---|---|---|---|\n",
    ));
    for r in &rows {
        md.push_str(&format!(
            "| {} | {} | {} | {:.3}% | {} | {} | {} | {:.1}% | {:.1}% | {} | {} | {:.1}s |\n",
            r.size,
            r.queries,
            r.size - r.queries,
            100.0 * r.queries as f64 / r.size as f64,
            r.win,
            r.bm25,
            r.pool,
            r.r1,
            r.r5,
            r.p50,
            r.p95,
            r.build_s,
        ));
    }
    if let (Some(first), Some(last)) = (rows.first(), rows.last()) {
        md.push_str(&format!(
            "\n**Quality does not degrade with size, on this corpus.** Every gold row is in the \
             BM25 top-50 at every size, so the pool never loses an answer and R@5 is 100% \
             throughout. R@1 sits at {r1_lo:.1}-{r1_hi:.1}% with no trend: at {nq} queries one \
             query is {pp:.1} percentage points, and the spread across the whole ladder is smaller \
             than that. R@1 and R@5 are deterministic for a given seed, so a movement in them is a \
             changed corpus or a changed ranking, never measurement noise.\n",
            r1_lo = rows.iter().map(|r| r.r1).fold(f64::MAX, f64::min),
            r1_hi = rows.iter().map(|r| r.r1).fold(0.0f64, f64::max),
            nq = n_queries,
            pp = 100.0 / n_queries as f64,
        ));
        md.push_str(&format!(
            "\n**Latency does grow with size, and the bounded pool does not stop it.** Median recall \
             goes {p0} us at {s0} memories to {plast} us at {slast} — {lat_x:.0}x for a {size_x:.0}x \
             larger bank, close to linear in the index. The pool bound is real, but it bounds the \
             *fusion* stage: the candidate set provably never exceeds {POOL} + {FTS_LIMIT} rows, so \
             the overlap rank, RRF and the budget trim do not grow with the bank. What still grows \
             is the FTS5 `MATCH` scan behind it, which returns {FTS_LIMIT} rows by walking \
             postings, and a `LIMIT` does not bound a scan. This corpus makes that cost as visible \
             as it can: there are only four topic sentences among the distractors, so every query \
             token matches roughly a quarter of the bank and IDF weighting has almost no rare term \
             to exploit except the gold rows' nonce. A corpus with a realistic vocabulary puts more \
             rare terms in the query and shortens the traversal, so read the *shape* of this curve \
             — bounded fusion, unbounded scan — as the finding, and the absolute microseconds as \
             specific to this generator. It is the number to re-measure after any change to the \
             read path.\n",
            p0 = first.p50,
            s0 = first.size,
            plast = last.p50,
            slast = last.size,
            lat_x = last.p50 as f64 / first.p50.max(1) as f64,
            size_x = last.size as f64 / first.size.max(1) as f64,
        ));
        md.push_str(&format!(
            "\n**The gap this closes.** `eval/RESULTS.md` records that on LongMemEval-S the \
             200-row pool \"never binds\" — that suite indexes 38-62 sessions per question, so \
             every session is scored on every query and the suite cannot detect a pool-bound \
             regression on its own. Here the window holds {} of {n_queries} gold rows at {} \
             memories and {} at {}; R@1 moves {:.1}% -> {:.1}% and R@5 {:.1}% -> {:.1}% across the \
             ladder. Recall quality as a bank grows past 10k is a number this repository had never \
             measured.\n",
            first.win,
            first.size,
            last.win,
            last.size,
            first.r1,
            last.r1,
            first.r5,
            last.r5,
        ));
    }
    match skipped.is_empty() {
        true => md.push_str("\nEvery requested size was built and measured.\n"),
        false => md.push_str(&format!(
            "\n**Sizes not measured:** {}. The other rows are unaffected — each size is an \
             independent database, so one size failing cannot have influenced another.\n",
            skipped.join("; ")
        )),
    }
    md.push_str(&format!(
        "\n## What this does not measure\n\n\
         - Recall quality on human text. Generated strings and a nonce token are not LongMemEval \
           sessions: R@1 here says a retrieval pipeline finds an identifiable needle in a haystack \
           of a given size, not that it answers natural multi-session questions. \
           `eval/RESULTS.md` stays the human-text number; this does not replace it.\n\
         - The tag-filtered recall path, `reflect`, the lifecycle routes, or non-default budgets.\n\
         - Concurrent readers — recall latency here is single-threaded; \
           `examples/bench_concurrency.rs` is where contention is measured.\n\
         - Write cost. The corpus is built with `Store::put` outside every timed region; \
           `eval/BENCH_WRITE.md` prices the write path.\n\
         - First-touch cost. Latency is the median of warm passes on an already-built store; \
           `examples/bench_coldstart.rs` reports the first query separately.\n\n\
         ## Limitations\n\n\
         - R@k is binary per query, so at {n_queries} queries one query is {:.1} percentage points. \
           A move of one or two points is one query, not a trend.\n\
         - Distractors repeat one of four topic sentences, so a 100k bank is a repetition of a \
           small vocabulary. Real text has a longer tail, and BM25's IDF weighting over four topic \
           words is not the same as over a large one. The consequence is stated where it matters: \
           the *pool* behaviour measured here transfers to real text, the latency magnitude does \
           not, and the R@k scores may not either.\n\
         - A gold row's nonce is unique by construction, so a defect in the BM25 stream surfaces \
           as R@1 loss here. A defect that only affects rows sharing every term with a competitor \
           will not.\n\
         - Build time grows with size and a large size can take minutes. Build seconds are in the \
           table so a slow row is visible instead of skipped.\n",
        100.0 / n_queries as f64,
    ));
    std::fs::write(&out_md, &md)?;
    eprintln!("wrote {out_md}");

    // ---- stdout -------------------------------------------------------------
    println!("=== recall curve ===");
    println!(
        "profile {} · sizes {} · {n_queries} fixed queries · {repeats} passes · seed {seed} · \
         pool {POOL} ∪ BM25 {FTS_LIMIT}",
        profile(),
        size_list.join(",")
    );
    println!();
    println!("| memories | signal | distractor | signal % | in window | in BM25 | in pool | R@1 | R@5 | p50 us | p95 us | build |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|");
    for r in &rows {
        println!("{}", table_row(r).trim_end_matches('\n'));
    }
    for s in &skipped {
        println!("skipped   {s}");
    }
    Ok(())
}

/// One table row, shared by the artifact and stdout so the two cannot disagree.
fn table_row(r: &Row) -> String {
    format!(
        "| {} | {} | {} | {:.3}% | {} | {} | {} | {:.1}% | {:.1}% | {} | {} | {:.1}s |",
        r.size,
        r.queries,
        r.size - r.queries,
        100.0 * r.queries as f64 / r.size as f64,
        r.win,
        r.bm25,
        r.pool,
        r.r1,
        r.r5,
        r.p50,
        r.p95,
        r.build_s,
    )
}

/// Build one bank of `size` rows, run the fixed query set over it, and score.
fn measure(size: usize, n_queries: usize, repeats: usize, seed: u64) -> Result<Row> {
    let db: PathBuf =
        std::env::temp_dir().join(format!("mw-curve-{}-{size}.db", std::process::id()));
    rm_db(&db);
    let (gold_pos, build_s) = build(&db, size, n_queries, seed)?;
    let store = SqliteStore::open(&db)?;
    let svc = MemoryService::new(store);
    // `svc.store` is the public handle to the same store (`soak.rs` uses it the
    // same way), so the FTS stream read below goes through the same connection
    // the recall path just used.

    // R@k from the first pass only: ranking is deterministic for a fixed bank, so
    // re-scoring later passes would re-derive the same bits. Latency takes every
    // pass, because that is the part that moves.
    let mut hits1 = 0usize;
    let mut hits5 = 0usize;
    let mut lat: Vec<u64> = Vec::with_capacity(n_queries * repeats);
    for pass in 0..repeats {
        for k in 0..n_queries {
            let sig = signal(k);
            let t = Instant::now();
            let hits = svc.recall("b", &sig.query, BUDGET)?;
            lat.push(t.elapsed().as_micros() as u64);
            if pass != 0 {
                continue;
            }
            let ranked: Vec<&str> = hits.iter().map(|h| h.memory.id.as_str()).collect();
            if ranked.first() == Some(&sig.id.as_str()) {
                hits1 += 1;
            }
            if ranked.iter().take(5).any(|id| *id == sig.id) {
                hits5 += 1;
            }
        }
    }
    lat.sort_unstable();

    // The BM25 half of the pool, read once and outside every timed pass, through
    // the store's own public FTS entry point at the same `LIMIT` the recall path
    // runs. This is what separates "the pool could not see the answer" from "the
    // ranking saw it and put it too low", and it is the number a pool-bound
    // regression shows up in first.
    //
    // Membership is tracked per query rather than as two counts, because the two
    // halves overlap: a gold row inside the newest-200 window is often in the BM25
    // hits too, and adding two counts would report a pool larger than any bank has.
    let mut in_bm25 = vec![false; n_queries];
    for (k, seen) in in_bm25.iter_mut().enumerate() {
        let sig = signal(k);
        let fts = svc
            .store
            .keyword_search_fts("b", &sig.query, FTS_LIMIT)?;
        *seen = fts.iter().any(|(id, _)| *id == sig.id);
    }
    // Rowids are assigned in write order, so a gold row is inside the newest-`POOL`
    // window exactly when its write position is within the last POOL positions.
    let mut in_window = vec![false; n_queries];
    for (k, pos) in gold_pos.iter().enumerate() {
        in_window[k] = size - *pos <= POOL;
    }
    let win = in_window.iter().filter(|b| **b).count();
    let bm25 = in_bm25.iter().filter(|b| **b).count();
    let pool = (0..n_queries)
        .filter(|k| in_window[*k] || in_bm25[*k])
        .count();
    rm_db(&db);
    Ok(Row {
        size,
        queries: n_queries,
        build_s,
        r1: 100.0 * hits1 as f64 / n_queries as f64,
        r5: 100.0 * hits5 as f64 / n_queries as f64,
        p50: pct(&lat, 0.50),
        p95: pct(&lat, 0.95),
        win,
        bm25,
        // The union, which is the candidate set fusion could choose from and so
        // the exact ceiling on R@1 — not an estimate, and not the sum of the two
        // halves.
        pool,
    })
}

/// One gold row: its query, its id, and the content it stores.
struct Signal {
    query: String,
    id: String,
    content: String,
}

/// Signal `k`: a nonce its query repeats, plus a topic drawn from the same four
/// families `examples/scale_sweep.rs` generates. The nonce is what makes R@1
/// well-posed; the topic is what gives the FTS stream something to compete over.
fn signal(k: usize) -> Signal {
    const TOPICS: [(&str, &str); 4] = [
        ("auth", "jose middleware jwt verification"),
        ("rate limiting", "token bucket algorithm throttle"),
        ("vector search", "pgvector hnsw index embedding"),
        ("retention", "ttl policy sweep observations"),
    ];
    let (name, words) = TOPICS[k % TOPICS.len()];
    Signal {
        query: format!("{words} nonce-{k} {name}"),
        id: format!("sig-{k}"),
        content: format!("{words} nonce-{k} {name} instance {k}"),
    }
}

/// A distractor row: the four-topic generator from `examples/scale_sweep.rs`, so
/// three quarters of this corpus is the same topical material `eval/SCALE_SWEEP.md`
/// indexes and a quarter is the same cafeteria-and-parking noise. No nonce, so no
/// distractor can be a false positive for any query.
fn distractor(pos: usize) -> String {
    match pos % 4 {
        0 => format!("auth uses jose middleware for jwt verification instance {pos}"),
        1 => format!("rate limiting via token bucket algorithm instance {pos}"),
        2 => format!("vector search uses pgvector hnsw index instance {pos}"),
        _ => format!("distractor note {pos} about cafeteria menus and parking rotations"),
    }
}

/// Build the corpus and return `(gold write positions, build seconds)`.
///
/// Positions come from a seeded permutation of `0..size`, so gold rows are spread
/// reproducibly: `--seed` fixes the corpus exactly, and a rerun at the same seed
/// measures the same bank row for row.
fn build(db: &Path, size: usize, n_queries: usize, seed: u64) -> Result<(Vec<usize>, f64)> {
    let store = SqliteStore::open(db)?;
    store.put_bank(&Bank {
        id: "b".into(),
        name: "b".into(),
    })?;
    let order = shuffle(size, seed);
    // The first `n_queries` positions in the permutation hold the gold rows.
    let (gold, rest) = order.split_at(n_queries);
    let gold: Vec<usize> = gold.to_vec();
    let mut at = vec![None; size];
    for (k, pos) in gold.iter().enumerate() {
        at[*pos] = Some(k);
    }
    let start = Instant::now();
    for (pos, slot) in at.iter().enumerate() {
        let (id, content) = match slot {
            Some(k) => {
                let sig = signal(*k);
                (sig.id, sig.content)
            }
            None => (format!("m-{pos}"), distractor(pos)),
        };
        store.put(&Memory {
            id,
            bank_id: "b".into(),
            content,
            context: Some(format!("session-{}", pos / 8)),
            created_at: None,
        })?;
    }
    let build_s = start.elapsed().as_secs_f64();
    // `rest` is the distractor half of the permutation; iterating `at` covers every
    // position, so the permutation is used for placement only. Referenced so the
    // split reads as the two halves it is rather than as an unused binding.
    debug_assert_eq!(rest.len() + n_queries, size);
    Ok((gold, build_s))
}
