# Outline plan — remaining gaps, redundancy, and upgrades (2026-11-21)

Written at `1485add`. The previous plan
(`2026-11-20-remaining-gaps-plan.md`) was written the same way this one starts —
from the tree — and a rulings review then found four errors in it, including one
structural misframing. **This plan is built to be reviewed the same way, and it
names the questions the last one failed to ask.**

The headline is different from last time. The register is nearly empty (7 open
rows, 4 of them blocked on you or the world). What is actually left is not
cleanup — it is **one unmeasured gap that matters more than everything on the
register**, and two structural questions nobody has asked.

---

## 0. What this plan is not

- **Not a re-litigation of `docs/handoff/CLOSED.md`.** Nothing here proposes
  vectors, a reranker, consolidation, or a score threshold (AGENTS §8; §32/§48
  on the threshold).
- **Not a release plan.** The operator has ruled: **release after development
  is final**. R3a was measured (10.6515 MiB, `DT_NEEDED=3`, +3.92% over
  v0.7.0, 68 commits past `v0.10.0`) and **not cut**. That stands.
- **Not another register sweep.** The last review's sharpest structural point
  was that I answered "gaps, redundancies or upgrades" with "remaining register
  rows". This plan leads with the questions the register cannot answer.

---

## 1. What I verified before writing this

| check | result |
|---|---|
| `git status` | clean, HEAD `1485add` |
| TODO rows | 7 open, 31 done |
| open rows | 10, 11, 12, 13, 31, 36, 38 |
| installed vs repo | **0.9.0** vs **0.10.0**, 68 commits behind |
| release | measured, **not cut** (CONSISTENCY §71) |
| `v0.10.0` exists | yes; `v0.9.0..HEAD` = 96, `v0.10.0..HEAD` = 68 |
| integration test files | 6 files, 22 tests, **none per-host round-trip** |
| server-side injection format | none — `format=summary`/`toon` exist for recall bodies, not for an injection block |
| policy duplication | 4,037 lines across the four auto-injection doors |

---

## 2. The gap that matters most, and it is not on the register

**Nothing in this repository verifies that a memory written in session 1
reaches a model's prompt in session 2, on any real host.**

Verified: `grep` across `tests/` for `before_agent_start`, `ev.system`,
`inject` returns nothing. The six integration test files cover backup, daemon,
e2e retain/recall/reflect, installer, recall budget and scale — **none drives a
host and reads back what the host injected.**

This is the exact failure that produced the worst defect on record. TODO 35: the
opencode door **shipped dead on the host it named**, and it was found by
accident, while probing for something else. Every Rust test passed. The door
exported the v1 shape against a 2.x loader, `connect` reported success, and the
host silently had no memory.

Why this is the top item and not a footnote:

- Every other defect in this repository's record is a *policy* defect in a door
  that was at least running. This one is "was it running at all".
- The last review named it independently and called it cheaper than the parity
  matrix it had been asked to rank. **A write-then-read smoke test is one
  script, not a four-driver suite.**
- It is the only check that catches the class `connect` cannot: a host that
  changed its loader between versions.

**What "done" means:** a script that, for each detected host, starts a real
memory-wire, wires the host, drives the host's own lifecycle (or the closest
thing available without an API key), and asserts the injected block contains a
marker string that was retained through the normal path. Where a host needs a
live agent runtime and none is installed, the check prints **SKIPPED** loudly —
never silent, because a silent skip is the opencode door again.

---

## 3. Tier 1 — build now, no decision needed

### 3.1 The per-host write-then-read smoke test (§2)

New `plugin/integrations/door_roundtrip_test.sh`, wired into the trust gate.
Sourced from `Host::ALL` for the same reason as `5aea83a`: a literal list is how
round 4 found a script covering 6 of 8 hosts.

Cost: the bulk is a stub server (memory-wire already has one pattern for this in
`trust_gate_test.py`) plus a driver per host shape — `Style::Mcp`,
`Style::Plugin`, `Style::Extension`, `Style::Copilot`. Four drivers, not eight
hosts.

### 3.2 TODO 38's remaining half, rescoped per R2

The review's R2 finding stands and I am adopting it: the original "one fixture
set, four doors, one oracle" spec was wrong because the doors **legitimately
render differently** (`###` vs `##`, bullets vs numbers, footer vs none, chars
vs code points vs UTF-16 units). A single oracle would be red on day one for
reasons that are design.

