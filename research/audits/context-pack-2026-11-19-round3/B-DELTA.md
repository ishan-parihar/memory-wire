# Delta since round 2 (2026-11-18 → 2026-11-19)

Round 2 audited the shipped W0–W7 tree at `b4b4698`. Three commits have landed
since. This section is what changed, with the evidence class marked inline, so
the reviewer audits the *delta* against the record rather than re-deriving it.

## D1 — `1f5ee17`: the trust tier had no bottom rung

`[LIVE]` `src/api.rs`, `src/hooks.rs`, `plugin/integrations/hermes/memory-wire/__init__.py`

Round 2's finding 4a ("no read-time trust gate: W2 only downgrades on write")
was verified and adopted. Implementing it exposed a deeper defect the finding
did not name: `Trust::tier` accepted **three** tiers (`agent_inferred`,
`human_stated`, `verified`) and mapped every other value to `agent_inferred` —
**including `untrusted_external`**, the one tier the gate refuses. The
plan's own schema table and `src/memory.rs` both documented that value as
storable. So the gate, built as specified, would have been **dead code that
every test still passed**: no row could carry the marker it matched on.

What shipped: `UntrustedExternal` added below `AgentInferred` (the ceiling's
`downgrade` now bottoms out there); `trust` carried on the `full`/`summary`
recall wire; the tier dropped from `hooks::recall_section` **before** the
`MAX_LINES` take; the same hole closed in the hermes plugin's `prefetch`.

Explicitly **not** a relevance threshold: a deterministic allow/deny on a
stored tier with no fitted constant. CONSISTENCY §32/§48 (no score threshold,
ever) is untouched.

## D2 — `81a2212`: three recall failures were byte-identical silence

`[LIVE]` `src/hooks.rs`, `docs/CONSISTENCY.md` §52

`recall_at_fmt` returned `None` for three different reasons — connection
refused, a non-2xx answer, and a 200 body this build could not parse — and all
three produced the same observable: exit 0, empty stdout, nothing anywhere. An
agent and a user could not tell "no memories matched" from "the server is down"
from "the hook and the server disagree about the wire".

Each path now emits a distinct `tracing::warn!` on stderr. Exit stays 0, stdout
stays empty, `RUST_LOG=off` is total silence. Observed live against three real
servers (127.0.0.1:1; a 404 responder; a 200 with `{"not":"an array"}`).

## D3 — the architecture doc is now demonstrably stale

`[LIVE]` `docs/ARCHITECTURE.md` vs `src/lib.rs`, measured this round

Not a code defect but a **documentation** one, and it is the kind a reviewer
should weigh: the doc that exists to tell a newcomer what this system *is*.

- It says "a Rust library with **five modules** behind one service", then lists
  **six** rows in its own table. `src/lib.rs` declares **nine** `pub mod`s:
  eight unconditional (`api`, `capture`, `embed`, `hygiene`, `grounding`,
  `memory`, `recall`, `store`) plus `vector` behind the `embed` feature.
- Its module table omits `hygiene`, `grounding`, `embed`, `vector`,
  `connect*`, `daemon`, `doctor`, `mcp`, `cli_home` and the rest of the binary's
  own modules entirely.
- "One service, **three doors**" lists HTTP, MCP-stdio and CLI. There are four:
  `mcp_http` mounts MCP on the serve router (`/mcp`, `/mcp/<bank>`), which is a
  fourth door on the same service. `docs/INTEGRATION_GAPS.md` (added this
  round) records the asymmetry per capability.

The doc was written at v0.4.0-scale and never re-derived after W0–W7.

## D4 — the round-2 verification's own open item

`[LIVE]` `TODO.md` row 31

Round 2's sharpest structural finding (its §3a) was verified and adopted as
"door parity": one `MemoryService`, four doors, each exposing a **different**
subset. Recorded in `docs/INTEGRATION_GAPS.md` with the verified table; left
open because closing it is a surface decision (document the asymmetry as
intended, or ship `redact` on MCP/HTTP) rather than a bug fix.

The widest gap in that table: **`redact` has no MCP tool and no HTTP route.**
An agent that retained a secret through MCP cannot retract it through MCP.

## What has NOT changed since round 2

- No ranking, scoring or fusion parameter moved. `[LIVE]` `docs/handoff/CLOSED.md`
  is unchanged; the fitted `overlap 0.25` is still fitted, not earned.
- The gate at HEAD: 659 tests, clippy clean, 0 doc warnings. `[LIVE]`
- No new dependency. `DT_NEEDED` is still 3, binary 11,141,240 B. `[LIVE]`
- LongMemEval consultation count unchanged at 5. `[LIVE]` `CONSISTENCY.md`