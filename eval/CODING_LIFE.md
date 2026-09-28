# coding-life eval (memory-wire)

Dataset: vendored `eval/data/{sessions,queries}.json` (15 sessions, 15 labeled queries, from agentmemory `eval/data/coding-agent-life-v1`). Scoring mirrors their `eval/runner/score.ts`. k=5.

Run 2026-09-28 from `--profile=release`.

The corpus is 15 sessions, so the 200-row recall candidate pool cannot bind here and every session is scored on every query. P@5 / R@5 / hit rate are deterministic and are what this suite gates on. **The p50 latency column is not.** It is a 5-sample median over one run of one machine: it has been measured across release runs of this binary between roughly 260 and 620 us, so it moves several-fold with the box's load and must be quoted as a range, never as a regression signal. Re-run it; do not pin it.

| Adapter | P@5 | R@5 | Hit rate | p50 latency | n |
|---|---|---|---|---|---|
| memory-wire | 22.7% | 96.7% | 100.0% | 422 µs | 15 |
| grep | 22.7% | 96.7% | 100.0% | 5 µs | 15 |
