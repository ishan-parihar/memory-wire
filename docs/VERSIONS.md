# Pinned sources — memory-wire synthesis baseline

Synthesis date: 2026-09-27. All numbers below are the exact inputs the current
Rust code was transmuted from. To update: `git -C _audit/<repo> pull`, re-run
the audit deltas, record the new SHAs here, then port behavior per §3.

> **These pins describe the tree as published in `v0.3.0`; the current release
> is `v0.5.0`.** `v0.5.0` was published 2026-09-29T20:37:13Z with assets
> `memory-wire-linux-x86_64.tar.gz` (3,978,654 B) and
> `memory-wire-linux-aarch64.tar.gz` (3,503,968 B), built **on this machine**, not
> by GitHub Actions. It is the first release to carry a Linux aarch64 asset, and
> the first with no macOS gap closed — those still require Apple's SDK. It carries
> `build.rs` and `src/vector.rs` as tree state — all of which post-date `v0.3.0` —
> and none of §1–§3 below was re-derived for that cut. §1's competitor SHAs are the
> synthesis baseline the code was transmuted from and were deliberately not
> re-pulled; §2 is the toolchain that built it; §3 is its locked direct dependency
> set **as of `v0.3.0`**. `Cargo.lock` gained `fastembed`, `ort`, `ort-sys` and
> `tokenizers` back under the optional `embed` feature, and the version field moved
> to `0.5.0` — neither is reflected in §3. The per-build evidence for `v0.2.0` and
> earlier stays where it was measured, in `docs/BENCHMARK.md` and
> `docs/CONSISTENCY.md`; the record of this banner being retired, becoming true
> again, and being retired a second time is `docs/CONSISTENCY.md` §15.2 and §18.7.

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

  **Complete as of 2026-09-28. `rmcp` version updated 2026-09-29.**
  The gap the previous revision flagged is closed: `rmcp` is now listed, and the
  whole row was regenerated rather than patched.

  - anyhow 1.0.104, axum 0.7.9, chrono 0.4.45, clap 4.6.7, regex 1.13.1,
    **rmcp 3.5.0**, rusqlite 0.32.1 (bundled), serde 1.0.229, serde_json 1.0.151,
    sha2 2.10.9, thiserror 2.0.21, tokio 1.53.1, tracing 0.1.44,
    tracing-subscriber 0.3.23, uuid 1.26.1

  `rmcp` is declared `rmcp = { version = "3.5", default-features = false,
  features = ["server", "transport-io", "transport-streamable-http-server"] }`.
  Its `schemars 1.2.2` is transitive and correctly does not belong in this list.

  **`rmcp` 3.5.0 (2026-09-29) moved the MCP transport to spec `2026-07-28`,
  which is stateless.** The HTTP mount now serves every request with no session
  and no `initialize` handshake; a client POSTing `tools/list` cold succeeds, and
  no `Mcp-Session-Id` is issued. A legacy client that still handshakes at
  `2025-06-18` is answered and negotiates `2025-06-18`, so the three hosts that
  depend on that surface are unaffected. `GET /mcp` now answers `405`; the SSE
  stream it used to open never carried a notification. Cost: **+291,872 B
  (+2.85%)** on the release binary, `DT_NEEDED` unchanged at 3.

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