So the rescope, which is the review's own "cheapest version that still catches
the class":

- **The `Host::ALL` half is done** (`5aea83a`): two tests, enumerate `ALL`,
  wire and unwire in a pristine home.
- **New:** a cross-door *row-inclusion* invariant, which is the thing that
  actually broke four times (trust before take, blank before take, footer
  count). Not rendered bytes — which rows were included.
- **New:** a `cargo test` that fails when a host in `Host::ALL` has no
  door-table entry.
- **Dropped:** the four-door rendered-output oracle. The trust gate's new
  behavioural comparison (7 shared fixtures, both TS doors, identical output)
  covers the TS pair where a shared renderer is meaningful.

### 3.3 TODO 31 — door parity residue: document the asymmetry, don't keep adding doors

Verified state: `redact` is on MCP; `hygiene` has no MCP/CLI door; `supersede`
has no HTTP route; `grounding` has neither. The review's audit of the last round
confirmed this is absence, not lockout.

Recommendation: **rule it and record it.** Every added door is another surface
to keep in parity, and the audit's own finding is that asymmetry is what happens
when each capability is added through the door its item needed. Add a short
section to `docs/INTEGRATION_GAPS.md` stating the doors are deliberately
asymmetric, with the reason. This is a one-page doc change that closes a
recurring review finding.

### 3.4 The count-claim hygiene the review exposed

The review found **ten** sites needing a tool-count correction, nine of which
TODO 34 listed. The tenth was hermes. That is not a one-off — it is a class:
**a number in prose that has no test behind it.**

Cheap fix: extend the module-table gate's shape to cover the counts that have
actually drifted — tool count, host count, module count — each sourced from the
enum or the code, never a literal. `Host::ALL.len()` is 8; `src/mcp.rs`'s tool
list is 6. A test asserting "the number in `AGENTS.md` §4 equals the number in
the code" is ten lines and would have caught all ten sites.

**And the very act of writing this plan found an eleventh, today.** `AGENTS.md`
line 200 opens **"**Agent hosts.** Six: `claude-code`, `codex`, `copilot-cli`,
`cursor`, `opencode`, `hermes`"** while `Host::ALL` holds **eight** (adding
`omp` and `pi`) and `IMPLICIT` holds seven. The section then spends three
sentences on hermes's asymmetry without ever naming the two hosts the list
omits. This is the same defect as the ten the review found, caught by the same
method: read the prose, then read the code it describes.

---

## 4. Tier 2 — the two structural questions nobody has asked

These are the last review's sharpest contribution, and they are questions, not
conclusions. Both are recorded here so they are not lost.

### 4.1 Why does rendering policy live in the doors at all?

Every defect on record is the same shape: rendering policy (trust gate, blank
rows, footer, cap) re-implemented client-side in Rust, Python and TypeScript.
4,037 lines across four doors, and a check in N−1 of them — which is TODO 33's
finding, restated.

The server already serves `format=summary` and an opt-in `toon`. **A
server-rendered injection format is the untested alternative**: one
implementation, parity nearly trivial, doors thin.

What the pack could not settle, and what would:

- Does it conflict with the frozen HTTP error contract (AGENTS §9)? A new
  response shape is an API change; a new *content type* on an existing route may
  not be.
- Do the doors' query and cache logic survive it? The extension derives its
  query from `lastPrompt` and clears it after; hermes passes `_strip_reminders`
  of the last user turn. Both are client-side by necessity — the server never
  sees the prompt.
- What does it cost on the wire? The block is capped at `INJECT_CAP_CHARS` in
  every door; server-side rendering means the server picks the cap.

**This is a design question and needs a decision, not an afternoon.** I am
recording it, not proposing to build it.

### 4.2 Does the opencode door actually load on the installed host, today?

TODO 35 fixed the export shape and the install layout. The fix was verified on
opencode 2.3.2 live. **But `plan §7` of the last plan admits no live 2.x load
has been checked since the port**, and the trust gate is explicitly a shape
check, not a load test.

One command, if opencode is installed here: load the plugin, inject a prompt,
confirm the block arrives. If it is not installed, the answer is "not verifiable
here" and §3.1's SKIPPED path is the honest result.

