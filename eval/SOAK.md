# Concurrency soak (memory-wire)

Run 2026-09-28 from `--profile=release` on AMD Ryzen 9 5900X 12-Core Processor, 24 logical CPUs.

Workload: 16 clients x 4 banks x 10s against ONE `SqliteStore` (one writer `Mutex<Connection>` plus a 4-connection WAL read pool, writer-first with spill-on-contention), a six-op schedule weighted one storm in ten, on a database this run created at /var/tmp/ishanp-agentic/mw-soak-143212.db. Build profile, machine and date are in the provenance line above; the profile alone moves these numbers 2-5x.

**Aggregate throughput: 2132 ops/s** over 21344 attempts in 10.0s (median 626 us, p95 48194 us, p99 95272 us across all ops).

| op | count | ops/s | p50 us | p95 us | p99 us |
|---|---|---|---|---|---|
| retain | 4269 | 427 | 2503 | 74695 | 127456 |
| recall | 4271 | 427 | 849 | 3247 | 5066 |
| stats | 4271 | 427 | 663 | 3145 | 5266 |
| page | 4269 | 427 | 57 | 223 | 1870 |
| replace | 2139 | 214 | 13521 | 89173 | 151365 |
| append | 2125 | 212 | 81 | 55122 | 101642 |

Statuses: ok 19219 · 409 2125 · 5xx 0 · other 0. SQLITE_BUSY/LOCKED 0. Panicked clients 0. Post-storm probe ok.

Row exactness: per bank [1057, 1080, 1056, 1084], expected sum 4277, db sum 4277, per-bank exact true. Storm documents: replace [1, 1, 1, 1], append [1, 1, 1, 1].

RSS: 7.6 MB -> 27.2 MB (delta +19.6 MB) · db on disk 1.7 MB · ceiling 23.8 MB (page cache + 1 MB/client).

Append pre-check plan: SCAN CONSTANT ROW | SCALAR SUBQUERY 1 | SEARCH memories USING COVERING INDEX idx_memories_bank_doc (bank_id=? AND document_id=?)

Indexed: true. Conflict probe: conflict p50 31 us on a 2-row bank -> 27 us on a full one (0.9x, limit 25x).

Baseline p99: none given; this run establishes it.

Verdict: **PASS**

## What this does not measure

- Throughput scaling. This run fixes the client count; `examples/bench_concurrency.rs` sweeps it and is where a serialization signature is actually visible.
- Write cost. The workload keeps every bank growing, so a run's throughput depends on its length; `examples/bench_write.rs` prices a single retain.
- Recall quality, recall versus bank size, startup cost, and binary or storage footprint. Those are `bench_recall_curve`, `bench_coldstart` and `bench_footprint`.
- HTTP and MCP transport cost: this is the library call in-process, so axum, tokio and a socket are absent by design.

## Limitations

- A time-boxed run measures throughput as a function of the scheduler as well as of the store. Quoting one run's ops/s as a property of the store overstates it; re-run and quote the spread, as `eval/CODING_LIFE.md` does for its p50.
- The p99 baseline is only comparable across runs on the same profile *and* the same kind of storage: a dev build against a release build differ about 7x, and a scratch db on disk against one on tmpfs about 5x, both measured here.
- One run bounds an RSS leak, it cannot prove the absence of one: rerun at a different `--seconds` or `--clients` and check the number does not track the work.
- The append pre-check flatness check sizes a per-row scan or a retry loop. It is not sensitive to a constant-factor regression, and its 25x budget deliberately allows for timing jitter between two idle single-threaded bursts.
