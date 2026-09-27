# coding-life eval (memory-wire)

Dataset: vendored `eval/data/{sessions,queries}.json` (15 sessions, 15 labeled queries, from agentmemory `eval/data/coding-agent-life-v1`). Scoring mirrors their `eval/runner/score.ts`. k=5.

Run 2026-09-27 from `--release` (`cargo run --release --example coding_life`). The
corpus is 15 sessions, so the 200-row recall candidate pool cannot bind here and
every session is scored on every query. P@5 / R@5 / hit rate are deterministic and
are what this suite gates on; p50 is a 15-sample median and moves ±40% run to run
(measured 364–616 µs across five release runs of this binary), so quote it as a
range, never as a regression signal.

| Adapter | P@5 | R@5 | Hit rate | p50 latency | n |
|---|---|---|---|---|---|
| memory-wire | 24.0% | 100.0% | 100.0% | 266 µs | 15 |
| grep | 22.7% | 96.7% | 100.0% | 4 µs | 15 |
