# coding-life eval (memory-wire)

Dataset: vendored `eval/data/{sessions,queries}.json` (15 sessions, 15 labeled queries, from agentmemory `eval/data/coding-agent-life-v1`). Scoring mirrors their `eval/runner/score.ts`. k=5.

Run 2026-09-28 from `--profile=release`.

The corpus is 15 sessions, so the 200-row recall candidate pool cannot bind here and every session is scored on every query. P@5 / R@5 / hit rate are deterministic and are what this suite gates on. **The p50 latency column is not.** It is a 5-sample median over one run of one machine: it has been measured across release runs of this binary between roughly 260 and 620 us, so it moves several-fold with the box's load and must be quoted as a range, never as a regression signal. Re-run it; do not pin it.

| Adapter | P@5 | R@5 | Hit rate | p50 latency | n |
|---|---|---|---|---|---|
| memory-wire | 22.7% | 96.7% | 100.0% | 441 µs | 15 |
| grep | 22.7% | 96.7% | 100.0% | 6 µs | 15 |

## The same suite, swept across fusion configurations

`recall_with_weights` is the identical code path `recall` takes with the weights as an argument, and all rows run on one index, so a row and the `memory-wire` row above differ only in the fusion arithmetic. Queries are outer and configurations inner, so a load spike lands on every row in the same pass. **P@5/R@5/hit rate are deterministic; the latency column is not** — same caveat as above, and at 15 samples it is a median of 15.

| configuration | BM25 | overlap | coverage | idf | P@5 | R@5 | Hit rate | p50 latency | n |
|---|---|---|---|---|---|---|---|---|---|
| shipped | 1.00 | 0.25 | 0.00 | no | 22.7% | 96.7% | 100.0% | 408 µs | 15 |
| raw overlap=0.50 | 1.00 | 0.50 | 0.00 | no | 24.0% | 100.0% | 100.0% | 446 µs | 15 |
| raw overlap=0.75 | 1.00 | 0.75 | 0.00 | no | 24.0% | 100.0% | 100.0% | 500 µs | 15 |
| raw overlap=1.00 | 1.00 | 1.00 | 0.00 | no | 24.0% | 100.0% | 100.0% | 463 µs | 15 |
| idf overlap=0.25 | 1.00 | 0.25 | 0.00 | yes | 22.7% | 96.7% | 100.0% | 422 µs | 15 |
| idf overlap=0.50 | 1.00 | 0.50 | 0.00 | yes | 24.0% | 100.0% | 100.0% | 405 µs | 15 |
| idf overlap=0.75 | 1.00 | 0.75 | 0.00 | yes | 24.0% | 100.0% | 100.0% | 415 µs | 15 |
| idf overlap=1.00 | 1.00 | 1.00 | 0.00 | yes | 24.0% | 100.0% | 100.0% | 415 µs | 15 |
| idf overlap=1.25 | 1.00 | 1.25 | 0.00 | yes | 24.0% | 100.0% | 100.0% | 404 µs | 15 |
| shipped + coverage=0.10 | 1.00 | 0.25 | 0.10 | no | 22.7% | 96.7% | 100.0% | 410 µs | 15 |
| shipped + coverage=0.25 | 1.00 | 0.25 | 0.25 | no | 24.0% | 100.0% | 100.0% | 430 µs | 15 |
| shipped + coverage=1.00 | 1.00 | 0.25 | 1.00 | no | 24.0% | 100.0% | 100.0% | 398 µs | 15 |

### Missed query ids, per configuration

- **shipped**: none
- **raw overlap=0.50**: none
- **raw overlap=0.75**: none
- **raw overlap=1.00**: none
- **idf overlap=0.25**: none
- **idf overlap=0.50**: none
- **idf overlap=0.75**: none
- **idf overlap=1.00**: none
- **idf overlap=1.25**: none
- **shipped + coverage=0.10**: none
- **shipped + coverage=0.25**: none
- **shipped + coverage=1.00**: none
