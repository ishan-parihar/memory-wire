# AGENTS.md — rules for agents working in this repository

Read this before changing anything. It is short on purpose.

## 1. The evaluation set is not a training set

This is the rule that matters most here, and it is the one most easily broken by
accident.

`eval/RESULTS.md` holds LongMemEval-S results over 500 questions. Those 500
questions are an **instrument for measuring**, never an objective to optimise
against.

**Never do any of the following and then describe the result as an improvement:**

- sweep a parameter, weight, threshold, or feature configuration against the
  LongMemEval numbers and keep the winner
- add, remove, or reorder a retrieval signal because it moved R@5 / R@10 /
  NDCG@10 on LongMemEval
- re-run a change, observe a delta, and iterate until the delta is positive
- quote a LongMemEval number as evidence that a component is "better" when the
  component was chosen by looking at that number

**A change selected by looking at the evaluation set is fitted, not earned.**
When that has happened — and it has — say so in the commit message and in
`docs/CONSISTENCY.md`, mark the value provisional, and treat re-validation on
independent data as outstanding work.

### What is allowed

| Allowed | Not allowed |
|---|---|
| Measuring on LongMemEval and reporting the number | Choosing a value by comparing LongMemEval numbers |
| Fixing a bug that LongMemEval exposed | Tuning until a metric stops complaining |
| Deciding a change on a mechanistic argument, then *observing* the metric | Deciding a change on the metric |
| Selecting against a **development** set that is not LongMemEval | Using the test partition as a dev set |
| Reporting that a change did not help | Reverting a change because a benchmark "regressed" while keeping an equal-or-worse alternative that scored better |

### The three-set discipline

- **dev** — a labelled set that is *not* LongMemEval. All selection happens here.
  `eval/data/locomo/` is the intended dev corpus (independent source, 1540
  queries, gold document ids). See `docs/EVALUATION_HYGIENE.md`.
- **test** — LongMemEval-S. Consulted for **measurement and release acceptance
  only**. Never for selection.
- **synthetic** — `examples/bench_recall_curve.rs` and `tests/scale.rs` generate
  their own corpora with fixed seeds. Useful for scaling and regression
  assertions, but we author the distractors, so it is weak evidence about
  ranking quality. Do not tune against it either.

### Count the consultations

Every change that is *selected* on the test set burns it. `docs/CONSISTENCY.md`
records the count. If a component's parameters were fitted on LongMemEval, it
carries a `provisional:` marker in the code and in the docs until an independent
set confirms it.

## 2. Artifacts are the record of account

- A number in prose must exist in a committed artifact under `eval/`, or the
  prose must say where it came from and that it is not in a committed artifact.
- When a doc and an artifact disagree, **the artifact wins** — except for a
  historical record that deliberately describes a specific dated build, which
  stays and keeps its date.
- Never hand-edit a generated artifact under `eval/`. Fix the generator.
- `docs/CONSISTENCY.md` is the append-only record of what was measured, on what,
  at what load, and what was rejected. Add to it. Do not rewrite it.

## 3. Measurement hygiene

- **Latency and RSS are only meaningful on a quiet box.** Check `loadavg` before
  and after, and record it. Numbers taken at load > 20 on 24 cores are ranges at
  best and are labelled as such.
- Sweeps that are infeasible to repeat (≥ 30 min) run once and are reported
  without a CI, not three times.
- Do not report a percentage without the denominator and the n.

## 4. Rejected work stays rejected

Levers already measured and removed, with the measurement that killed each, are
listed in `docs/RERANKING_PLAN.md` §3 and `docs/PERFORMANCE_PLAN.md`. Do not
re-propose one without new evidence. If you find yourself re-litigating a
rejected lever, the answer is in those documents, not in your intuition.

## 5. Code rules

- `cargo test --locked`, `cargo clippy --all-targets --all-features --locked --
  -D warnings`, and `cargo doc --no-deps --all-features --locked` (0 warnings)
  are the gate. All three must pass.
- `deny(missing_docs)` is on. Public items need `///` docs.
- No `unwrap()` in non-test code. `expect()` needs a reason that names the
  invariant.
- No new dependencies without saying what the standard library or an existing
  dependency cannot do.
- Blocking SQLite work must not run on a tokio worker; see `src/store.rs`.
- Do not tag, publish, or cut a release unless asked. Do not force CI to run —
  `ci.yml` deliberately ignores branch pushes to conserve Actions quota.
- Delegate only with the `space-bunny` model.

## 6. Honesty rules for reports

- Report what a measurement shows, not what you hoped it would show.
- If an agent before you made a claim wrong, correct it by name.
- If you cannot determine which of two conflicting values is right, leave both
  and say so. Do not invent a reconciliation.
- A null result is a result. Record it.
