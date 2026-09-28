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
| **memory-wire** | **93.0%** | 97.4% | **99.6%** | 83.5% | 83.9% |

**We are not at parity.** We lead their BM25-only arm by +6.8pp R@5 and +10.5pp
NDCG@10, and we lead on R@20. We trail their hybrid by 2.2pp R@5 and 4.4pp
NDCG@10. The "bit-identical" results are our own pre/post baseline, not a match
with agentmemory.

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

`src/recall.rs` fuses with **equal-weight RRF, k=60**. Agentmemory uses weighted
RRF (0.4 keyword / 0.6 vector / 0.3 graph) plus a 5% cross-stream agreement
bonus. We have two streams and no weights.

Sweep the BM25-vs-overlap weight over the 500-question set. This is pure
arithmetic on signals that already exist: no new dependency, no schema change,
no migration. A full run is ~20 seconds, so the whole sweep is minutes.

Gate: R@5 and NDCG@10 improve, **R@20 must not regress**, and the per-category
table must be recomputed so we can see *where* it moved. A null result is a
legitimate outcome and should be recorded as one — equal weights may already be
optimal, in which case this closes a question rather than a gap.

## Phase E2 — Porter stemming

FTS5 is on the default `unicode61`, with no stemmer. Agentmemory's own analysis
credits stemming explicitly. The change is `tokenize='porter unicode61'` and we
already have the one-shot marker machinery from `detail=none`
(`schema_markers`), so the migration is a second marker plus a rebuild.

Unlike `detail=none`, **this is a semantic change and the metrics will move.**
That is the point, and it is also the risk: stemming over-matches. Gate on the
full 500-value per-question diff, the per-category table, and the recall curve at
1k–100k. Accept only if R@5/NDCG@10 rise and R@20 does not fall.

## Phase E3 — IDF weighting in the overlap stream

The second stream scores by **raw token-overlap count**, so a document matching
"the" counts the same as one matching "authentication". Both parents weight their
lexical signals by IDF. This is dependency-free to fix — IDF is derivable from
the FTS index or the candidate pool.

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
