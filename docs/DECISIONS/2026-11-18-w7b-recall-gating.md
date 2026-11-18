# W7b — Should `recall_section` gate on a raw score?

**Date:** 2026-11-18
**Status:** Decision (deferred)
**Plan item:** W7b

## Question

W7a added `memory_supersede` (lifecycle retirement). W7b was: "should
`recall_section` / `memory_recall` apply a raw relevance-score threshold
to decide what enters the result set?"

## Decision: No. Leave the result set ungated and say so.

### Why not

1. **A raw score is not a probability.** The composite score is a weighted
   sum of BM25, overlap, and recency components. It is not calibrated
   against any probability model. A threshold derived from "scores below X
   are noise" is a fitted constant, and fitting it on LongMemEval would
   burn the evaluation set (AGENTS §6).

2. **The score distribution is data-dependent.** A query against a dense
   bank produces a different score distribution than the same query
   against a sparse one. A fixed threshold is wrong for one of them.

3. **Score-gating is a runtime policy, not a store concern.** memory-wire
   is the deterministic substrate; the runtime is the reasoning layer.
   The runtime should decide what to inject into a prompt. The store
   serves a ranked result set and lets the caller filter.

4. **CLOSED.md forbids it.** A raw score cut on retrieval was tried and
   killed (`docs/handoff/CLOSED.md`). The measurement showed that the
   memories the cut removed were not the wrong ones — they were the
   right ones at positions the cut did not reach. Re-proposing a raw
   score gate without new evidence violates §8.

5. **The honest signal is the ranking, not the score.** A result that
   ranks at position 1 is the best the system can do; a result at
   position 20 is the worst. The position is the signal the caller can
   act on. The score is an internal detail.

### What memory-wire does instead

- **Ranks and returns a budget-trimmed result set.** The caller sees
  the ranking and decides how many to consume. This is the current
  behavior; it stays.
- **`memory_reflect` returns the top hit with its id prefixed.** The
  citation prefix lets the caller verify the answer against the source.
- **`memory_supersede` retires a memory explicitly.** The caller (or
  the runtime) decides a memory is stale and marks it superseded. The
  store does not auto-retire.

### What the runtime should do

The runtime is the policy layer. A reasonable policy is:

- **If the top hit's score is above a runtime-calibrated threshold,**
  include it. The threshold is the runtime's, not the store's.
- **If the result set is empty, fall back to no injection.** An empty
  result is a fact about the store, not a failure.
- **If the result set has one item, inject it.** A single hit is the
  strongest signal the store can give.

These are runtime decisions, not store decisions.

## Effect on the plan

W7b is closed with a decision rather than code. The `recall_section`
function in `src/grounding.rs` (W5) does not gate on score; it orders by
composite and trims by budget. That is the correct behavior.

## What would change this decision

- **A calibrated score.** If the composite score were calibrated against
  a probability model (e.g., logistic regression on a held-out set), a
  threshold would be defensible. That is a separate work item and would
  require a labelled set that is not LongMemEval.
- **A runtime that asks for it.** If a downstream consumer of the store
  API requests score-gating as a first-class feature, the store can add
  it as an opt-in parameter with a clear "this is a raw score cut" label.
