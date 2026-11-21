# Outline plan: the remaining implementation gaps, consolidation, and operational upgrades

**Status:** proposal, 2026-11-21. Written from `2865938`, which is the state
the last session's plan review asked for and got. Every claim below was checked
against the tree or a live command; where I could not check, it says so.

**Predecessor:** `docs/plans/2026-11-21-remaining-gaps-plan.md`, whose Tier 1
landed (`dc97c40`, `3050ab2`, `3a4b1a6`, `303a096`) and whose three remaining
items were re-scoped by the review that read it.

---

## 0. What this plan is not

It is not a sweep for work to fill a list. I asked for the *actual* gaps, and
found four kinds, in descending order of how much they cost:

1. **A gap on the operator's own machine right now** (§1). Not a hypothetical.
2. **A gap in what the documentation says is true** (§2). Same class the last
   review spent its effort on.
3. **A gap in what is built but not wired** (§3). TODO 31, 36, 38.
4. **A gap in what is measured** (§4). One benchmark arm, one LLM-judged run.

Everything here is scoped to fit in sessions, and each tier is separately
committable. Nothing in §1-§2 should wait for §3-§4.

---

## 1. The operator's own machine is running a stale binary, and it shows

**Measured, not inferred.** `~/.local/bin/memory-wire` reports `memory-wire
0.9.0`, dated Oct 20. The tree is `0.10.0` at `2865938`, 68 commits past the
`v0.10.0` tag. Live consequences, each verified against the installed binary:

| what 0.9.0 lacks | evidence |
|---|---|
| `memory_redact` (the 6th MCP tool) | `git show v0.9.0:src/mcp.rs \| grep -c memory_redact` → **0**; current tree → 3 |
| `connect --list`'s newer host entries | installed `mcp` help advertises 4 tools, current advertises 6 |
| 68 commits of fixes, including the opencode v2 port (§62) | 68 commits between the tag and HEAD |

**One correction to the record.** `docs/CONSISTENCY.md` §67 says the installed
build "predates `memory_redact` (§65) and the new doctor block (§66)". The
first half is right and I verified it. **The second half is wrong:** the doctor
bank-finding block was added in `bde4b6a`, which is `v0.6.0~4` — it is in
0.9.0, and `memory-wire doctor` on this machine prints it correctly right now.
§67 should name what is actually missing, not a block that shipped five
versions earlier.

**Live defect this exposes, and it is not the binary's fault.** The doctor
prints:

> `finding  bank 'ishan-parihar-memory-wire' holds no memories; 'omp' holds 4241.
> A hook here resolves to 'ishan-parihar-memory-wire' and will recall nothing.`

That is the doctor working. It is also a machine where **the bank a hook
resolves to is empty and the memories are in `omp`** — 4,241 of them. Every
hook that fires in this directory recalls nothing. That is TODO 35's failure
mode, on the operator's live host, and it has been true since at least
`bde4b6a`. The doctor names it; nothing fixes it.

**Decision required, and it is the user's:** re-run the installer with
`--bank omp` (which forwards the pin to every `connect`, per §11), or leave it.
This plan does not run it. What it *does* propose is that the doctor finding
should not be the end of the story — see §3.4.

### 1.1 Ship or explicitly defer the release

`AGENTS §11` is unambiguous: "Do not tag, publish, or cut a release unless
asked." §68 measured the footprint and left the decision where it belongs. That
is correct and it should stand. What this plan adds is the *cost of not
deciding*, in one line each:

- a release would put `memory_redact` in the hands of the hosts already wired
- not releasing means the door-vs-binary drift in §1 keeps growing
- **and the 0.9.0 tag is 68 commits behind while `Cargo.toml` says 0.10.0**,
  so "the newest release" and "the newest code" are different things

**Recommendation:** no action in this plan. The user has been explicit that a
release waits until developments are final, and §3 has real work left.

---

## 2. Documentation that would mislead an agent (small, but it is the point)

The last review's sharpest finding was that `AGENTS.md` — the file an agent is
told to read first — had stale counts. It was right, and I fixed three of them.
Two more of the same class surfaced while writing this plan:

### 2.1 CONSISTENCY §67's doctor-block claim (above)

Wrong in a specific, checkable way. Fix: name `memory_redact` and the host
entries, drop the doctor claim, and note the block predates 0.9.0.

### 2.2 The "next free section" line, again

I removed the number from `AGENTS.md` §13 on the grounds that it has been stale
every time anyone checked it (§36, then §50, then §72). **That edit is
correct but incomplete**: `docs/CONSISTENCY.md`'s own last line still carries a
hand-written next-section number (§73 as of this writing, §68 before the last
four sections landed). Same defect, one file over. Fix it the same way — state that the file's own last
section is the answer, and stop writing a number that nothing checks.

**Cost:** two small doc edits, one of which corrects a claim I made in the
previous batch. **Why it is in the plan at all:** the previous review's whole
finding was that this repository's numbers drift, and a plan that fixes them in
one file and not the next is the same mistake with extra steps.

---

## 3. Built but not wired — TODO 31, 36, 38

### 3.1 TODO 31 — the door-parity gaps, after the narrowing

TODO 31 was narrowed on 2026-11-20 and is still open. What remains, and what
each is worth:

| gap | state | cost | value |
|---|---|---|---|
| `hygiene` has no MCP/CLI door | `GET /banks/:id/hygiene` exists (`src/api.rs:1397`) | one route + one tool entry | an agent cannot run its own hygiene scan; it has to shell out |
| `supersede` has no HTTP route | `Store::supersede` exists (3 impls); `grep route(.*supersede src/api.rs` -> none | one route + tests | a caller on the HTTP door cannot supersede, only delete |
| `grounding` has neither | nothing | larger | the one that matters for §4.2's honesty claims |

