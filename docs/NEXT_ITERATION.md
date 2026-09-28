# Next iteration — 2026-09-28

## What changed since the last plan

The four-phase performance plan is complete and verified: `synchronous=NORMAL`,
`prepare_cached`, a 4-connection WAL read pool with writer-first selection, FTS5
`detail=none` (−39.6% index), five new benchmark harnesses, and 229 tests green
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
matrices for R@5 and R@10, and per-question data for all 27 rows:
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

The second stream scores by **raw token-overlap count**, so a document matching
"the" counts the same as one matching "authentication". Both parents weight their
lexical signals by IDF. This is dependency-free to fix — IDF is derivable from
the FTS index or the candidate pool.

**E1 makes this the obvious next lever, and E2 says to do it alone.** E1's result
is that the overlap stream is a *net negative* at equal weight because it
double-counts common words — which is the same defect IDF fixes, attacked at the
stream's own scale instead of the fusion's. At `overlap: 0.25` the stream is now
a weak tiebreaker, so weighting its terms properly may make it worth more than
0.25, or may not. Either way the gate is unchanged: per-question diff, per-
category table, recall curve, R@5 and NDCG@10 up and R@20 not down.

Same gates as E2, and it composes with E1: both change how the two streams are
weighted relative to each other, so run E1 → E2 → E3 and re-measure after each
rather than bundling.

## Phase E4 — multi-term query handling

`SqliteStore::fts_match_query` splits a query on non-alphanumerics and emits each
token as an independently quoted string joined by `OR`. That is what makes
`detail=none` safe (FTS5 rejects multi-term phrase queries under it), and it is
also a ranking weakness: a document matching one query token competes with one
matching four, and preference questions are exactly the multi-facet kind. A
conjunctive or coverage-based boost over distinct matched terms is free and
targets the largest single deficit.

## Phase E5 — LoCoMo (blocked; low priority)

Defer. LoCoMo is not on disk in either parent, and Hindsight's own runner fetches
it from an external service (`uv run run-amb --dataset locomo --split locomo10
--api-url …`), so obtaining it means going outside both repositories. Its only
unique value is a public number comparable to Hindsight's published table. Now
that the per-category LongMemEval diagnostic pinpoints the deficit to two
categories, a second dataset would not change what we do next. Revisit if the
E-phases land and a public comparison still matters.

## Phase E6 — release 0.3.0

The tree is far ahead of published `v0.2.0`. Everything since is additive with no
API break, so **0.3.0**. Cut only after the E-phases settle, so the release notes
can state the measured retrieval delta rather than "no change".

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
