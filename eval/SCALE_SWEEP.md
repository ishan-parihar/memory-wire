# Scale sweep (memory-wire)

Method mirrors agentmemory `benchmark/SCALE.md`: ~8 obs/session; built-in tokens = full corpus chars/4; agentmemory tokens = top-10 hits chars/4 (constant). DB = file-backed SQLite+FTS5 bytes.

Run 2026-09-27 from `--release` (`cargo run --release --example scale_sweep`), the
profile that ships. **DB bytes is the main SQLite file only** — the store runs in
WAL mode, so while the process holds it open most pages are still in `-wal`: at
10,000 memories the main file is 3,194,880 B but the true on-disk footprint is
7,442,440 B (main 3,194,880 + `-wal` 4,214,792 + `-shm` 32,768), falling back to
3,194,880 B after a clean `PRAGMA wal_checkpoint(TRUNCATE)`. Read this column as
the settled size, not the transient one. Search p50 is 20 recalls (4 queries × 5)
per row, so treat single-digit-percent movement between runs as noise.

| Memories | Sessions | Index build | Search p50 | DB bytes | Built-in tokens | Top-10 tokens | Savings |
|---|---|---|---|---|---|---|---|
| 240 | 30 | 313 ms | 588 µs | 126976 | 3362 | 144 | 95.7% |
| 1000 | 125 | 1404 ms | 893 µs | 393216 | 14097 | 144 | 99.0% |
| 5000 | 625 | 7173 ms | 2578 µs | 1634304 | 71597 | 144 | 99.8% |
| 10000 | 1250 | 15347 ms | 4903 µs | 3194880 | 143472 | 144 | 99.9% |
