# Relevance dev set — labelled cases over the real banks

**Not LongMemEval.** This directory is the labelled development set the
retrieval-efficacy audit (`docs/RETRIEVAL_EFFICACY_AUDIT.md` GAP-9) requires
before any relevance-gating decision can be made honestly. LongMemEval-S
remains the test partition, consulted for release acceptance only
(`AGENTS.md` §6). Nothing in this directory may tune against LongMemEval,
and nothing measured here is a claim about it.

## What is in here

- `cases.jsonl` — the seed cases: the audit's 10 graded recall cases,
  verbatim queries, with the labels a gating decision needs: `gold`
  (content prefixes that must rank) and `junk` (content prefixes that
  must not be auto-injected). Labels are hand-reviewed against the bank
  as of 2026-10-08.
- `run_battery.py` — runs every case through the same REST route the
  surfaces use, in both shapes that matter:
  - `auto` — the auto-injection shape (`exclude_tags: ["transcript",
    "marker"]`, budget 1200): what a turn in flight would receive.
  - `search` — the explicit-search shape (no exclusions, budget 2000):
  what a model calling `memory_recall` sees.

  It reports per case: gold@5, junk@5, and the injected-character cost,
  plus totals.

## How to use it

```bash
python3 eval/relevance/run_battery.py --bank omp
python3 eval/relevance/run_battery.py --bank omp --case B   # one case
```

The battery re-runs after every retrieval-affecting change and the numbers
go in `docs/CONSISTENCY.md`. The 2026-10-08 baseline (pre-fix) is recorded
in the audit doc §4; the first post-fix run is recorded in
`docs/CONSISTENCY.md` §32.

**Artifacts are committed, not paraphrased.** Every run whose numbers are
quoted in prose ships its full output here as a `BATTERY-<date>[-<tag>].txt`
file *in the same commit* as the prose (AGENTS §7: the artifact wins). The
v0.6.3 baseline has no such file — its output was not saved at measurement
time, so §32's baseline column is prose-only by miss; the first committed
artifact is `BATTERY-2026-10-08-v0.7.0.txt`.

## The discipline

- A change is **decided** on this set, then measured once on LongMemEval
  for the report — never the other way round.
- Labels are ground truth by human review. A case whose label proves wrong
  gets its label fixed in the same commit that records the measurement.
- Growing the set: harvest new cases from real `turn-*` rows (the question
  is the query, the row itself is gold, unrelated turns are distractors).
  10 cases is a seed; ~100 is the size at which a gating decision becomes
  evidence rather than anecdote.