**Recommendation:** do `supersede` and `hygiene` together — both are one route
plus one surface entry, both are "the door exists and the capability does not",
and both are the kind of asymmetry the door-parity check in the trust gate is
for. **Defer `grounding`**: it is a capability, not a door, and it needs the
§4.2 evidence first to justify the shape.

### 3.2 TODO 36 — the TypeScript domain core

Still open, and the row is honest about why: "the opencode port changed the file
enough that a straight extract is now the wrong first move."

**What changed since that row was written:** the opencode door now has the
capture subscription (§62), which the other TypeScript door does not have in the
same shape. So an extraction now would have to decide whether the shared core
includes the event subscription — which is a real design question, not a
mechanical one.

**Recommendation:** **do not extract.** The duplication TODO 36 measures is 101
lines, and the two doors' remaining differences are exactly the parts that
should differ (opencode has an event stream; the extension has a MV3 service
worker lifetime). Extracting would create a third file that both must import,
for 101 lines, across two runtimes. The `ponytail:` marker convention already
records this kind of ceiling. **Close the row as "deliberately not extracted,
with the reason", not as "done".**

### 3.3 TODO 38 — the other half of the round-trip check

Half done, per the row. The remaining half is a real drive of `claude -p` and
`opencode run`, which the check skips **loudly** because it would rewrite the
operator's live host config.

**Blocked, and I measured why.** `opencode run` has no config-dir flag
(`--help` lists none), so isolating it requires overriding `HOME`, and the
environment guard blocks `HOME=` assignment. A real drive therefore needs
either the user's go-ahead to touch the live config, or a container.

**Recommendation:** record the block with the evidence (the `--help` output, the
guard), and do **not** fake it with a stub. The check's value is that it drives
the real thing; a stubbed drive is the `trust_gate_test.py` mistake again.

### 3.4 What the doctor finding should do about an empty resolved bank

`doctor` already names the bank that holds the memories. It cannot fix it, and
should not — silently moving memories between banks would be the worst kind of
helpful. But it can **tell the user what the fix is**, which it currently does
in prose: "Reach them with `--bank omp`, or set `MEMORY_WIRE_BANK=omp`."

The improvement worth making is small: `doctor` should say whether the resolved
bank is *pinned* (a host config names it) or *derived* (the directory did), and
name the one command that would fix each. `connect --bank` exists; the doctor
currently implies it without saying it.

**Cost:** a doctor block, tests. **Value:** the operator's live machine has this
exact problem and the tool explains it without offering the command.

---

## 4. Measured but not earned — the two remaining evidence gaps

### 4.1 TODO 11 — the LLM-judged arms

AMB `rag` (BEAM/PersonaMem/LifeBench), OAMB LME-60, memorybench ConvoMem,
MemBase. These need `eval/llm_shim.py` and aroute, which `docs/AROUTE_BUGS.md`
documents as slow and defective.

**Recommendation:** none in this plan. The review's own finding applies —
the plan should not promise an LLM-judged run it cannot schedule.

### 4.2 TODO 10 — agent-memory-bench, Claude Code arm

The one external benchmark that grades the *product* (an agent with memory)
rather than the retriever. Still the highest-value evidence gap, and still
unrun.

**Recommendation:** carry it forward as the next evidence item, unchanged, and
do not start it in a session that is also doing §1-§3.

---

## 5. What I am deliberately not putting in this plan

- **A release.** §11, and the user has said release waits for final
  developments.
- **A new retrieval lever.** §6/§8: the queue is empty, `CLOSED.md` is the
  list, and the last review's finding was that the plan's evidence density was
  the problem, not its ambition.
- **A long context-pack review round.** The last one scored 4/10 on evidence
  density and falsified six of nine claims. Running another before §1-§2 land
  would ask a reviewer to review a plan with known-stale numbers in it.
- **Extracting the TypeScript core** (§3.2), with the reason recorded.
- **Faking the real-host drive** (§3.3).

---

## 6. Order of work

| # | item | tier | gates | blocked by |
|---|---|---|---|---|
| 1 | §2.1 + §2.2 doc corrections (CONSISTENCY §67's claim, §68's line) | 1 | docs-check, count gate | nothing |
| 2 | §3.4 doctor names the fix for an empty resolved bank | 1 | tests, clippy, doc, trust gate | nothing |
| 3 | §3.1 `supersede` + `hygiene` doors | 2 | full gate + a door-parity test | #2 for the pattern |
| 4 | §3.2 close TODO 36 as deliberately not extracted, with the reason | 2 | docs-check | nothing |
| 5 | §3.3 record the real-drive block with its evidence | 2 | docs-check | nothing |
| 6 | §4.2 agent-memory-bench, Claude Code arm | 3 | its own protocol | §1-§2 landed, so the doc it cites is true |

**Tier 1 is two commits and under an hour. Tier 2 is the substantive work.
Tier 3 is a research item that should not be started in the same session as
anything above it.**

---

## 7. What I need from the user

1. **The bank decision (§1).** The live machine's hook resolves to an empty
   bank while 4,241 memories sit in `omp`. Fixing it means running the
   installer with `--bank omp`, which rewrites five host configs. I will not do
   that unasked. Everything else in this plan is unaffected.
2. **Release timing (§1.1).** No action proposed; confirming that the "wait
   until final" instruction still holds, since 68 commits have accumulated.
3. **Whether §3.3's real drive is worth a container.** It is the only way to
   close TODO 38's remaining half without touching the live config.