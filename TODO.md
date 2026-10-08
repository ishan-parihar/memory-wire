# TODO — what is left to implement

Started 2026-10-08, at `v0.7.0`. One row per item; when an item closes,
strike it and name the commit — the `docs/INTEGRATION_GAPS.md` convention.
This file is the *implementation* register. The domain docs keep their own
lists and this file points at them rather than duplicating them:

- `docs/INTEGRATION_GAPS.md` "What is left": G4 (bank opt-in policy),
  G6.3 (`--uninstall --guidelines` still refused), G6.4 (no `--endpoint`
  flag on the hook path), G6.8 (`system_prompt_block`), G6.9 (doctor does
  not check that installed hooks name a live binary)
- `docs/INTEGRATION_PLAN.md`: Phase 0/1 correctness items; two decisions
  were blocking Phase 1 when it was written
- `docs/RETRIEVAL_EFFICACY_PLAN.md` §7: operant-side tickets — a different
  repo (`agentic-harness/operant`), indexed here so nobody hunts for them
  in this one

## Open

| # | item | why | where | acceptance |
|---|---|---|---|---|
| 1 | `connect` preserves a surface's baked bank when `--bank` is absent | kills the installer's `--no-connect` dance on pinned machines (README §Installation documents it); today a bare re-connect silently re-defaults to `memory-wire` | `src/connect.rs` + test | against a temp `HOME` pinned to a second bank, a bare `connect <host>` re-wires with the same bank, unchanged |
| 2 | `doctor` reports a serve running an older binary than the installed one, with the restart command | nothing today detects the "serve keeps old behavior until restarted" window — the exact gotcha from the v0.7.0 deploy on both machines; the running binary outlives the swap by an unbounded time | `src/doctor.rs`, tail of `install/get-memory-wire.sh` | a stale serve produces a finding line naming the restart command; a current serve produces none |
| 3 | `memory-wire update` subcommand | one command for check-latest → sha256-verify → swap → re-connect installed hosts (banks preserved via #1) → serve restart (via #2); today all five update steps are manual and one of them (hermes) is invisible to the installer entirely | `src/main.rs`, composing `connect.rs`/`doctor.rs`/`daemon.rs` | end-to-end test against a fake releases API served from a temp dir; the live network check stays behind a flag and never runs in tests |
| 4 | INTEGRATION_PLAN Phase 0, starting with P0.1 (hooks resolve the endpoint from the daemon state file) | re-verify each item against the `v0.7.0` tree first — the plan was written 2026-09-30 and its blocking decisions may have been answered by the retrieval-efficacy waves | `docs/INTEGRATION_PLAN.md` | the plan's own gates |
| 5 | GAP-8: name the `hook:stop` marker producer | effect already closed — it execs the installed binary, so its markers are tagged and auto-excluded since `v0.7.0`; only the identity is unknown (absolute-path invocation, absent from every visible binary, config and package on the box) | start at the 2026-10-08 addendum in `docs/INTEGRATION_AUDIT.md` | a named producer with evidence, or a recorded decision to stop looking |
| 6 | dev-set growth before any score gating | the standing decision is tag hygiene, no threshold (CONSISTENCY §32); gating may only be revisited after the labelled set grows well past today's 10 cases — and the D-case judgment (cross-project turn rows ranking in a single-user bank) is the first thing to re-measure | `eval/relevance/`, `docs/EVALUATION_HYGIENE.md` | a gated change carries its dev-set numbers and states its fitted-ness, per AGENTS §6 |

## Deliberately not doing

- **Score thresholds on auto-injection** without the dev set above — the
  lever list is composition, caps, labels and tags; measured in
  CONSISTENCY §32.
- **Parent parity, tool-count matching, container images** —
  `docs/INTEGRATION_GAPS.md` "Explicitly not recommended"; the reasons are
  measured, not stylistic.
- **Anything LongMemEval was consulted to choose** — AGENTS §6, always.
