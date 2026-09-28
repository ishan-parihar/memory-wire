# Footprint (memory-wire)

Run 2026-09-28 from `--profile=release` on AMD Ryzen 9 5900X 12-Core Processor, 24 logical CPUs.

Measured, not hand-copied: the README's headline numbers were, until this harness existed, measured by hand — `ls -l`, `ps -o rss` and a manual `wal_checkpoint` done at some point in the past. (When this harness was written the README read "8.4 MB binary, 10.5 MB idle RSS, 3.0 MB for 10k memories"; its current figures are re-pinned from runs of *this* harness, and the RSS rows below are what it publishes.) This harness is that procedure, committed, so a change can no longer require redoing it by hand. Corpus 10000 rows in `scale_sweep`'s four-topic shape, `--repeats 3`, median reported.

| Dimension | Value | How read |
|---|---|---|
| Release binary | 8836032 B (8.43 MB) | `std::fs::metadata` on the binary next to this example's own path |
| Harness idle RSS (floor) | 2.86 MB | `VmRSS`, this process, before any store is opened |
| Harness RSS with 10000 rows | 7.47 MB (7.43-7.51) | same process, store open, index built |
| Serve idle RSS | 9456 kB (9.23 MB) | `VmRSS` of a real `memory-wire serve`, after `/health` answers |
| Serve RSS after 1 retain | 11080 kB (10.82 MB) | same process, after one HTTP retain |
| Storage, WAL unflushed | 7237496 B (6.90 MB) | main 3014656 + `-wal` 4190072 + `-shm` 32768, store holding the file open |
| Storage, settled total | 3055616 B (2.91 MB) | the same three files after `PRAGMA wal_checkpoint(TRUNCATE)` |
| Storage, settled main file | 3022848 B (2.88 MB) | main only — the figure `eval/SCALE_SWEEP.md` and the README quote |
| Index bytes per 1k memories | 305561 B (0.29 MB) | settled total / rows x 1000 |

**The two storage figures are different measurements, not a discrepancy.** The store runs in WAL mode: while the process holds the database open, 10000 rows occupy 7237496 B (6.90 MB); after a clean `wal_checkpoint(TRUNCATE)` the same rows occupy 3055616 B (2.91 MB), of which 3022848 B is the main file and the remainder is the fixed 32768-byte shared-memory mapping. A server that has just been fed 10000 memories and not yet checkpointed is using 2.4x the settled figure. The README and `eval/SCALE_SWEEP.md` quote the settled main file and say so; this harness measures both every time, so a change that moves one and not the other shows up as exactly that.

## What this does not measure

- RSS under load. Nothing here is concurrent; `examples/soak.rs` and `examples/bench_concurrency.rs` bound that.
- MCP stdio session memory, or a client that keeps connections open.
- Any backend but SQLite, and any index-only figure: the storage rows are the whole store (tables, indexes, FTS5 shadow tables and free pages), not the search index alone.
- What a `VACUUM` would reclaim. Nothing here vacuums, so growth and free-page reuse are indistinguishable in these numbers.
- A packaged or stripped binary. Binary size is the raw `target/release` file, LTO build, unstripped by whatever profile `Cargo.toml` declares.

## Limitations

- RSS comes from `/proc`, so every RSS row reads `unavailable` on a platform without it and the run still completes — the same degradation `examples/soak.rs` has. Off Linux the storage and binary rows are still exact.
- RSS is a whole-process number: it includes allocator arenas and whatever the kernel has mapped, so it can move for reasons unrelated to the store. That is why `--repeats` exists and why a range is printed rather than a point.
- Storage depends on the filesystem `--db` lands on. The figures above assume an ordinary local disk; tmpfs, a network mount and a copy-on-write filesystem will each report something else, and none of them is comparable to these.
- The harness RSS rows and the serve RSS rows are different processes doing different work. The harness rows are a floor for a bare library user; only the serve rows are comparable to the README's headline.
