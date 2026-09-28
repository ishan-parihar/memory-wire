# Evaluation hygiene: the test set is not a training set

Adopted 2026-09-28. Enforced by `AGENTS.md` §1. This document is the reasoning
and the audit that motivated it.

## 1. The rule

> A parameter, weight, threshold, feature set, or configuration that was chosen by
> looking at the evaluation set is **fitted**, not earned. Report it as such.

The failure mode is not lying. It is a number that looks like a generalisation
estimate and is not one. The tell is always the same in the write-up: a delta
reported as an improvement, with no mention of how many things were tried before
this one.

A hard corollary, which is where most of the value is:

> **Fixing a bug that an evaluation exposed is fine. Tuning until the evaluation
> stops complaining is not.** The difference is whether you chose the change by
> mechanism or by score.

### What this does *not* forbid

- Measuring on the test set and publishing the number.
- Fixing a correctness bug the test set surfaced.
- Deciding a change on a mechanistic argument and then *observing* what the test
  set says — including "it didn't help", which is a result.
- **Proving something algebraically and not consulting data at all.** A proof is
  not a fit. E4 was rejected because RRF is linear in the weights, so
  `coverage: w ≡ overlap: 0.25 + w` for any query with no repeated token. That
  rejection used no data and is not a subject of this rule.

## 2. Audit of this repository's own history

Done honestly, including where the answer is uncomfortable.

### 2.1 Confirmed violation — `overlap: 0.25` is fitted

`eval/SWEEP_FUSION.md` records 46 configurations, all scored on LongMemEval-S's
500 questions. `0.25` was kept because it won on aggregate R@5. One parameter,
swept on test set, selected on the test set. **The shipped value was
provisional.**

> **Resolved 2026-09-28 — the marker is discharged.** The independent
> confirmation §3.2 asked for was built (`examples/locomo.rs` →
> `eval/LOCOMO.md`) and it confirms the direction: over 1,531 LoCoMo queries
> that never chose the value, `0.25` beats equal weight by **+1.4pp R@5 and
> +2.9pp NDCG@10**. Per the rule in `AGENTS.md` §1 — a fitted value is marked
> provisional *until an independent set confirms it* — the weight is no longer
> provisional. `docs/CONSISTENCY.md` §14.1 records the decision and its limits.

**What promotion does and does not buy.** It retires the *marker*; it does not
convert the +4.2pp into an earned number. The magnitude is now bounded from both
ends rather than left unknown: the unfitted equal-weight configuration measured
R@5 93.0 / NDCG@10 83.5, the fitted one measures 97.2 / 88.2, and the
*independently measured* value of the delta is **+1.4pp R@5 / +2.9pp NDCG@10**.
So the true generalisation gain is in **[0, +4.2pp]** with a measured out-of-sample
point of **+1.4pp**, and the best out-of-sample estimate of the shipped
configuration is **≈94.4% R@5**, not 97.2%. Quote the marker as discharged; do
not quote +4.2pp without that sentence attached.

### 2.2 The consequence for our parity claim

This is the part that has to be said out loud. We have been reporting
"97.2% R@5, beating agentmemory's 95.2%". But:

| | R@5 | status |
|---|---|---|
| agentmemory BM25+Vector | 95.2% | committed, committed harness — tuning undisclosed |
| agentmemory BM25-only | 86.2% | committed, committed harness — tuning undisclosed |
| **memory-wire, unfitted** | **93.0%** | clean |
| **memory-wire, fitted `overlap: 0.25`** | **97.2%** | **not a generalisation estimate** |

The **only clean LongMemEval number we have is 93.0%**, which is +6.8pp over
agentmemory's BM25-only arm and −2.2pp behind their hybrid. The "+2.0pp lead"
quoted in the README is measured against a fitted configuration. That does not
make it false, and the independent confirmation this section asked for has since
arrived (§2.1) — but it confirms the *direction* and prices the *delta* at
+1.4pp, not +4.2pp. The honest out-of-sample figure is **≈94.4% R@5: behind
agentmemory's 95.2% hybrid by ≈0.8pp, not ahead of it by 2.0pp.** That is the
number that belongs in a headline, and the README now carries it.

