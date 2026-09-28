# Pinned sources — memory-wire synthesis baseline

Synthesis date: 2026-09-27. All numbers below are the exact inputs the current
Rust code was transmuted from. To update: `git -C _audit/<repo> pull`, re-run
the audit deltas, record the new SHAs here, then port behavior per §3.

> **These pins describe the tree as published in `v0.3.0`, and the tree is now
> ahead of that.** `v0.3.0` **is** a published GitHub release (2026-09-28, asset
> `memory-wire-linux-x86_64.tar.gz`), which is why the earlier "the tree is ahead
> of the published release" banner was retired. That retirement no longer holds:
> `HEAD` is **19 commits past the `v0.3.0` tag**, with a large uncommitted wave on
> top, while `Cargo.toml` still says `0.3.0`. So the version string describes the
> release and not the tree, and nothing in §1–§3 below covers the `embed` feature,
> `build.rs` or `src/vector.rs` — all of which are post-`v0.3.0` tree state.
> §1's competitor SHAs are the synthesis baseline the current code was transmuted
> from and were deliberately not re-pulled for this cut; §2 is the toolchain that
> built it; §3 is its locked direct dependency set. The per-build evidence for
> `v0.2.0` and earlier stays where it was measured, in `docs/BENCHMARK.md` and
> `docs/CONSISTENCY.md`, and the record of this framing being retired and then
> becoming true again is `docs/CONSISTENCY.md` §15.2.

## 1. Competitor snapshots (`_audit/`, git, branch `main` both)

**Not re-pulled on 2026-09-28.** These SHAs are the synthesis baseline the code
was transmuted from, and the code in this tree still is. Re-pulling is a
separate task (procedure in §4); nothing in the 2026-09-28 work changed an
upstream input, so moving them would only break the delta.

| Repo | Remote | SHA | Commit date | Subject |
|---|---|---|---|---|
| hindsight | https://github.com/vectorize-io/hindsight | `ccfe85b4851957ac2adf88b4a9ddf9668b2882f1` | 2026-09-26 10:14:44 -0400 | chore(deps): bump pipecat-ai to 1.12.0 in pipecat integration lockfile (#4814) |
| agentmemory | https://github.com/rohitg00/agentmemory | `bcf4f0d00d1c71e9221287a4778e4122ce2d7574` | 2026-09-26 14:06:00 +0530 | perf(index): store vectors in fixed buckets and rebuild BM25 from content (#1416) |

Upstream package versions observed at those SHAs: `hindsight-api-slim` 0.10.1
(Python ≥3.11), `@agentmemory/agentmemory` 0.9.29, iii-engine v0.22.1.

## 2. Toolchain

- `rustc 1.98.0 (88d9e12ae 2026-08-18)`, `cargo 1.98.0 (797e8a9bc 2026-08-05)`
  — re-verified 2026-09-28, unchanged.
- `memory-wire` 0.3.0, edition 2021, `MIT OR Apache-2.0` — the version this
  file describes.

## 3. Locked direct dependencies (`cargo tree --depth 1`)

**Complete as of 2026-09-28.** The gap the previous revision flagged is closed:
`rmcp 1.8.0` is now listed, and the whole row was regenerated rather than
patched.

- anyhow 1.0.104, axum 0.7.9, chrono 0.4.45, clap 4.6.7, regex 1.13.1,
  **rmcp 1.8.0**, rusqlite 0.32.1 (bundled), serde 1.0.229, serde_json 1.0.151,
  sha2 0.10.9, thiserror 2.0.21, tokio 1.53.1, tracing 0.1.44,
  tracing-subscriber 0.3.23, uuid 1.26.1

`rmcp` is declared `rmcp = { version = "1.8", default-features = false,
features = ["server", "transport-io"] }`. Its `schemars 1.2.2` is transitive and
correctly does not belong in this list.

**Removed at `v0.3.0`, and back since — the lock state has moved again.** On
2026-09-28, at the `v0.3.0` cut, `fastembed`, `ort` and `tokenizers` had zero
matches in `Cargo.lock` and the `embed` cargo feature was gone from `Cargo.toml`.
**None of that is true of the tree now.** `Cargo.toml` carries
`fastembed = { version = "7.1", optional = true, default-features = false }` under
`embed = ["dep:fastembed"]`, and `fastembed`, `ort`, `ort-sys` and `tokenizers`
are all present in `Cargo.lock` again.

The direct-dependency list above still describes a **default** build correctly,
because `default = []` and nothing outside the `embed` feature references
`fastembed`, so a default build does not activate it. `cargo tree --depth 1`
shows the default graph; `cargo tree --depth 1 --features embed` is the
invocation that shows `fastembed`. Re-run §4 and regenerate the list once
`embed` is settled, since the two graphs now differ.

- Full transitive lock: `memory-wire/Cargo.lock` (commit it; CI uses `--locked`)

## 4. Transmute procedure (future pulls)

1. `git -C _audit/hindsight pull && git -C _audit/agentmemory pull`
2. `git -C _audit/<repo> log -1 --format='%H %ad %s' --date=iso` → append rows to §1
3. Diff upstream `retain/recall/reflect`, hook protocol, RRF/MCP surfaces against
   `src/{api,recall,capture,store}.rs`; port behavior-only (no copied code)
4. `cargo test` + full clippy gate + `tests/e2e.rs` + benchmark (§5 of benchmark report)
5. Update this file + `PLAN.md` status; commit lockfile with the change
