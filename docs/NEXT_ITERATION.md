# Next iteration — 2026-09-28

## What changed since the last plan

The four-phase performance plan is complete and verified: `synchronous=NORMAL`,
`prepare_cached`, a 4-connection WAL read pool with writer-first selection, FTS5
`detail=none` (−39.6% index), five new benchmark harnesses, and 237 tests green
with CI on `main`.

Retrieval quality is **unchanged** by all of it — every one of the 500
LongMemEval per-question values is bit-identical before and after. That was the
goal, and it was met.

The open question was "are we at parity with the parent projects?" That question
is now answerable with data rather than estimate, because both parents are real
git clones with committed results on the identical benchmark.

## Parity, measured

Agentmemory ships `benchmark/LONGMEMEVAL.md` plus committed per-question JSON for
both arms: LongMemEval-S, 500 questions, `recall_any@K`, dataset
`xiaowu0162/longmemeval-cleaned`, no LLM in the loop. Same benchmark, same
metric, same question count as ours.

| System | R@5 | R@10 | R@20 | NDCG@10 | MRR |
|---|---|---|---|---|---|
| agentmemory BM25+Vector | 95.2% | 98.6% | 99.4% | 87.9% | 88.2% |
| agentmemory BM25-only | 86.2% | 94.6% | 98.6% | 73.0% | 71.5% |
| **memory-wire, equal-weight fusion (this section's baseline)** | **93.0%** | 97.4% | **99.6%** | 83.5% | 83.9% |
| **memory-wire, as shipped after E1** | **97.2%** | **98.6%** | **99.6%** | **88.2%** | **89.2%** |

**We were not at parity.** Against the equal-weight baseline we trailed their
hybrid by 2.2pp R@5 and 4.4pp NDCG@10. **Phase E1 closed that and passed it**:
the shipped fusion now leads their hybrid by **+2.0pp R@5 and +0.3pp NDCG@10**,
with the gain concentrated in exactly the two categories this table localises
the deficit to. The "bit-identical" results are our own pre/post baseline, not a
match with agentmemory.

### The deficit is concentrated, and it is a ranking deficit

Per question type, ours computed from `eval/results.json` against their published
table:

| Type | n | MW R@5 | AM hybrid | Δ R@5 | MW R@10 | Δ R@10 |
|---|---|---|---|---|---|---|
| knowledge-update | 78 | 98.7% | 98.7% | +0.0 | 100.0% | +0.0 |
| single-session-user | 70 | **98.6%** | 90.0% | **+8.6** | 98.6% | +1.5 |
| multi-session | 133 | 96.2% | 97.7% | −1.5 | 98.5% | −1.5 |
| temporal-reasoning | 133 | 94.0% | 95.5% | −1.5 | 97.0% | −0.7 |
| single-session-assistant | 56 | 87.5% | 96.4% | **−8.9** | 96.4% | −1.8 |
| single-session-preference | 30 | 56.7% | 83.3% | **−26.6** | 86.7% | −10.0 |
| overall | 500 | 93.0% | 95.2% | −2.2 | 97.4% | −1.2 |

`single-session-preference` + `single-session-assistant` are 86 of 500 questions
and contribute **−2.60pp of the −2.20pp overall gap**; single-session-user gives
back +1.20pp.

The decisive detail is the R@5→R@10 movement. On preference questions we go
56.7% → 86.7% (+30.0pp); agentmemory goes 83.3% → 96.7% (+13.4pp). **The gold
session is being retrieved and ranked 6th–10th.** That is a reordering problem,
not a coverage problem — which is the same argument used to decline a vector arm,
now measured rather than assumed. A perfect reranker would put us at 97.4 R@5;
closing half the preference+assistant deficit would put us at ~94.3.

## Phase E1 — RRF fusion weight sweep (do first)

`src/recall.rs` fused with **equal-weight RRF, k=60**. Agentmemory uses weighted
RRF (0.4 keyword / 0.6 vector / 0.3 graph) plus a 5% cross-stream agreement
bonus. We had two streams and no weights.

### E1 result — ACCEPTED, `overlap: 0.25`

`FusionWeights` (four `f64`s: `bm25`, `overlap`, `agreement`, `k`) now carries the
fusion, defaulting to the swept point, and `examples/sweep_fusion.rs` sweeps it.
The harness builds one index per question and scores **every** configuration on
it, so two rows cannot disagree because an index built differently, and the grid
is cheap enough to re-run on any change to the fusion. Full grid, per-category
matrices for R@5 and R@10, and per-question data for all 46 rows:
`eval/SWEEP_FUSION.md`.

| | R@5 | R@10 | R@20 | NDCG@10 | MRR | R@5 up/down |
|---|---|---|---|---|---|---|
| control: equal weight, k=60 | 93.0% | 97.4% | 99.6% | 83.5% | 83.9% | — |
| **shipped: overlap 0.25, k=60** | **97.2%** | **98.6%** | **99.6%** | **88.2%** | **89.2%** | **23 / 2** |
| A: overlap 0.00 (BM25 only) | 97.0% | 99.0% | 99.6% | 89.9% | 91.4% | 23 / 3 |
| A: overlap 0.50 | 95.4% | 98.4% | 99.6% | 86.5% | 87.4% | 14 / 2 |
| A: overlap 0.75 | 94.0% | 98.2% | 99.6% | 85.1% | 85.5% | 7 / 2 |
| A: overlap 1.25 → 2.00 | 92.8% → 91.6% | | 99.6% → 99.4% | 82.7% → 81.0% | | 1 / 2 → 1 / 8 |
| B: equal weight, k ∈ {10, 20, 120} | 95.0 / 94.6 / 92.6% | | 99.6% | 85.3 / 85.1 / 84.7% | | 8 / 0 → 0 / 2 |
| C: agreement ∈ {0.05…0.50} | *identical to the shipped row at every magnitude* | | | | | 0 / 0 |
| D: overlap only | 88.4% | 93.6% | 97.4% | 77.7% | 79.0% | 2 / 25 |
| E: the A×B corner | ≤ 97.2% | | 99.6% | ≤ 88.3% | | — |

**Gate: PASS.** R@5 +4.2pp, NDCG@10 +4.8pp, R@20 unchanged at 99.6%. The
per-category table moves only in the intended direction:

| Type | n | R@5 before | R@5 after | Δ | R@10 Δ |
|---|---|---|---|---|---|
| single-session-preference | 30 | 56.7% | 86.7% | **+30.0** | +6.7 |
| single-session-assistant | 56 | 87.5% | 100.0% | **+12.5** | +3.6 |
| temporal-reasoning | 133 | 94.0% | 96.2% | +2.3 | +1.5 |
| knowledge-update | 78 | 98.7% | 100.0% | +1.3 | +0.0 |
| multi-session | 133 | 96.2% | 97.0% | +0.8 | +0.8 |
| single-session-user | 70 | 98.6% | 98.6% | +0.0 | +1.4 |

**No category regressed**, and the two that carried −2.60pp of the −2.20pp
overall gap now carry **+2.29pp**. The two questions that regressed are
`d3ab962e` (multi-session) and `29f2956b_abs` (single-session-user) — both
already at 98.6–100% before, so neither is the deficit.

**The finding is that the token-overlap stream was a net negative as a co-equal
voter, and the reason is structural rather than incidental.**
`fts_match_query` ORs every query token, so BM25's own ordering already accounts
for term matching; adding a raw distinct-token-count voter at equal weight
promotes documents that merely repeat the query's words over documents BM25
ranked on term rarity. On a multi-facet preference question ("what do I prefer
for X") that is exactly backwards. The stream is not useless — see the cost
below — but at 1.0 it is worth **−4.2pp R@5**.

**The cross-stream agreement bonus is measured-and-rejected.** Swept at
0.05/0.10/0.25/0.50 in its own section, it moved **0 of 500 questions** at every
magnitude, in both the equal-weight and the shipped-weight settings. The field
stays in the struct only because it is what made that measurement expressible;
it is not a knob this build turns. Agentmemory's 5% is not portable to a
two-stream fusion whose streams already agree on their ordering.

**What E1 costs, measured on the two harnesses LongMemEval cannot see.** The
per-category table above is the designated instrument, and it is unanimous. It
is not the only instrument, and pretending otherwise is how the `detail=none`
latency claim ended up unsupported:

| Suite | equal weight | overlap 0.25 | Δ |
|---|---|---|---|
| coding-life R@5 (n=15) | 100.0% | 96.7% | **−3.3** (1 question) |
| coding-life P@5 | 24.0% | 22.7% | −1.3 |
| recall-curve R@5 @1k | 100.0% | 96.9% | **−3.1** |
| recall-curve R@5 @10k | 100.0% | 84.4% | **−15.6** |
| recall-curve R@5 @50k | 100.0% | 90.6% | **−9.4** |
| recall-curve R@5 @100k | 100.0% | 90.6% | **−9.4** |

All 32 gold rows still reach BM25's top-50 at every size, so the 200-row fusion
pool is still not the binding constraint; what changed is how **near-tied**
candidates are ordered. Both suites measure finding a needle whose one
discriminating token is rare among many common ones — the case a token-count
voter is built for. LongMemEval measures the opposite case and gains 4.2pp.
`bench_recall_curve` says so about itself: "R@1 here says a retrieval pipeline
can find an identifiable needle in a haystack of a given size, not that it
answers natural multi-session questions."

**The trade-off is not a cliff, and the middle is measured.** Sweeping
`bench_recall_curve` across the weight, with the LongMemEval R@5 from the grid:

| overlap | curve R@5 @1k / 10k / 50k / 100k | LongMemEval R@5 | NDCG@10 |
|---|---|---|---|
| 1.00 (was shipped) | 100 / 100 / 100 / 100 | 93.0% | 83.5% |
| **0.75** | **100 / 100 / 100 / 100** | **94.0%** | **85.1%** |
| 0.50 | 96.9 / 90.6 / 96.9 / 96.9 | 95.4% | 86.5% |
| **0.25 (shipped)** | 96.9 / 84.4 / 90.6 / 90.6 | **97.2%** | **88.2%** |

`overlap: 0.75` holds both needle suites at 100% and still gains **+1.0pp R@5 /
+1.6pp NDCG@10** on LongMemEval — but it leaves the preference deficit 26.7pp of
its 30.0pp wide open, and it regresses two LongMemEval categories by one question
each (multi-session 96.2 → 95.5, single-session-user 98.6 → 97.1). The shipped
0.25 is the point the project's designated instrument picks; **0.75 is named here
so the choice is revisitable, not buried**, and moving to it is a one-constant
change with a committed grid either side of it.

Two further properties of the accepted change, both measured rather than argued:

- **`overlap: 0.0` is *not* the right answer** despite scoring +6.4pp NDCG@10 and
  +7.5pp MRR (R@5 97.0%, one question below 0.25). BM25 is `LIMIT 50` in SQL, so
  a zero weight truncates the candidate set to 50 for any query matching more
  than 50 rows. LongMemEval cannot see that — 38–62 sessions per bank with BM25
  truncating at 50 means the suite contains no such query — while the curve
  harness at 1k–100k is built entirely out of them. Pinned by
  `a_query_matching_more_rows_than_bm25_returns_must_still_surface_the_overflow`:
  120 matching rows return 120 hits at 0.25 and exactly 50 at 0.0.
- **The k sweep is a second, smaller lever and does not compose for free.** At
  equal weight, k=10 gains +1.6pp and k=120 loses 0.4pp; at the shipped weight the
  whole corner (E) never beats 0.25/k=60 on R@5, and the two axes act on the same
  quantity — how far a low BM25 rank can be outvoted — so their additivity was
  measured, not assumed. k stays 60.

## Phase E2 — Porter stemming — REJECTED, reverted

FTS5 is on the default `unicode61`, with no stemmer. Agentmemory's own analysis
credits stemming explicitly.

### E2 result — the change works, and the metrics say no

The implementation was built and measured, and it is correct: `tokenize='porter
unicode61'` on the same `detail=none` external-content table, a second one-shot
marker (`fts_porter_stemmed`) in the existing `schema_markers` table, the same
drop-triggers / drop-table / recreate / `'rebuild'` / set-marker transaction as
`detail=none`, no new versioning scheme. Verified, not assumed: the migration
fires once and three later opens leave the index content-hash identical;
`integrity-check` passes on the stemmed table and still *fails* on a deliberately
drifted one, so the self-heal is intact; all three trigger paths work, including
the `'delete'` half of `memories_au` that must re-supply the exact original text
(the retired terms verifiably stop matching); and `caching`/`cache`/`cached`/
`caches` all reach a `cache` row, with the pre-migration index shown *not* to do
so, so the behaviour cannot pass by construction. `memories_fts_config` was
re-checked and again records only `version=4` — the tokenizer is not readable from
the schema, which is the whole reason the marker exists.

**The measurements, on top of the E1-accepted default (500 questions, seed 42,
all 2,500 per-question values diffed):**

| | R@5 | R@10 | R@20 | NDCG@10 | MRR |
|---|---|---|---|---|---|
| E1 only, unstemmed | 97.20% | 98.60% | 99.60% | 88.23% | 89.20% |
| E1 + stemming | 97.20% | 98.80% | 99.60% | 88.89% | 89.54% |
| Δ | **+0.00** | +0.20 | +0.00 | +0.65 | +0.34 |

Per question: R@5 **4 improved, 4 regressed, 492 unchanged**; R@10 3/2; R@20 1/1;
NDCG@10 66/58. Per category:

| Type | n | ΔR@5 | ΔR@10 | ΔR@20 | ΔNDCG@10 |
|---|---|---|---|---|---|
| single-session-preference | 30 | +3.33 | +0.00 | +0.00 | +0.58 |
| single-session-assistant | 56 | **−1.79** | +0.00 | +0.00 | −1.17 |
| multi-session | 133 | +1.50 | +0.75 | +0.75 | +2.29 |
| single-session-user | 70 | +1.43 | +0.00 | +0.00 | +0.94 |
| knowledge-update | 78 | +0.00 | +0.00 | +0.00 | +0.68 |
| temporal-reasoning | 133 | **−2.26** | +0.00 | **−0.75** | −0.36 |

Recall curve at 1k / 10k / 50k / 100k: **byte-identical to the E1-only run**
(71.9/75.0/75.0/71.9 R@1, 96.9/84.4/90.6/90.6 R@5). Stemming changed nothing
there at all.

**Gate: FAIL.** R@5 did not rise — it is flat to the question — and two
categories regress, one of them (`temporal-reasoning`, 133 of 500 questions) on
R@20 as well. This is stemming over-matching, and the mechanism is legible in
which categories it hit: Porter collapses `policing`/`policy`,
`retries`/`retry`, `deployment`/`deploy`, and the inflected number and
time words that `temporal-reasoning` questions are made of, so a query acquires
competitors it did not have and pushes a gold session down.

**E1 had already taken the headroom.** On its own, at equal weight, stemming
*passes* the same gate: R@5 93.0 → 94.2, NDCG@10 83.5 → 84.7, R@20 flat. But
that is E1's deficit to close, not stemming's: on preference questions stemming
alone moved R@5 56.7 → 66.7, and reweighting alone moved it 56.7 → 86.7. The two
levers attack the same questions, and once the reweighting has lifted them into
the top 5 there is nothing left for a lexical change to win. **The two are not
additive, and the order mattered: E1 first, and E2 has nothing left to do.** That
is the finding, and it is the reason the plan sequenced E1 before E2.

Reverted cleanly. Nothing from E2 is in the tree: no second marker, no tokenizer
change, no stemming test. One idea from it survived because it is a better test
regardless — the `detail=none` migration's idempotency check now fingerprints the
index by **content hash** (`memories_fts_data` doclist blocks) instead of page
count, since a drop-and-rebuild of the same corpus can land on the same number of
pages and hide behind an equal total. That was the E2 lesson and it applies to
the migration that shipped before it.

**Not done, deliberately.** Agentmemory ships a hand-curated `synonyms.ts`.
Copying a content list is a maintenance burden and a licensing question, and
stemming is the cheaper half of the same idea — which is exactly what makes E2's
negative result informative: the cheap half of the synonym idea does not pay on
this data, so the expensive half is not worth reaching for either.

## Phase E3 — IDF weighting in the overlap stream

**REJECTED on the gate. Shipped configuration unchanged.** The lever works, the
diagnosis behind it was right, and the instrument that motivated it turns out not
to be measuring what the hypothesis assumed.

### Where the IDF comes from

From the bounded candidate pool — the newest `store::RECALL_POOL_LIMIT` (200) rows
of the bank ∪ the BM25 top-50, so `N ≤ 250` — with `df(t)` the number of those
documents containing query token `t`. The weight is the one FTS5's own `bm25()`
uses, `idf = ln(1 + (N − df + 0.5)/(df + 0.5))`. No new dependency, no schema
change, no migration, no extra SQL.

`df` is counted **in the single tokenizing pass the overlap stream already makes**
over the pool: each document's matched query tokens go into a flat bit matrix
(`n_docs × ⌈slots/64⌉` words — 2 KB at this store's pool ceiling for a query of up
to 64 distinct tokens, 4 KB for 128) as they are seen, and the weighted sum is
arithmetic over that matrix afterwards. Re-tokenizing to read `df` back off a
second pass would have doubled the dominant loop in `rank_candidates` for nothing.

Two alternatives were checked against the bundled SQLite rather than assumed:

- **The FTS5 index.** `idf()` is `no such function`; `bm25()` is a whole-document
  score, not a per-term statistic; and under `detail=none` the shadow tables
  (`t_data`, `t_docsize`, `t_idx`) are opaque blobs with no readable per-term
  counts. There is no per-term IDF in the index to read.
- **`fts5vocab`.** It exists and it does expose per-term document counts — but
  creating it is a new schema object plus a backfill of every term in the bank,
  i.e. a migration. Out of scope for a phase whose whole argument is that it costs
  nothing.

The pool is the *right* denominator independently of what is cheap: the overlap
stream ranks the pool, so "how rare is this term" is only ever asked relative to
the set being ranked.

**Unseen term** (`df == 0`): it occurs in no pool document, so its bit is never set
in the bit matrix and it contributes nothing to any score. The largest weight the
formula can produce is exactly the case that must contribute zero, and it does. The
term still goes to FTS5 unchanged, so a query token the pool happens not to hold is
still a token BM25 can match.

**Universal term** (`df == N`): `ln(1 + 0.5/(N+0.5))` — small but **strictly
positive**. A query made only of terms every document carries still returns those
documents, in a stable order. A weight that could reach zero would silently empty
the overlap stream of everything that matched; one that could go negative would rank
the worst match first. Neither is reachable, and
`a_universal_term_should_still_score_strictly_positive` pins both halves.

### The sweep — `overlap` 0.00 → 2.00, raw and IDF-weighted

500 questions, seed 42, every row scored on one shared index per question. The
full 46-row grid and all per-question data are in `eval/SWEEP_FUSION.md`; the
pairing is also printed there as its own table.

**The box was loaded throughout** — loadavg 60.42 at the start of the sweep,
24.64 at the start of the recall curve, on 24 cores, against a ~15 threshold. Every
number in this section is a retrieval metric (R@k, NDCG, MRR) and those are
load-independent and deterministic, so they stand. **No latency claim is made from
these runs, and none is comparable to a committed one.** The visible tell is in
the artifacts themselves: the same 46 configurations took 398.8s of index build
here against 160.6s in the E1 run, which is the machine, not the code.

| overlap | A: raw R@5 | A: raw NDCG@10 | F: idf R@5 | F: idf NDCG@10 | Δ R@5 | Δ NDCG | R@20 raw | R@20 idf |
|---|---|---|---|---|---|---|---|---|
| 0.00 | 97.0% | 89.9% | 97.0% | 89.9% | +0.0 | +0.0 | 99.6% | 99.6% |
| 0.25 | **97.2%** | 88.2% | 96.8% | **89.9%** | **−0.4** | **+1.6** | 99.6% | 99.6% |
| 0.50 | 95.4% | 86.5% | 96.8% | 89.3% | +1.4 | +2.8 | 99.6% | 99.6% |
| 0.75 | 94.0% | 85.1% | 96.6% | 89.1% | +2.6 | +4.1 | 99.6% | 99.6% |
| 1.00 | 93.0% | 83.5% | 96.4% | 88.6% | +3.4 | +5.2 | 99.6% | 99.6% |
| 1.25 | 92.8% | 82.7% | 96.2% | 88.5% | +3.4 | +5.8 | 99.6% | 99.6% |
| 1.50 | 92.4% | 82.2% | 96.4% | 88.5% | +4.0 | +6.4 | 99.4% | 99.6% |
| 1.75 | 92.0% | 81.7% | 96.2% | 88.4% | +4.2 | +6.7 | 99.4% | 99.6% |
| 2.00 | 91.6% | 81.0% | 96.2% | 88.3% | +4.6 | +7.4 | 99.4% | 99.6% |

**E1's diagnosis is confirmed, and then some.** E1 found the overlap stream a net
negative as a co-equal voter *because it double-counts common words*. E3 says that
was the right diagnosis: at every weight ≥ 0.50 the rarity-weighted stream beats the
raw one, by +1.4pp R@5 at 0.50 rising to +4.6pp at 2.00, and +2.8 to +7.4pp
NDCG@10. Across the ladder the raw stream collapses 97.2 → 91.6; the IDF stream
falls only 97.0 → 96.2, and it holds R@20 at 99.6% everywhere. **A rarity-weighted
overlap stream can take a full vote without the collapse the raw stream suffers.**

**And it still fails the gate, because the gain is only reachable at weights that
were already rejected.** At the shipped 0.25 the best IDF row is R@5 96.8 /
NDCG@10 89.9 against the shipped 97.2 / 88.2: **−0.4pp R@5 bought for +1.7pp
NDCG@10 and +2.2pp MRR.** Per question: two lost, none gained, and both are still
inside R@10 — `07b6f563` (single-session-preference) and `88432d0a` (multi-session)
move from the top 5 into positions 6–10. Nothing leaves the top 20. Per category:

| Type | n | ΔR@5 | ΔR@10 | ΔR@20 | ΔNDCG@10 |
|---|---|---|---|---|---|
| single-session-preference | 30 | **−3.33** | +0.00 | +0.00 | +5.79 |
| multi-session | 133 | −0.75 | +0.00 | +0.00 | +0.22 |
| single-session-assistant | 56 | +0.00 | +0.00 | +0.00 | +7.22 |
| single-session-user | 70 | +0.00 | +0.00 | +0.00 | +1.02 |
| knowledge-update | 78 | +0.00 | +0.00 | +0.00 | +0.00 |
| temporal-reasoning | 133 | +0.00 | +1.54 | +0.00 | +1.10 |

One persistent casualty across the whole sweep: `07b6f563` is also lost by raw at
0.75. At 0.75, **raw loses 16 questions and IDF loses 3** — which is the strongest
argument for the lever, and it is an argument about 0.75.

### The recall curve says the motivating mechanism does not exist

R@5, 32 fixed queries (one query = 3.125pp), raw against IDF at the same weight:

| overlap | raw: 1k / 10k / 50k / 100k | IDF: 1k / 10k / 50k / 100k |
|---|---|---|
| 0.25 | 96.9 / 84.4 / 90.6 / 90.6 | 96.9 / 84.4 / 90.6 / 90.6 |
| 0.50 | 96.9 / 90.6 / 96.9 / 96.9 | 96.9 / 90.6 / 96.9 / 96.9 |
| 0.75 | 100.0 / 100.0 / 100.0 / 100.0 | 100.0 / 100.0 / 100.0 / 100.0 |
| 1.00 | 100.0 / 100.0 / 100.0 / 100.0 | 100.0 / 100.0 / 100.0 / 100.0 |

R@1 likewise: identical at every size for 0.50, 0.75 and 1.00 (at 1.00, both
78.1 / 75.0 / 81.2 / 75.0). At 0.25 the per-query gold-rank vectors do differ — 8 of
32 queries at 1k, 9 of 32 at 50k and 100k — so IDF genuinely reorders the tail. But
nothing crosses into or out of the top 5, so R@1 and R@5 are unchanged at every size.

**The proposed mechanism cannot be what is happening.** The hypothesis was that "a
distractor matching many common query tokens out-votes the gold row that matches the
one rare nonce". On this corpus it cannot: every query is six tokens — five from its
topic sentence, repeated across a quarter of the bank, plus the unique nonce — the
gold row contains all six, and a same-topic distractor contains five. **6 > 5, so the
raw count already ranks the gold row first inside the overlap stream.** There is no
ordering inversion for IDF to fix. What actually caps the stream at weight 0.25 is
not its ordering but its *vote*: 0.25 of a term against ~50 same-topic rows sitting
at BM25 ranks 1–50, with RRF's k=60 damping on top. IDF re-ranks correctly and
changes nothing, because the ranking was already correct.

`coding-life` is the third, independent witness and says the same thing: R@5 96.7%
at `overlap` 0.25 (raw *and* IDF) and 100.0% at every weight from 0.50 up (raw *and*
IDF), with the missed-query id list identical. Full table in `eval/CODING_LIFE.md`.
**Both needle instruments respond to the overlap weight and to nothing else.**

### What E3 changed, and what it did not

E3 set out to dominate the 0.25-vs-0.75 argument. It half-does:

- **The recall curve cannot tell the two scorers apart at any weight.** Raw 0.75 and
  IDF 0.75 both give 100% at all four sizes; raw 0.50 and IDF 0.50 both give
  96.9 / 90.6 / 96.9 / 96.9. **IDF buys nothing on the instrument that motivated it.**
- **LongMemEval: the cost of a full overlap vote drops from −3.2pp R@5 (raw 0.75 =
  94.0) to −0.6pp (IDF 0.75 = 96.6), and NDCG@10 from −3.1pp to +0.9pp.** The same
  +15.6pp of curve R@5 at 10k costs 5.3× less LongMemEval R@5. That is a real
  improvement to the shape of the trade.

But the gate is an AND, and IDF 0.75 holds the curve at 100% while LongMemEval R@5
lands at 96.6, below the 97.2 floor. The Pareto frontier moved; the shipped point
did not become admissible. **Nothing is shipped.**

**Deficit closed: none, and slightly reversed.** LongMemEval R@5 deficit vs perfect
goes 2.8pp → 3.2pp. Recall-curve R@5 deficit at 10k stays 15.6pp. coding-life R@5
stays 96.7%. E3's whole contribution is a *re-pricing*: if the recall curve ever
becomes the binding constraint, the right configuration is
`overlap: 0.75, overlap_idf: true`, not `overlap: 0.75` — a conclusion that costs
2.6pp of LongMemEval R@5 today and would be free to take the moment the curve
mattered more.

**Kept in the tree, default off.** `FusionWeights::overlap_idf` ships `false`, and
`false` runs the *original* single pass on the original scratch buffer — the
un-weighted path is asserted to reproduce `overlap_score` exactly, for six queries
across three corpora, in
`the_unweighted_path_should_reproduce_the_raw_distinct_token_count`. The E1 grid rows
in `eval/SWEEP_FUSION.md` therefore still reproduce the committed E1 numbers rather
than merely resembling them. This is the same treatment `agreement` got: measured,
rejected, kept only because the field is what made the measurement expressible.

## Phase E4 — multi-term query handling

**REJECTED on the gate, and the finding is stronger than "it did not help": a
distinct-term-coverage signal is arithmetically identical to the overlap weight.**

### What was built

A third RRF stream over the same candidate pool the overlap stream ranks, ordered by
`distinct query tokens present ÷ distinct query tokens in the query`, weighted by
`FusionWeights::coverage` and dropped entirely at 0.0. `fts_match_query` is
**unchanged**: still one independently double-quoted token per query token joined by
`OR`, never a multi-term phrase, because FTS5 rejects a phrase under `detail=none`
and emitting one would break every existing database at query time. The distinct
count is read out of the overlap stream's own pass (`score_doc` now returns
`(score, matched)`) and re-sorted in `api.rs`; nothing re-tokenizes, and at
`coverage: 0.0` the stream is not built at all.

### The finding

**For any query with no repeated token, the coverage stream's rank list is identical
to the overlap stream's.** The denominator is constant within one query, so ranking
by coverage is ranking by distinct-match count, and the overlap score is a monotone
function of that same count. Identical orderings contribute identically to the
fusion, and the fusion is linear in the weights:

    1.0/(60+r) + 0.25/(60+r) + w/(60+r)  ==  1.0/(60+r) + (0.25+w)/(60+r)

**`coverage: w` is not a new lever. It is the E1 overlap-weight sweep wearing a
different name, and E1 already priced it.** Confirmed at rank level rather than in
the aggregate: on all 32 recall-curve queries at 10k, `shipped + coverage=0.25`
produces the gold-rank vector

    [4,0,0,4,1,5,5,5,2,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]

byte-identical to `raw overlap=0.50`'s.

The two orderings diverge only on a query that *repeats* a token, where the raw
score charges the repeat and coverage does not. There the residual is small and it
runs the wrong way on the category it was aimed at:

| coverage | R@5 | R@10 | R@20 | NDCG@10 | MRR | ≡ raw overlap | R@5 there | NDCG there |
|---|---|---|---|---|---|---|---|---|
| 0.05 | 97.0% | 98.4% | 99.6% | 87.7% | 88.6% | 0.30 | — | — |
| 0.10 | 96.6% | 98.4% | 99.6% | 87.6% | 88.6% | 0.35 | — | — |
| 0.25 | 95.4% | 98.4% | 99.6% | 86.5% | 87.5% | 0.50 | 95.4% | 86.5% |
| 0.50 | 94.0% | 98.2% | 99.6% | 85.1% | 85.6% | 0.75 | 94.0% | 85.1% |
| 1.00 | 93.2% | 97.2% | 99.6% | 82.8% | 83.4% | 1.25 | 92.8% | 82.7% |

Shipped for reference: 97.2 / 98.6 / 99.6 / 88.2 / 89.2. R@20 never moves.

At `coverage: 0.25`, the worst row: `single-session-preference` 86.7 → 73.3
(−13.3) and `single-session-assistant` 100.0 → 94.6 (−5.4), with every other
category within 0.8pp. **The conjunctive signal is worst exactly where it was
supposed to be best.** That is not a coincidence — it is the E1 curve again:
preference questions are the ones that lose when the overlap stream is loud, and a
coverage stream at weight 0.25 *is* the overlap stream at 0.50.

Recall curve R@5: 96.9 / 84.4 / 90.6 / 90.6 at `coverage: 0.10`, 96.9 / 90.6 / 96.9 /
96.9 at 0.25, 100.0 everywhere at 1.00 — the identical curve E1 already measured for
`overlap` 0.35, 0.50 and 1.25, and rejected for the identical reason.
`coding-life`: 96.7% at `coverage: 0.10`, 100.0% at 0.25 and 1.00, matching `raw
overlap` 0.35 / 0.50 / 1.25 exactly.

**Gate: FAIL.** Monotonically harmful on LongMemEval at every weight tested, with the
damage concentrated in the two categories the lever targets, and no offsetting gain
anywhere. **Deficit closed: none.** LongMemEval R@5 deficit 2.8pp → 3.0pp at best
(`coverage: 0.05`) and 7.0pp at `coverage: 1.00`; the other two instruments are
unchanged from what the equivalent overlap weight already gives.

**Why this is worth more than the number.** E3 asked whether a smarter *score* helps
and the answer was "not on the instrument that motivated it". E4 asked whether a
*different signal* helps and the answer is that the signal is not different — it
reduces algebraically to the weight E1 already swept. The productive question for
this half of the space is not "what else can the overlap stream know about the
query" but "what does the overlap stream know about the *document* that a token
count throws away" — length, field structure, position. All of those need a scoring
change rather than a weighting change, and all of them are unmeasured.

Kept in the tree, default off, same treatment as E3 and `agreement`.

**Update, after both phases were measured and rejected.** `overlap_idf` and `coverage` were then removed from the tree — the fields, the IDF scorer, the third stream, the F and G grid rows in `examples/sweep_fusion.rs`, and their tests — which is the same treatment E2's stemming code got, and the findings above are the reason it was safe: a lever that can never be turned on is not configuration. `eval/SWEEP_FUSION.md` is deliberately **not** regenerated and its F/G rows stay exactly as written; that artifact is the committed evidence for this removal. No retrieval number moved: the raw single-pass scorer the E1 grid measures was left byte for byte, and the shipped row still reads 97.2 / 98.6 / 99.6 / 88.2 / 89.2 with the control still 93.0. `agreement` is left in place, unlike E3/E4 — it is a swept axis of the live E1 harness, not extra code.

## Phase E5 — LoCoMo (blocked; low priority)

Defer. LoCoMo is not on disk in either parent, and Hindsight's own runner fetches
it from an external service (`uv run run-amb --dataset locomo --split locomo10
--api-url …`), so obtaining it means going outside both repositories. Its only
unique value is a public number comparable to Hindsight's published table. Now
that the per-category LongMemEval diagnostic pinpoints the deficit to two
categories, a second dataset would not change what we do next. Revisit if the
E-phases land and a public comparison still matters.

## Phase E6 — release 0.3.0 (cut; this is the record of what was cut)

**0.3.0 is cut.** `Cargo.toml` says `0.3.0` and `v0.3.0` is tagged, so the phase
ran as written — and the reason the phase existed is the one it named: everything
since `0.2.0` was additive with no API break, and the cut could state the measured
retrieval delta from E1 (+4.2pp R@5, +4.8pp NDCG@10) rather than "no change".

What is in the tag: the `0.2.0` feature set plus E1's reweighting. The E2, E3 and
E4 levers are **not** in it — all three were removed from `src/` before the cut
(`git show v0.3.0:src/recall.rs` already shows `FusionWeights` carrying only
`bm25`, `overlap`, `agreement` and `k`). E7's oracle harness landed on `main` after
the tag, so it is not in the release either; `eval/ORACLE_RERANK.md` and
`examples/oracle_rerank.rs` are `main`-only.

## Sequencing and honesty rules

1. One lever per measurement. E1→E2→E3→E4, re-running the full battery after
   each. Bundling them makes the result unattributable, which is how the
   `detail=none` latency claim ended up unsupported in the first place.
2. Every retrieval change gates on the **per-question** diff, not the aggregate.
   An aggregate that holds can still be two categories moving in opposite
   directions.
3. Latency claims require an **idle box**. The §13 re-measurement showed the
   loaded box inflated levels 1.1–2.5× and the noise floor by 2.9×; any
   difference under ~2.5× on a loaded box is not a measurement.
4. Record null results. "Equal weights were already optimal" and "stemming
   over-matched" are findings.

## Where E1–E4 left the retrieval weights

`overlap: 0.25`, `k: 60` — unchanged from E1 (and now the *only* weights the struct
carries: `overlap_idf` and `coverage` have since been removed, see the E4 update
above), and justified against three consecutive rejected attempts to move it
rather than against one benchmark. The shape of the argument:

| | what wants a higher `overlap` | what the higher weight costs |
|---|---|---|
| LongMemEval (500 q) | +4.2pp R@5 from 1.0 → 0.25 | −3.2pp R@5 from 0.25 → 0.75 (−0.6pp with IDF) |
| recall curve (32 q × 4 sizes) | +15.6pp R@5 at 10k from 0.25 → 0.75 | nothing measured |
| coding-life (15 q) | +3.3pp R@5 from 0.25 → 0.50 | nothing measured |

Both needle instruments want 0.50–0.75 and cannot tell a rarity-weighted overlap
stream from a raw one at any weight; LongMemEval wants 0.25 and is the only
instrument that punishes raising it. Every attempt to make one instrument stop
disagreeing with the other by changing *what the overlap stream knows* has failed
— E2 (stemming, different problem, same answer), E3 (IDF), E4 (coverage, which
turned out not to be a new signal at all).

The next lever that is not a re-run of the weight sweep has to be about the
**document** rather than the query: length, field structure, position, or a
proximity term. None of those is measured. The one thing this map does establish
is the ceiling: at 0.75 the fusion reaches 100% R@5 on both needle instruments and
loses only 0.6pp of LongMemEval R@5, so the headroom that E3 identified is real and
sized — it just is not free yet, and buying it is a data problem, not a weighting
one.

## Phase E7 — the oracle-rerank ceiling (sizing, not building)

The deficit section above argues from R@5→R@10 movement that this is a reordering
problem, not a coverage problem. That argument was inference. It is now measured:
`examples/oracle_rerank.rs` builds, per question, the exact pool `MemoryService::recall`
ranks — the same three primitives, the same `FusionWeights::SHIPPED`, in one pass, with
the reconstructed pool checked row-for-row and score-for-score against what recall
serves — and asks where the first gold row actually sat. Artifact:
`eval/ORACLE_RERANK.md`, 500 questions, seed 42.

| First gold rank in pool | 1 | 2–5 | 6–10 | 11–20 | 21–50 | 51–200 | not in pool |
|---|---|---|---|---|---|---|---|
| Questions | 419 | 67 | 7 | 5 | 2 | 0 | 0 |

**Oracle R@5 is 100.0%, not 99.6%.** The 99.6% is the *current* R@20; pool coverage
itself is 500/500, so a perfect reranker over the pool this build already assembles
would post R@5 = 100.0 against today's 97.2. The whole 2.8pp R@5 deficit is
rerankable: of the 14 questions recall misses at rank 5, **12** have their gold row at
rank 6–20, **2** at rank 21–50, and **0** are missing from the pool. The mass is
concentrated — `single-session-preference` supplies 4 of the 12 on 30 questions
(86.7% → 100.0%), `temporal-reasoning` 5 of them on 133 — and `knowledge-update` and
`single-session-assistant` are already at 100% with nothing to gain.

Two corollaries the artifact states and this plan should not lose. The **larger** prize
is at the top: current R@1 is 83.8% against the same 100.0% oracle, **+16.2pp** against
+2.8pp at R@5, with 419 questions already at rank 1 and 81 not. And the thing that
actually binds on this suite is **not** the 200-row candidate window — the bank is only
38–62 rows, so `recall_inputs` reads all of it — but the **token budget**: the
100,000-token budget leaves a caller 26/35/47 rows against a pool of 38/47/62, on all 500
questions. That costs nothing at R@5/10/20 (0 questions diverge before rank 20), but it
does hide the deep candidates a reranker would be reordering.

What this does **not** settle: with 0 questions missing from the pool, this suite has no
coverage failure for any new retrieval signal to fix, so it can neither support nor
refute a vector arm or an LLM. And an oracle is a ceiling computed with the gold labels,
not a prediction — it sizes the prize, it does not say any reranker closes it.
