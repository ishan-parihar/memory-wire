# OmniMemEval Experiment Report — memory-wire-mw_h10_medium

> Generated: 2026-10-16 06:42:24

## Experiment Configuration

| Parameter | Value |
|-----------|-------|
| Benchmark | HaluMem |
| Memory Framework | memory-wire |
| Version | mw_h10_medium |
| Run Time | 2026-10-16 06:42:21 IST |
| Git | `a5c8b4b` (main) (dirty) |
| WORKERS | 2 |
| LLM_WORKERS | 10 |
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
| Results Directory | `results/halumem/memory-wire-mw_h10_medium` |

**Reproduce:**

```bash
./scripts/run_halumem_eval.sh --replay results/halumem/memory-wire-mw_h10_medium
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

## Context Tokens

> Context Tokens are answer-stage prompt/input tokens. They are used as an approximate proxy for retrieved search-result length and should be compared only under the same benchmark, answer model, and prompt template.

| Metric | Value |
|--------|-------|
| Answer Model | poolside/laguna-s-2.1:free |
| Answer API Calls | 3,467 |
| Context Tokens (avg) | 1966.7 |

## Evaluation Results

### Overall Scores

| Metric | Score |
|--------|-------|
| QA Accuracy (LLM-as-Judge) | 0.6129 ± 0.0000 |
| F1 | 0.1439 |
| ROUGE-1 | 0.0916 |
| ROUGE-2 | 0.0350 |
| ROUGE-L | 0.0796 |
| BLEU-1 | 0.0503 |
| BLEU-4 | 0.0099 |
| METEOR | 0.1918 |
| BERTScore F1 | - |
| Semantic Similarity | - |

### By Question Type (QA Accuracy)

| Question Type | QA Accuracy | Questions |
|---------------|-------------|-----------|
| Basic Fact Recall | 0.3804 | 744 |
| Memory Boundary | 0.9372 | 828 |
| Memory Conflict | 0.7018 | 768 |
| Generalization & Application | 0.5375 | 746 |
| Multi-hop Inference | 0.4394 | 198 |
| Dynamic Update | 0.2056 | 180 |

### By Difficulty

| Difficulty | QA Accuracy | Questions |
|------------|-------------|-----------|
| easy | 0.6248 | 1967 |
| hard | 0.6429 | 98 |
| medium | 0.5940 | 1399 |

### Pipeline Status

| Stage | Failed Users | Failed Questions | Skipped Records | Status Counts |
|-------|--------------|------------------|-----------------|---------------|
| search | 0 | 0 | 0 | success=3467 |
| answer | 0 | 0 | 0 | success=3467 |
| eval | 0 | 3 | 0 | success=3464 |

### Latency Metrics

| Metric | Avg (ms) | P50 (ms) | P95 (ms) |
|--------|----------|----------|----------|
| Add Latency | 95.8 | 91.6 | 144.1 |
| Search Latency | 43.0 | 41.8 | 60.9 |
