# LongMemEval-S retrieval results (memory-wire)

Methodology: per-question fresh index, session-as-document, question-text query — same as agentmemory `longmemeval-bench.ts` (retrieval-only, no LLM judge).

Run 2026-09-27 from `--release`, seed 42, all 500 questions. Each question indexes
38–62 haystack sessions (mean 47.7), so the 200-row recall candidate pool never
binds on this suite — every session is scored on every query, and the suite cannot
detect a pool-bound or overlap-scorer regression on its own. The >200-row path is
covered by `tests/scale.rs` (5,000 memories) and `eval/SCALE_SWEEP.md`. The
retrieval metrics are deterministic: re-running this binary reproduced all 2,500
per-question values bit-for-bit.

| Slice | R@5 | R@10 | R@20 | NDCG@10 | MRR | p50 ms | n |
|---|---|---|---|---|---|---|---|
| knowledge-update | 98.7% | 100.0% | 100.0% | 94.9% | 95.7% | 6 | 78 |
| multi-session | 96.2% | 98.5% | 99.2% | 83.3% | 89.2% | 6 | 133 |
| single-session-assistant | 87.5% | 96.4% | 100.0% | 79.7% | 74.6% | 7 | 56 |
| single-session-preference | 56.7% | 86.7% | 100.0% | 51.5% | 42.0% | 6 | 30 |
| single-session-user | 98.6% | 98.6% | 100.0% | 89.5% | 86.6% | 5 | 70 |
| temporal-reasoning | 94.0% | 97.0% | 99.2% | 82.5% | 83.6% | 6 | 133 |
| **overall** | **93.0%** | **97.4%** | **99.6%** | **83.5%** | **83.9%** | **6** | **500** |
