# Concurrency vs throughput (memory-wire)

Run 2026-10-13 from `--profile=release` on AMD Ryzen 9 5900X 12-Core Processor, 24 logical CPUs.

Workload: `std::thread` clients against ONE `SqliteStore` (one writer `Mutex<Connection>` plus a 4-connection WAL read pool, writer-first with spill-on-contention), `recall` op, 2000 seeded memories, 200 ops per client, 3 repeats per client count. Aggregate ops/s is `clients x 200 / wall`; latencies are microseconds. `RSS MB` is this process at the end of the step.

Percentiles pool every op of every client in a step, so they describe the contention that step created rather than one client's experience; across repeats the median of each percentile is shown. `ops/s` carries its min-max range across repeats — thread scheduling is not this harness's to control, so single-digit-percent movement between runs is noise, not a result. `efficiency` is `vs 1 client / clients`: 1.00 is perfect scaling, and a store that can overlap only part of an operation plateaus below it.

| Clients | ops/s (min-max) | p50 us | p95 us | p99 us | RSS MB | vs 1 client | Eff |
|---|---|---|---|---|---|---|---|
| 1 | 583.7 (451.8-674.9) | 1705 | 3103 | 3371 | 12.8 | 1.00x | 1.00 |
| 2 | 758.5 (702.1-795.1) | 3069 | 3480 | 3607 | 12.8 | 1.30x | 0.65 |
| 4 | 1210.8 (1091.0-1357.6) | 3741 | 5787 | 7071 | 12.8 | 2.07x | 0.52 |
| 8 | 986.0 (830.2-1110.8) | 5910 | 20757 | 35090 | 12.8 | 1.69x | 0.21 |
| 16 | 1192.5 (1140.4-1250.7) | 6490 | 40539 | 67371 | 12.8 | 2.04x | 0.13 |
| 32 | 1139.1 (1097.4-1186.0) | 8911 | 99066 | 162437 | 12.8 | 1.95x | 0.06 |
| 64 | 1081.1 (1026.9-1131.4) | 13787 | 227120 | 380216 | 12.8 | 1.85x | 0.03 |

**Scaling gate: 1.85x aggregate ops/s at 64 clients against the 1-client step (0.03 of ideal), floor 1.50x — PASS, throughput grows with client count.**

RSS across the whole sweep: 6.8 MB -> 12.8 MB (delta +6.0 MB). Ceiling 71.8 MB (store page cache + 1 MB/client). Database at exit: main 692224 B, `-wal` 24752 B, `-shm` 32768 B — the WAL is unflushed for as long as the store holds the file open. Panicked clients across the whole sweep: 0.

## What this does not measure

- HTTP, MCP and JSON cost. The clients are threads over the library call, so axum, tokio and a socket are deliberately absent.
- Any store but `SqliteStore`, any second process, and any deployment where the server is not the only reader or writer.
- Request redaction, non-default budgets, tag filters, and the lifecycle routes.

## Limitations

- A p99 over 64 threads on a shared machine is scheduler noise before it is store contention. Quote the ops/s range; treat the percentile tail as indicative only.
- `--op retain` grows the bank by `clients x ops` rows over the sweep, so later steps run against a larger bank than earlier ones. Recall cost is pool-bounded so the effect is small, but the step column is not a constant-size measurement.
- One step bounds a leak, it cannot prove the absence of one: rerun at a different `--ops-per-client` and check the RSS column does not track the work done.
- `SCALING_FLOOR` is an absence-of-gain check, not a performance target. Passing it means the store overlaps *some* work, not that it scales well — read the `Eff` column for how much of the ideal gain is actually realized, since a store that releases the lock during the non-SQL part of a recall can grow aggregate throughput while still serializing every query. A regression that halves efficiency can pass this gate.
