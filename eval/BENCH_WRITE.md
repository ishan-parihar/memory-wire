# Write path (memory-wire)

Run 2026-09-28 from `--profile=release` on AMD Ryzen 9 5900X 12-Core Processor, 24 logical CPUs.

Three write paths over the same generated content, each from a fresh database. `service retain` is the full `MemoryService::retain` (redaction, bank-config read, content-hash dedup, one transaction, one commit). `store put` is `Store::put` (one row, one commit, no service work). `batched insert` is N rows in ONE `rusqlite` transaction, paying one commit instead of N. Sizes 1000 / 10000; 1 repeat(s) per cell, the median wall time is reported and ops/s carries its range.

Store connection configuration: `journal_mode=wal`, `synchronous=2 (FULL)` as an **audit connection** reads them, i.e. SQLite's compiled-in defaults. The store does **not** run with the `synchronous` shown here: it sets `PRAGMA synchronous=NORMAL` in `configure()`, and both pragmas are per-connection so an audit handle cannot see that. Every absolute figure in this artifact was measured under the store's real configuration — `journal_mode=wal`, `synchronous=NORMAL` (no fsync per commit), not the `FULL` an audit handle reports. The D1 before/after is in `docs/CONSISTENCY.md` §11.1; the numbers here are the *after* side, so a `FULL` audit reading must not be read back as the cost of these writes.

Main-file bytes are read after the store closed, so the WAL is already folded in. The WAL-*unflushed* figure — what a server that has just been fed 10k memories and not yet checkpointed occupies — is a different measurement and belongs to `eval/BENCH_FOOTPRINT.md`.

| Memories | Path | Build time | ops/s (min-max) | mean us/op | p50 us/op | main B | B/memory |
|---|---|---|---|---|---|---|---|
| 1000 | service retain | 0.56s | 1782.6 | 561 | 318 | 434176 | 434 |
| 1000 | store put | 0.47s | 2149.7 | 465 | 244 | 376832 | 377 |
| 1000 | batched insert | 0.09s | 10914.8 | 92 | n/a | 299008 | 299 |
| 10000 | service retain | 6.84s | 1461.9 | 684 | 337 | 3600384 | 360 |
| 10000 | store put | 5.53s | 1808.4 | 553 | 275 | 3022848 | 302 |
| 10000 | batched insert | 1.14s | 8767.7 | 114 | n/a | 2252800 | 225 |

At 1000 memories: one-commit-per-row is **5.1x** the wall time of one commit for the whole batch (0.47s vs 0.09s), and the full service path is **6.1x** it. A `synchronous` change moves the first ratio; only an API that can batch moves the second.

At 10000 memories: one-commit-per-row is **4.8x** the wall time of one commit for the whole batch (5.53s vs 1.14s), and the full service path is **6.0x** it. A `synchronous` change moves the first ratio; only an API that can batch moves the second.

**The ratio is not a constant, and must not be extrapolated.** It moves with the corpus because the batch stops being commit-bound: at the smaller size the batch is one fsync against a handful of rows, so the ratio is nearly the whole difference; by the larger size the batch is dominated by building the FTS index for every row, which no pragma can avoid. Read the ratio as *the share of single-row write time that is commit overhead at that size*. The ops/s range is the number to quote for the absolute cost.

## What this does not measure

- Concurrency. Nothing here runs two writers at once; that is `examples/bench_concurrency.rs --op retain`.
- A batch through the public API. `Store` has no batch write, so the batched row goes through `rusqlite` directly against the same schema. It populates `memories_fts` identically (the `memories_ai` trigger does that work), which is what makes the commit-count comparison valid — but it skips redaction, dedup and tag handling, so it is not a faster way to retain, only a way to price one commit.
- Recall over the written bank, document upserts, tag writes, deletes, or settled storage after a bulk load (`examples/bench_footprint.rs` reports that).
- Real content cost. Generated strings are short and PII-free, so redaction and content hashing see the cheap end of their input distribution; a corpus with emails and tokens in it redoes more work per row.

## Limitations

- Every `service retain` and `store put` row is its own transaction, so this is the worst case by construction. A caller that batched through a future API would land nearer the batched row.
- At 10k memories the total is dominated by fsync and moves with the storage underneath it. That is why `--repeats` exists and why the ops/s range is the number to quote.
- `B/memory` is per path and not comparable across paths at equal row counts: `service retain` also writes `memory_tags` rows and a `content_hash`.
- A p50 is reported for the two single-row paths only. The batched path has one duration, so a percentile over it would be invented; its `mean us/op` is the honest figure.
- The store-put-to-batch ratio is size-dependent, as the prose above the table says. A ratio quoted without its size is not a measurement.