Note also that both parents develop against a public dataset. Whether agentmemory
tuned on these exact 500 questions is not recorded in their repo. A head-to-head
on a public benchmark is only clean if neither party did; we now know we did.

### 2.3 A trade-off accepted on evaluation evidence

E1 improved LongMemEval R@5 and regressed the recall curve (100% → 84.4% at 10k)
and coding-life R@5 (100% → 96.7%). The regression was accepted because the gain
was larger. That is a legitimate engineering call, but the *decision rule* used a
test-set number on one side, so it inherits the same problem. The honest framing
is "two benchmarks disagree and we picked the one the project optimises for" —
which is defensible, and should be stated rather than buried.

### 2.4 Reasoning from the instrument that turned out incomplete

We declined a vector arm partly on the argument "R@20 is 99.6%, so the gap is
reordering, which is a cross-encoder's job." That inference is drawn from the
evaluation set. It was not wrong, but it was framed around R@5 and understated
the opportunity: the same measurement later showed R@1 at 83.8% with a 16.2pp
oracle gap — five times the R@5 gap. **The argument generalised worse than it
looked**, which is exactly the risk of reasoning from one instrument.

### 2.5 Correctly handled — worth keeping

- The LoCoMo attempt produced a real measurement (1540 queries, 272 docs) and
  it was **not** published as a comparison, because the harness could not be
  validated end-to-end without LLM keys. Labelled "unvalidated context". That is
  the right call and cost us a headline.
- E4's rejection used algebra, not data.
- The idle-box re-measurement replaced loaded-box numbers rather than keeping the
  flattering ones.

### 2.6 Never validated — `k = 60`

The RRF constant came from Hindsight's published value, not from our data. So it
is **borrowed, not fitted** — which means it is also unvalidated here. A value
inherited from another project is a hypothesis, not a decision.

## 3. The protocol

### 3.1 Three sets

| set | what | role |
|---|---|---|
| **dev** | a labelled corpus that is *not* LongMemEval | **all selection happens here** |
| **test** | LongMemEval-S, 500 questions | measurement and release acceptance only |
| **synthetic** | `bench_recall_curve`, `tests/scale` | scaling and regression assertions; we author the distractors, so weak evidence about ranking quality |

### 3.2 The dev set already exists

`eval/data/locomo/` (cloned from `vectorize-io/agent-memory-benchmark`, the
canonical LoCoMo distribution) is **272 documents and 1,540 queries**, with
`gold_ids` naming the gold document and `user_id` giving the isolation unit. That
is the same shape as LongMemEval and it is retrieval-only — `gold_answers` is
answer prose we do not need. It is from an entirely different source, so it is
genuinely independent.

Building a harness for it was the first piece of work, and it retroactively
settles §2.1: **does `overlap: 0.25` beat equal weight on independent data?** **It
does — it has been built and run** (`examples/locomo.rs` → `eval/LOCOMO.md`:
1,531 queries, +1.4pp R@5, +2.9pp NDCG@10; `docs/CONSISTENCY.md` §14.1), so the
effect replicated and `0.25` is no longer a provisional value.

**It does not re-open the weight for tuning.** The LoCoMo table is a measurement,
not a shortlist: §14.2 records that `overlap: 0.00` ties it on R@1 and NDCG@10
while churning 69 queries up and 41 down — a tie, recorded as a null result and
deliberately not acted on. Picking a new value off either table is a selection
that must be counted under §3.3 and must never be checked against LongMemEval.

### 3.3 Counting rule

Every selection made on the test set is logged in `docs/CONSISTENCY.md` with the
date, the parameter, the search space size, and the delta. The count is the
project's overfitting budget and it is meant to be visible and finite.

### 3.4 Learned components

No component with fitted parameters ships until all of the following hold:

1. it is fitted on the dev set, never on the test set
2. the dev/test protocol above is followed and the fold structure is recorded
3. the gate is pre-registered in the plan document **before** fitting
4. the test set is consulted once at the end, and the result is reported
   whatever it is
5. a null result is written down like any other

## 4. What is allowed to ship without any of this

Deterministic changes with a mechanistic justification and no free parameters:
a correctness fix, a missing index, restoring a signal the code already computes
but discards. These can be decided by argument. What they cannot do is acquire
five magic numbers through a search against the benchmark.
