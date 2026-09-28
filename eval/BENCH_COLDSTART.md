# Cold start (memory-wire)

Run 2026-09-28 from `--profile=release` on AMD Ryzen 9 5900X 12-Core Processor, 24 logical CPUs.

Three costs a warmup latency benchmark cannot see: process start to `/health` answering, store open on an existing database, and first recall against warm recall. Sizes 0 / 1000 / 10000; 3 repeats each; 20 warm recalls per size at a 2000-token budget.

## Process start to `/health`

Timed from before `Command::spawn` to the first `GET /health` that answers 2xx with the body `ok`, polled every 2 ms, over 3 spawns of a real `serve` on a scratch database. The 2 ms poll interval is added slop, so this is an upper bound. A spawn that never answered is excluded rather than recorded as a slow one.

| Run | Start to /health |
|---|---|
| first (binary not in page cache) | 173.5 ms |
| subsequent | 39.2 / 52.1 / 52.1 (min / median / max of 2) ms |

## Store open, integrity check, and first recall

`open` is `SqliteStore::open` on an existing database: connection pragmas, the idempotent `CREATE ... IF NOT EXISTS` batch, the column patches, and the FTS consistency check. `integrity` is SQLite's own `PRAGMA integrity_check` on a second connection — thorough, and therefore slower than the store's own check, which is why both are here. `rebuilt FTS` counts repeats where the store decided the index had drifted and reindexed; a nonzero value would mean that run's open cost is not comparable to the others.

| Memories | open ms | integrity ms | integrity verdict | rebuilt FTS | first recall ms | warm p50 ms | warm p95 ms | first/warm | main B |
|---|---|---|---|---|---|---|---|---|---|
| 0 | 20.12 | 0.37 | ok | 0 | 0.13 | 0.06 | 0.14 | 2.1x | 69632 |
| 1000 | 15.35 | 3.69 | ok | 0 | 2.18 | 1.96 | 4.17 | 1.1x | 376832 |
| 10000 | 27.10 | 41.30 | ok | 0 | 13.79 | 8.29 | 17.14 | 1.7x | 3022848 |

**A first-query penalty is visible at 2 of 3 sizes** (0: 2.1x, 1000: 1.1x, 10000: 1.7x), against a 1.2x bar: there the first recall after a fresh open costs more than the warm median, which is the FTS first-touch cost this harness was built to find.

## What this does not measure

- MCP stdio startup. That is `rmcp`'s handshake over a pipe, not the HTTP server's, and the two have nothing in common past the store open this table does cover.
- The `seed` and `sweep` commands, TLS, or a reverse proxy in front of the server.
- A genuinely cold binary: the page cache is warm after the first spawn, which is why the first spawn is reported on its own row. A first-ever run on a cold page cache is slower and this harness does not attempt to reproduce that condition.
- A database on a network mount, where the open cost is dominated by fetching the file rather than by anything the store does.
- Concurrency, and any op other than recall.

## Limitations

- A size of 0 is a *fresh* database, so its open cost includes creating the schema. That is not what a returning user's server pays, and the two must not be averaged together — which is why the size is a column rather than a run that is silently folded in.
- `PRAGMA integrity_check` is O(database) and is the slowest number in this table on a large store. The store's own startup check is the cheaper one it actually runs; the gap between the two columns is the cost of being thorough, not a cost the product pays.
- First recall is a single sample by construction: there is only one first recall per process. `--repeats` gives repeats by reopening the store, which is the same condition, but one sample is still one sample and its tail is not characterized. That is why the verdict uses a 1.2x bar rather than any amount above 1, and why a ratio a hair under 1 is reported as noise rather than as a fast first query.
- Start-to-health includes the OS scheduler placing the process, which on a loaded machine is the dominant term. The min-max across repeats is the honest presentation.