---

## 5. Tier 3 — blocked on you, or on the world

Unchanged from the last plan, with the counts corrected:

| # | item | blocker |
|---|---|---|
| 3.1 | release (measured, not cut) | **you** — 68 commits past `v0.10.0`, +3.92% over v0.7.0 |
| 3.2 | install 0.10.0 over the 0.9.0 binary | **you** — a write to your live environment |
| 10 | agent-memory-bench Claude Code arm | **external** — needs the benchmark's harness + your agent budget |
| 11 | LLM-judged runs (AMB/OAMB/memorybench/MemBase) | **you** — free-tier quota |
| 12 | AMB provider at turn granularity | decided **against** on the dev set; measurement only |
| 13 | aroute defects B1–B4 | **external** — a different repo, memory-wire agents don't edit it |
| 31 | door parity residue | becomes a doc change under §3.3, not a blocker |
| 36 | TS door duplication | **R1 ruled: fence, don't extract** (`333185b`). What remains is whether the fence should also cover the glue. Low value. |

**Note on 3.1:** the operator's ruling is explicit — release after development
is final. So this is not blocked on a decision, it is blocked on *this plan
being done*. That is the correct reading and I am not going to re-ask.

---

## 6. What I am *not* proposing, and why

- **No TS door extraction.** The rulings review's measurement (`git log -L` over
  the 8 shared functions) showed divergent history in two, both of them commits
  from the same session, so extraction buys nothing *observed*. Fenced instead,
  behaviourally, in `333185b`.
- **No shared abstraction over `connect_*`.** B1 extracted the byte-identical
  helpers; the remainder is golden-expectation tests that would be obscured by
  one.
- **No new dependency.** AGENTS §1: the default build must not gain one.
- **No re-proposal of any closed lever** (`CLOSED.md`).
- **No change to the four `ponytail:` simplifications.** Each names a ceiling
  and an upgrade path.
- **No touch to any count inside a historical-record fence**
  (`docs/BENCHMARK.md`'s "4 tools" is a dated record).

---

## 7. Order, and what each tier costs

| # | item | tier | ~size | blocked on |
|---|---|---|---|---|
| 3.4 | count-claim gate (tool/host/module) | 1 | ½ day | no |
| 3.1 | per-host write-then-read smoke test | 1 | 1–2 days | no |
| 3.2 | TODO 38 rescoped: row-inclusion invariant | 1 | ½ day | no |
| 3.3 | door-parity asymmetry ruling, documented | 1 | an hour | no |
| 4.2 | opencode live load check | 2 | minutes | opencode installed |
| 4.1 | server-rendered injection: decide, don't build | 2 | a ruling | **you** |
| 5 | release, install, benchmarks, aroute | 3 | — | you, or external |

**Recommended order:** 3.4 → 3.1 → 3.3 → 3.2 → then stop and ask about 4.1.

That front-loads the cheapest check with the widest reach (3.4 catches a class
that produced ten defects), then the one unmeasured gap that matters (§2), then
the two doc-level items that close recurring review findings.

---

## 8. What I could not determine

Per AGENTS §12, stated rather than guessed:

- **Whether opencode is installed on this machine, and whether the current
  plugin loads.** I have not checked; §4.2 is the check.
- **Whether a server-rendered injection format is compatible with the frozen
  contract.** I can read the contract; I cannot settle whether a new content
  type on an existing route counts as a break without a ruling.
- **How many users the "other people's machines" cost in R1(d) actually
  represents.** The last review flagged this and it is still unevidenced: one
  box and one VPS.
- **Whether 3.92% growth is "material".** AGENTS §1 names no number. That line
  is the operator's to draw.

---

## 9. Definition of done

Tier 1 closed with a gate green and a `CONSISTENCY.md` section. Tier 2 answered
with a ruling recorded either way — **a question answered "not now, and here is
why" is done**, and a question silently dropped is the phantom that
`docs/handoff/` exists to prevent. Tier 3 items done on your word or recorded as
blocked with the blocker named.

And: this plan gets reviewed before it is executed, the same way the last one
did. That is not ceremony — the last review found four errors in a plan I had
checked against the tree, and one of them was a structural misframing that
would have sent three days of work at the wrong problem.
