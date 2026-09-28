# Scale sweep (memory-wire)

Method mirrors agentmemory `benchmark/SCALE.md`: ~8 obs/session; built-in tokens = full corpus chars/4; agentmemory tokens = top-10 hits chars/4 (constant). DB = file-backed SQLite+FTS5 bytes.

Run 2026-09-28 from `--profile=release`.

**`DB bytes` is the main SQLite file only.** The store runs in WAL mode, so while the process holds the database open most pages are still in `-wal` and the real on-disk footprint is larger than the column below. Both halves of that measurement belong to `eval/BENCH_FOOTPRINT.md`, which reports the WAL-unflushed and the post-`wal_checkpoint(TRUNCATE)` figures on every run; they are deliberately not repeated here, because a second copy of a byte count is a second thing to go stale.

| Memories | Sessions | Index build | Search p50 | DB bytes | Built-in tokens | Top-10 tokens | Savings |
|---|---|---|---|---|---|---|---|
| 240 | 30 | 97 ms | 988 µs | 131072 | 3362 | 144 | 95.7% |
| 1000 | 125 | 468 ms | 2005 µs | 372736 | 14097 | 144 | 99.0% |
| 5000 | 625 | 2569 ms | 5050 µs | 1519616 | 71597 | 144 | 99.8% |
| 10000 | 1250 | 4306 ms | 6033 µs | 3014656 | 143472 | 144 | 99.9% |

Search p50 is 20 recalls (4 queries × 5) per row, so treat single-digit-percent movement between runs as noise. **Index build and search p50 are wall-clock**: both move 2-3x with the machine's load and neither is a property of the build to be pinned. Token-savings is the share of the whole corpus the top-10 window replaces, so it is a property of the corpus shape, not of the index.
