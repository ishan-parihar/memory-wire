# Performance plan

Follows `docs/PERFORMANCE_AUDIT.md` (measured at `59eeb85`). Every phase below
exists because that audit found a specific, named gap. Phases with no gap behind
them are not in here.

## The target

Measured, not guessed: **oracle R@1 = 100.0% (Δ +16.2pp), oracle R@5 = 100.0%
(Δ +2.8pp), with 500/500 pool coverage.** The pool always contains the answer.
Everything here is about ordering what we already retrieve.

Two rules bind every phase:

1. **A phase is accepted only on the full 500-question per-question diff**, never
   on an aggregate. An aggregate that holds can be two categories moving in
   opposite directions — that is how the `detail=none` latency claim ended up
   unsupportable in the first place.
2. **R@20 and NDCG@10 may not regress**, and no `question_type` may regress by
   more than 1 question. Quality only. A null result is a valid outcome and gets
   recorded as one.
3. **No parameter is selected on the test set.** Phases P−1, P2, P3 and P4 select
   against LoCoMo; LongMemEval is measured and reported, never optimised. See
   `docs/EVALUATION_HYGIENE.md`.

Latency may be claimed **only** on a quiet machine (`/proc/loadavg` under ~5 on 24
cores) and then only with the load printed. Otherwise the phase reports quality
only. This box has not been quiet for most of the project's history, so treat any
timing number without a load line as unsupported.

---

## P−1 — Build the dev harness on independent data (added 2026-09-28)

**This phase now gates every measurement-driven phase below.**

`docs/EVALUATION_HYGIENE.md` established that `overlap: 0.25` was selected by
sweeping 46 configurations against LongMemEval's 500 questions. That makes it
**fitted**, it makes our headline 97.2% a non-generalisation estimate, and it
leaves the only clean comparison we have at the unfitted **93.0%**.

So: no parameter is selected against LongMemEval again, until there is another
labelled set to select it against.

`eval/data/locomo/` already exists on disk — 272 documents, 1,540 queries,
`gold_ids` naming the gold document, `user_id` giving the isolation unit, from
Snap Research's LoCoMo via the canonical
`vectorize-io/agent-memory-benchmark` distribution. Same shape as LongMemEval,
retrieval-only (the `gold_answers` prose is not needed), different source.
`eval/download.sh` should fetch it alongside LongMemEval, not hand-placed.

An `examples/locomo.rs` harness, by analogy with `examples/longmemeval.rs`, is
the precondition for P2, P3 and P4. It measures `recall_any@{1,5,10,20}`,
per-category via `meta.category`, and is selected against.

It also settles the existing debt directly: run both `overlap: 0.25` and the
unfitted `1.00` on LoCoMo. If 0.25 wins there too, the effect replicated on
independent data and it is promoted from provisional to earned. If it loses, it
was an artifact of 500 questions and we revert to 1.00 and re-baseline the
README. **Either answer is worth more than any new model.**

Gating rule: P0 (recording R@1) is bookkeeping and may proceed — it selects
nothing. P1's `k` sweep and P2/P3/P4 all select, and all wait for this phase.

## P0 — Recover the pre-E1 R@1, and make R@1 a tracked column

**Why.** The audit's biggest number (+16.2pp at R@1) is a metric this repo has
never tracked. `eval/RESULTS.md` and the E1 sweep record R@5/10/20, NDCG@10 and
MRR — not R@1. So there is no way to tell whether R@1 has been improving or has
been flat, and the phase that targets it would have no way to prove itself.

**What.** Add `recall_any_at_1` to the `longmemeval` harness output and to
`eval/results.json`; re-run the E1 sweep grid's control row (equal weight,
k=60, raw overlap) with R@1 recorded, to recover the pre-E1 baseline; publish
both in `eval/SWEEP_FUSION.md`.

**Gate.** R@1 present in the artifact with a per-question breakdown. A metric
added, nothing else changed. Cost: under an hour.

**Falsified if** the recovered pre-E1 R@1 is already ≥83.8%, which would mean
E1 did nothing for R@1 and the target is unmoved.

## P1 — Settle what the two retrieval streams actually do, then sweep `k`

**Why.** Two named problems from the audit. First, an unresolved contradiction:
the E1 agent reported the streams "return the same documents in the same order",
but RRF over two identical rankings is monotone in rank, so R@k could not have
moved — and R@5 moved 93.0 → 97.2. One of those is wrong, and every fusion
decision rests on the answer. Second, `k = 60` has never been swept; it controls
how sharply the top of the ranking separates from the tail, which is exactly the
R@1 axis.

