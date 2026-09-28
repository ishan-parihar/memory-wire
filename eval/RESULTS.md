# LongMemEval-S retrieval results (memory-wire)

Methodology: per-question fresh index, session-as-document, question-text query — same as agentmemory `longmemeval-bench.ts` (retrieval-only, no LLM judge).

Run 2026-09-28 from `--release`, seed 42, all 500 questions. Each question indexes
38–62 haystack sessions (mean 47.7), so the 200-row recall candidate pool never
binds on this suite — every session is scored on every query, and the suite cannot
detect a pool-bound or overlap-scorer regression on its own. The >200-row path is
covered by `tests/scale.rs` (5,000 memories) and `eval/SCALE_SWEEP.md`. The
retrieval metrics are deterministic: re-running this binary reproduced all 2,500
per-question values bit-for-bit. The `p50 ms` column is the exception: it is wall-clock, so it
moves with the machine's load (2-3x between the runs recorded here) and it is a record of *this*
run, not a property of the build. Do not pin it.

| Slice | R@5 | R@10 | R@20 | NDCG@10 | MRR | p50 ms | n |
|---|---|---|---|---|---|---|---|
| knowledge-update | 100.0% | 100.0% | 100.0% | 96.8% | 97.1% | 8 | 78 |
| multi-session | 97.0% | 98.5% | 99.2% | 86.0% | 91.4% | 8 | 133 |
| single-session-assistant | 100.0% | 100.0% | 100.0% | 92.8% | 90.4% | 11 | 56 |
| single-session-preference | 86.7% | 93.3% | 100.0% | 66.2% | 58.2% | 9 | 30 |
| single-session-user | 98.6% | 100.0% | 100.0% | 94.3% | 92.3% | 7 | 70 |
| temporal-reasoning | 96.2% | 97.7% | 99.2% | 85.3% | 87.1% | 8 | 133 |
| **overall** | **97.2%** | **98.6%** | **99.6%** | **88.2%** | **89.2%** | **8** | **500** |
