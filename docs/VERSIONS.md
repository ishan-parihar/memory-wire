# Pinned sources — memory-wire synthesis baseline

Synthesis date: 2026-09-27. All numbers below are the exact inputs the current
Rust code was transmuted from. To update: `git -C _audit/<repo> pull`, re-run
the audit deltas, record the new SHAs here, then port behavior per §3.

> **The working tree is substantially ahead of published `v0.2.0`.** Everything
> below is unchanged and still correct for `0.2.0`; the tree on `main` carries the
> SQLite/recall work, the TTL sweep, `detail=none` and the five benchmark
> harnesses that are **not** in the `v0.2.0` tag or release. The `0.2.0` version
> string in `Cargo.toml` is therefore *not* the version of the tree. Cutting the
> next release is a separate decision and nothing in this commit cuts one.

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
- `memory-wire` 0.3.0, edition 2021, `MIT OR Apache-2.0` — unchanged. See the
  banner: the tree is ahead of the `0.2.0` tag.

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

**Removed, and confirmed absent from `Cargo.lock` (2026-09-28):** `fastembed`,
`ort`, `tokenizers` — zero matches for all three names. The `embed` cargo feature
is gone from `Cargo.toml`. This is what the README means when it says the ONNX
dependency is gone; the lockfile is the proof.

- Full transitive lock: `memory-wire/Cargo.lock` (commit it; CI uses `--locked`)

## 4. Transmute procedure (future pulls)

1. `git -C _audit/hindsight pull && git -C _audit/agentmemory pull`
2. `git -C _audit/<repo> log -1 --format='%H %ad %s' --date=iso` → append rows to §1
3. Diff upstream `retain/recall/reflect`, hook protocol, RRF/MCP surfaces against
   `src/{api,recall,capture,store}.rs`; port behavior-only (no copied code)
4. `cargo test` + full clippy gate + `tests/e2e.rs` + benchmark (§5 of benchmark report)
5. Update this file + `PLAN.md` status; commit lockfile with the change
