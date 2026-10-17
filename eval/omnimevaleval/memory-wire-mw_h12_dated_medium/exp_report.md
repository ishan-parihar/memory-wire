# OmniMemEval Experiment Report — memory-wire-mw_h12_dated_medium

> Generated: 2026-10-17 05:43:14

## Experiment Configuration

| Parameter | Value |
|-----------|-------|
| Benchmark | HaluMem |
| Memory Framework | memory-wire |
| Version | mw_h12_dated_medium |
| Run Time | 2026-10-17 05:33:49 IST |
| Git | `a5c8b4b` (main) (dirty) |
| WORKERS | 2 |
| LLM_WORKERS | 4 |
| TOPK | 20 |
| VARIANT | medium |
| NUM_RUNS | 1 |
| SAVE_MODEL_INPUT | 0 |
| CLEAR | 0 |
| WAIT_AFTER_INGEST | 0 |
| ALLOW_EMPTY_SEARCH | 1 |
| SKIP_FAILED_SEARCH | 0 |
| SKIP_FAILED_ANSWER | 0 |
| SKIP_FAILED_JUDGE | 0 |
| Results Directory | `results/halumem/memory-wire-mw_h12_dated_medium` |

**Reproduce:**

```bash
./scripts/run_halumem_eval.sh --replay results/halumem/memory-wire-mw_h12_dated_medium
```

### Environment Variables (snapshot_eval.env)

| Variable | Value |
|----------|-------|
| `ANSWER_MODEL` | `small-stack` |
| `EVAL_MODEL` | `small-stack` |
| `ANSWER_TIMEOUT` | `300` |
| `EVAL_TIMEOUT` | `300` |
| `LLM_MAX_RETRIES` | `3` |
| `MEMORY_WIRE_URL` | `http://127.0.0.1:8899` |
| `MEMORY_WIRE_TIMEOUT` | `60` |
| `MEMORY_WIRE_SERVE_DATES` | `1` |

## Context Tokens

> Context Tokens are answer-stage prompt/input tokens. They are used as an approximate proxy for retrieved search-result length and should be compared only under the same benchmark, answer model, and prompt template.

| Metric | Value |
|--------|-------|
| Answer Model | poolside/laguna-s-2.1:free |
| Answer API Calls | 123 |
| Context Tokens (avg) | 2202.1 |

## Evaluation Results

### Overall Scores

| Metric | Score |
|--------|-------|
| QA Accuracy (LLM-as-Judge) | 0.7433 ± 0.0000 |
| F1 | 0.1469 |
| ROUGE-1 | 0.0964 |
| ROUGE-2 | 0.0411 |
| ROUGE-L | 0.0846 |
| BLEU-1 | 0.0510 |
| BLEU-4 | 0.0112 |
| METEOR | 0.2033 |
| BERTScore F1 | - |
| Semantic Similarity | - |

### By Question Type (QA Accuracy)

| Question Type | QA Accuracy | Questions |
|---------------|-------------|-----------|
| Basic Fact Recall | 0.6086 | 746 |
| Memory Boundary | 0.9118 | 828 |
| Memory Conflict | 0.8986 | 769 |
| Generalization & Application | 0.6568 | 746 |
| Multi-hop Inference | 0.5758 | 198 |
| Dynamic Update | 0.4056 | 180 |

### By Difficulty

| Difficulty | QA Accuracy | Questions |
|------------|-------------|-----------|
| easy | 0.7665 | 1970 |
| hard | 0.6531 | 98 |
| medium | 0.7169 | 1399 |

### Pipeline Status

| Stage | Failed Users | Failed Questions | Skipped Records | Status Counts |
|-------|--------------|------------------|-----------------|---------------|
| search | 0 | 0 | 0 | success=3467 |
| answer | 0 | 0 | 0 | success=3467 |
| eval | 0 | 0 | 0 | success=3467 |

### Latency Metrics

| Metric | Avg (ms) | P50 (ms) | P95 (ms) |
|--------|----------|----------|----------|
| Add Latency | 536.7 | 247.0 | 1727.5 |
| Search Latency | 143.6 | 101.4 | 380.6 |