**What.**
1. Instrument both streams per question: dump each stream's id order and ranks,
   compute Spearman/Kendall correlation between `rank_bm25` and `rank_overlap`,
   and count how many documents each stream returns that the other does not.
   Reconcile against the `agreement` finding and correct whichever record is
   wrong — in `docs/CONSISTENCY.md` and the E1 artifact, not silently.
2. Joint grid over the RRF constant `k` and the overlap weight, reusing
   `examples/sweep_fusion.rs`. `k ∈ {5, 10, 20, 40, 60, 120}`, overlap weight
   re-derived around the current 0.25.

**Gate.** R@1 improves; R@5, R@10, R@20, NDCG@10 and MRR do not regress; no
category regresses by more than 1 question. `k` stays a compile-time constant —
this is a value we tune once, not a per-request option.

**Falsified if** no `k` beats 60 on R@1 without costing R@5. That would mean the
RRF discount shape is not the lever, and the deficit is a content problem instead
(P2's job).

**Cost.** No schema change, no new dependency. One day.

## P2 — Offline feature analysis: can *any* lexical signal separate gold from the rest?

**Why.** The oracle says the answer is in the pool 500/500 times. It does not say
a reranker can find it. The only way to know whether a cheap reranker is possible
is to find out whether gold rows are lexically distinguishable from the rows
currently outranking them — and that is answerable offline, on 14 questions,
without building anything.

**What.** Take the 14 R@5-miss questions. For each, take the pool's top-20 and
label each row gold/non-gold. Compute candidate features and report, per feature,
whether any threshold or simple ordering separates them:

- term frequency of query terms in the row (the overlap stream uses **binary**
  presence — `src/recall.rs` `overlap_score` — so TF is untested and free)
- query-term IDF within the pool
- bigram / phrase overlap (note: this needs FTS positional data, which
  `detail=none` removed; record the index-size cost if it is worth pursuing)
- row length, and age
- per-stream provenance: found by BM25 only, overlap only, or both, and at what
  rank in each

Also do this for the R@1 question, where the prize is 81 questions rather than
14 — that is the real target and it may separate differently.

**Gate.** A written finding, not code: which features separate gold from the
rows above it, and how well. Nothing ships from this phase. If a feature separates
cleanly, P3 has its justification; if none does, that is the answer, and it means
the ordering deficit is semantic and only a model can address it.

**Falsified if** no lexical feature separates gold from its outrankers. That
result is worth as much as a positive one and should be recorded just as plainly.

**Cost.** One day, no production code touched.

## P3 — Build the cheapest reranker P2 justifies

**Why.** Only proceeds if P2 found signal. Ordered by cost, cheapest first:

1. **A scoring pass over the pool before the budget trim.** The audit found the
   100k budget leaves a caller 35 of 47 rows and never shows the deep candidates
   — so a reranker must sit *before* `trim_to_budget`, not after. This is an
   architectural constraint, not a preference.
2. Features in P2's order of demonstrated separation, as a linear combination
   over the existing `RankedHit` set. No model, no new dependency, no schema.

**Gate.** Same as P1, plus: the needle-curve suite (`eval/BENCH_RECALL_CURVE.md`)
must not regress below the shipped 96.9/84.4/90.6/90.6, since that suite is
already the known cost of the E1 trade and a second regression on top of it is
not acceptable without a new trade being stated.

**Falsified if** the reranker moves R@1 but costs R@5, which would mean it is
reordering well and retrieving nothing new — the wrong shape of win.

## P4 — Make the fusion weight size-aware, or accept the needle regression

**Why.** `detail=none` and the 0.25 weight were both chosen at one corpus size.
The 84.4% needle result at 10k (`eval/BENCH_RECALL_CURVE.md`) may be an artifact
of a single global constant applied across two very different regimes: 30–62
conversational sessions versus 100k adversarially-similar needles. That has never
been tested.

**What.** Sweep the overlap weight per corpus size on the curve harness and see
whether a single value can hold both suites. Only if no single value can, consider
making the weight a function of bank size — and if that is done, it must be a
measured curve, not a heuristic, and the README must carry the caveat that the
weight is no longer a constant.

**Gate.** The curve suite improves at 10k **and** LongMemEval R@5 holds at 97.2+.
Either outcome is fine; "the two cannot be reconciled" is also fine and must be
written down rather than papered over.

## P5 — Build a suite that has a coverage failure

**Why.** This is the binding constraint on all future evaluation. LongMemEval-S
retrieves its answer 500/500 times, so it can neither support nor refute a new
retrieval signal. Any proposal to add vectors, an LLM, or a wider pool is currently
**unfalsifiable with the harnesses in this repo.** That is a gap in the benchmark,
not a verdict on any technique.

**What.** A suite where the answer is genuinely not retrievable by lexical
matching — the honest version is a paraphrase-only split: questions whose wording
shares no vocabulary with the answer session, so BM25 and the overlap stream both
score zero and only semantic matching can find them. The needle harness is
adversarial in the other direction (too much shared vocabulary); this would be the
opposite axis.

**Gate.** The suite must have a non-trivial share of questions where the current
system fails to retrieve, with a published failure rate. A suite where the
existing system already scores 100% is worthless for this purpose — check that
first, before building anything.

## P6 — The cross-encoder question, now that it can be priced

**Why.** Only if P2 finds no lexical signal. Its cost was previously argued from
a guessed ceiling; the ceiling is now measured (+16.2pp R@1, +2.8pp R@5) and the
options can be compared against a real number.

- **(a) Do nothing.** Accept R@1 83.8%. The honest default if P2 and P3 both fail.
- **(b) Out-of-process optional reranker.** Keeps the 8.4 MiB binary and the
  single-file promise intact; costs a separate install and an IPC hop.
- **(c) Bundle a small ONNX cross-encoder** (~20–80 MB). Destroys the "8.4 MiB,
  three shared libraries, one file, nothing to install" claim, which is the
  product's main competitive argument against Hindsight's 0.8–1.0 GB.
- **(d) ONNX with first-run weight download.** Keeps the artifact small and
  breaks offline-first-use, the same objection that killed the vector arm.

**Gate.** Before building: a measured estimate of what fraction of the 16.2pp a
cross-encoder plausibly closes, from the literature on this exact suite, stated
with its source. If that estimate is small, (a) is the answer and the proposal
dies here rather than after a week of work.

---

## Explicitly not doing

Carried forward from earlier phases so nobody re-litigates them without new
evidence. Details in `docs/NEXT_ITERATION.md` and `eval/SWEEP_FUSION.md`.

- **Vector retrieval arm** — declined. R@20 is 99.6% and pool coverage is 500/500,
  so there is no coverage failure for it to fix on this suite; it would cost 3×
  artifact size, 4–7× idle RSS and 19–30× the write path. P5 is the prerequisite
  for ever reopening this.
- **Porter stemming (E2)** — measured flat on top of E1; temporal-reasoning
  regressed 2.26pp. Removed.
- **IDF-weighted overlap (E3)** — measured; the hypothesis that it would fix the
  recall curve was disproved. Removed.
- **Distinct-term-coverage stream (E4)** — proven algebraically identical to
  raising the overlap weight. Removed.
- **`cache_size` / `mmap` tuning (D4)** — measured, no gain, +15–17% RSS. Refused.
- **Hand-curated synonym table** — agentmemory ships one and credits it. Not
  copying a content list; it is a maintenance burden and a licensing question, and
  the cheap half of the same idea (stemming) was already measured and rejected.

## Order

**P−1 → P0 → P1 → P2 → P3 → P4 → P5 → P6.** P−1 is new and gates everything that
selects. P0 may run in parallel with P−1 because it selects nothing. P6 (the
cross-encoder) stays last and stays declined unless P2 and P3 both come back
empty, in which case the remaining gap is semantic and this architecture is at
its ceiling.

**P4 may be a deletion.** A 5% regression on synthetic distractors we author
ourselves is weak evidence, and if the honest engineering answer is "recall-curve
is adversarial to the overlap voter and we accept the trade", deleting P4 is a
legitimate completion.

**P0 → P1 → P2 → P3 → P4 → P5**, with P6 only if P2 and P3 both come back empty.

P0 and P1 are cheap and unblock measurement of the target. P2 is the decision
point: it either hands P3 a justified feature set or tells us the deficit is
semantic. Nothing after P2 should be built until P2 reports, because P3, P4 and P6
are three different answers to one question and building any of them early is
guessing.
