# LongMemEval-S retrieval results (memory-wire)

> **provisional:** every retrieval number in this artifact was measured at `overlap: 0.25` with `k: 60`, and that weight is **fitted, not earned** — it won 46 configurations scored on the test set (`AGENTS.md` §1), so it was chosen by looking at the same questions this table reports. Read every column as a measurement at a fitted value, not as a generalisation estimate, and do not quote the difference from an unfitted configuration as earned. See `docs/EVALUATION_HYGIENE.md` §2.1 for the audit, §3 for the three-set protocol that replaces it, and `eval/SWEEP_FUSION.md` for the grid that chose it.

Methodology: per-question fresh index, session-as-document, question-text query — same as agentmemory `longmemeval-bench.ts` (retrieval-only, no LLM judge).

Run 2026-09-28 from `--release`, seed 42, all 500 questions. Each question indexes
38–62 haystack sessions (mean 47.7), so the 200-row recall candidate pool never
binds on this suite — every session is scored on every query, and the suite cannot
detect a pool-bound or overlap-scorer regression on its own. The >200-row path is
covered by `tests/scale.rs` (5,000 memories) and `eval/SCALE_SWEEP.md`. The
retrieval metrics are deterministic: re-running this binary reproduced all 3,000
per-question values bit-for-bit. The `p50 ms` column is the exception: it is wall-clock, so it
moves with the machine's load (2-3x between the runs recorded here) and it is a record of *this*
run, not a property of the build. Do not pin it.

**R@1 is the column to read first**: it is the share of the 500 questions this build ranks the gold session *first* for, and it is the shipped ranking's own first-hit rate rather than a coverage claim. The distance from R@1 up to R@20 is reordering, not matching — R@20 is 99.6%, so the candidate pool already holds the gold row for 498 of the 500. Each row's percentage is over that row's own `n`.

| Slice | R@1 | R@5 | R@10 | R@20 | NDCG@10 | MRR | p50 ms | n |
|---|---|---|---|---|---|---|---|---|
| knowledge-update | 94.9% | 100.0% | 100.0% | 100.0% | 96.8% | 97.1% | 8 | 78 |
| multi-session | 87.2% | 97.0% | 98.5% | 99.2% | 86.0% | 91.4% | 9 | 133 |
| single-session-assistant | 85.7% | 100.0% | 100.0% | 100.0% | 92.8% | 90.4% | 12 | 56 |
| single-session-preference | 43.3% | 86.7% | 93.3% | 100.0% | 66.2% | 58.2% | 9 | 30 |
| single-session-user | 87.1% | 98.6% | 100.0% | 100.0% | 94.3% | 92.3% | 8 | 70 |
| temporal-reasoning | 80.5% | 96.2% | 97.7% | 99.2% | 85.3% | 87.1% | 9 | 133 |
| **overall** | **83.8%** | **97.2%** | **98.6%** | **99.6%** | **88.2%** | **89.2%** | **9** | **500** |
