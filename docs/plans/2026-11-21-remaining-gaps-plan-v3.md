# Outline plan: remaining gaps, cleanup, and upgrades — measured, with the work log

**Status:** **EXECUTED 2026-11-21** (second pass). Written from `790987d`;
all its items landed the same day. Verified 2026-11-22: the successor's header
(`...-plan-v4.md`) said "both now carry supersession headers" — **neither did**,
and this line was the correction.

Two questions were asked alongside this: *is a benchmark re-run really all that's
left*, and *what did the ~70 commits actually accomplish*. §1 answers the first,
§2 the second, and §3 onward is the plan. I went looking for gaps rather than
assuming the register is empty, and found a small real class.

---

## 0. The short answer to "is a benchmark re-run all that's left?"

**No — and it is the *least* useful thing left to do today.** Three reasons, each
checkable:

1. **The last benchmark change cannot move a ranking number.** §74 ran LoCoMo
   (R@10 94.2%, NDCG@10 73.6%, MRR 71.8%) and the relevance battery (H8 gate
   held, 41% less auto-injected) after the changes. LongMemEval-S would burn one
   of five counted consultations to confirm that doors and a doctor line do not
   reorder results. That is what §6 of AGENTS exists to prevent.
2. **Half the battery is blocked on external state, not on us.** `../_audit/amb`
   is gone, so the free PrecisionMemBench run cannot execute without a re-clone
   and a re-patch. agent-memory-bench needs an answering model plus Docker, which
   is your budget.
3. **The largest live defect is operational, not measured.** The installed binary
   is 0.9.0 and 82 commits behind; this host's hooks resolve to a bank holding
   nothing while `omp` holds 4,241 memories. No benchmark will tell you that.

What *is* worth running, and what it costs, is in §5.

---

## 1. The work log: what the ~119 commits actually accomplished

119 commits since `v0.8.2` (2026-10-16); 82 since `v0.10.0` (2026-10-27). Diff
against `v0.10.0`: **159 files, +36,278 / -2,627**.

**The headline property held.** `DT_NEEDED` is still **3**
(`libgcc_s`, `libm`, `libc`) and the binary is **10.65 MiB** against the v0.7.0
baseline of 10.25 MiB — a 0.4 MiB move over 82 commits, with no dependency added.
The competitive position in AGENTS §1 is intact, which was the risk in touching
anything that links.

### 1.1 The W0–W7 capability build (the bulk of the log)

| phase | what landed | commit |
|---|---|---|
| W0 | hermetic `golden_install_shapes`, redactor identifier tests | `a45ea07` |
| W1 | **dimensional schema** — scope/aspect/lifecycle/trust/parent_id/superseded_by/scope_globs | `e10ba0b` |
| W2 | mount profiles: `readOnly` + `maxTrust` on `BankConfig` | `8eaeac3` |
| W3 | retroactive redaction with a version audit trail | `052f3a6` |
| W4 | deterministic hygiene scan (`hygiene.rs`, 658 lines) | `ad8472a` |
| W5 | startup grounding capsule (`grounding.rs`, 350 lines) | `4540b19` |
| W6 | `connect --with-skills` + embedded `SKILL.md` (`skills.rs`, 282 lines) | `4793326` |
| W7a | `supersede` as the 5th MCP tool | `d34a0a5` |
| W7b | decision doc: no raw score gate on recall_section | `eb95a0a` |

Then the surface-audit follow-ups: `redact`/`grounding`/`hygiene` wired to real
surfaces (`37c39b5`), `readOnly` guarding every write (`af3a0a5`), occurrence
dates on three doors that dropped them (`0b518de`), the capsule knob reaching
the plugin doors (`1f93bf0`).

### 1.2 Three rounds of external review, and what each caught

This is the part worth reading, because the defects found were in our own work.

- **Round 2** (context-pack): the structural finding that "one service, three
  doors" is true of `MemoryService` and **false of parity** — every door exposes
  a different subset. Recorded in `INTEGRATION_GAPS.md`'s 2026-11-19 addendum.
- **Round 4**: the TypeScript cap counted *input* rather than output
  (`ed8eea4`), and the plugin comments pointed at a rejected lever (`518903d`).
- **Round 5**: two real defects in the previous session's own code — opencode
  cached an empty block across a failed recall (`5285ea3`), and the "and N more"
  footer understated the dropped count.
- **The rulings round**: it audited a plan rather than code, and found **four
  errors in my own work** (`333185b`), including that I had reflowed a comment to
  satisfy a byte-comparing fence. That fence is now a behavioural comparison over
  7 shared fixtures.
- **The plan review** (2026-11-21): scored the plan 4/10 on evidence density and
  falsified six of nine claims. Its three strongest findings were all about my
  own work, and all were fixed — the round-trip test had the same blind spot it
  accused TODO 35 of having (`b25a56b`), AGENTS.md's own counts were stale in
  three places (`fca968c`), and the plan said "stub server" when the built test
  uses a real one.

### 1.3 The real defect found by accident

`3376b5c` — **"The opencode door was silently dead: port it to the v2 plugin
API."** The door had never loaded on its named host. This was found by accident,
not by a check, and it is the origin of the whole verification push.

### 1.4 Today's batch (full-scale execution)

`d86d200` plan · `6df26dd` doctor names the input behind an empty bank ·
`34237bf` supersede got its HTTP route and hygiene its CLI · `59a2919` TODO 36
closed as a decision and TODO 38's block recorded · `b7bee48` CONSISTENCY §74 ·
`790987d` STATE.

**Net of the 119:** a capability build (W0–W7), three review rounds that each
found defects in our own work, a verification layer that now runs, and a
footprint that did not move.

---

## 2. What is genuinely still open

Six TODO rows say `open`. Four are blocked on you or the world:

| row | what | why it is open |
|---|---|---|
| 10 | agent-memory-bench, Claude Code arm | not cloned; needs an answering model + Docker (your budget) |
| 11 | LLM-judged arms (AMB `rag`, OAMB, memorybench, MemBase) | needs `aroute`, which `AROUTE_BUGS.md` documents as defective |
| 12 | AMB provider writes at product granularity | depends on 10/11 |
| 13 | aroute: 200-with-`{"error"}` logged `outcome=ok` | **not our repo** — a handoff to aroute |
| 31 | door parity | narrowed twice; see §4 |
| 38 | cross-door parity check | half done; real-drive half blocked on a container |

**Nothing here is a feature gap.** The remaining work is evidence, one aroute
bug we cannot fix, and two verification halves.

---

## 3. The gaps I found by looking (this is the new material)

### 3.1 A live stale count in source — `src/mcp_http.rs` says "four tools"

Measured. `mcp_http.rs` serves `Server::tools()` — the same **6** tools as
stdio — but says "four" in three places:

| line | text | reality |
|---|---|---|
| 3 | `one set of four tools, a second transport` | 6 |
| 86 | `gets its four tools answered` | 6 |
| 249-250 | `the four tools, a retain and a recall` | 6 |

This is not a historical doc. `docs/BENCHMARK.md`'s "4 tools" is a dated 0.3.0
snapshot and correctly stays; `INTEGRATION_GAPS.md` and `INTEGRATION_AUDIT.md`
are dated audits with explicit status lines and correctly stay. `mcp_http.rs` is
**live source that a reader would believe**, and it is wrong.

**Why it matters beyond tidiness:** this is the same class as the counts I fixed
in AGENTS.md on 2026-11-21, and the count gate was built for exactly it — but
the gate only checks AGENTS.md and the module table. The class is wider than the
gate.

### 3.2 `INTEGRATION_GAPS.md:53` claims "all 4" annotated

```
| MCP annotations | **zero** on 54 tools | all 75 registrations | all 4 (`src/mcp.rs:80-87`) |
```

The code has **6** annotated tools, and `tools_list_should_show_six_annotated_tools`
pins the set. The line's *point* — that our tools are annotated while
agentmemory's 54 are not — is true and worth keeping; the number is stale.

### 3.3 Two same-day plans with no supersession marker

`docs/plans/2026-11-21-remaining-gaps-plan.md` (v1) and `...-plan-v2.md` both
exist. v2 names v1 as its predecessor, but **v1 does not say it is superseded**.
A reader who opens v1 first gets a plan whose Tier 1 already landed and whose
"nothing verifies the round trip" headline was corrected by the review that read
it. Same class as CONSISTENCY's stale section number, which I removed rather than
kept updating.

### 3.4 Why the gate did not catch either one (this is the useful part)

`scripts/count_claim_check.sh` **already scans `docs/INTEGRATION_GAPS.md`** and
would have caught §3.2. It does not, for a documented and *correct* reason:

```sh
# Skip a table row: in INTEGRATION_GAPS' comparison table each cell is a
# different project, so "54 tools" there is agentmemory's count and correct.
case "$line" in *'|'*) continue ;; esac
```

The rule is right for the `54 tools` cell and wrong for the `all 4` cell in the
*same row*, because that cell is ours. The gate cannot tell the two apart
without knowing which column is ours.

And the gate never looks at `src/` doc-comments at all, which is where §3.1
lives — so `mcp_http.rs` is invisible to it by construction.

**Recommendation, narrowed after reading the gate:**

1. **`src/` doc-comments (new).** Scan for `N tools` / `N hosts` / `N modules`
   against the code, the same way the gate already derives `mcp_tools` from
   `src/mcp.rs`. This is the class that actually recurs.
2. **The table-row rule (leave it).** Do not "fix" it. The right fix for §3.2 is
   to reword the cell so it names the project — `all 6 (ours)` — which both
   reads better and stops the cell from being an unattributed claim.
3. **Do not** try to cover prose generally. That is how a gate becomes someone's
   job.

---

## 4. TODO 31 — what actually remains, precisely

The row has been narrowed twice and is still open. Measured today:

| surface | hygiene | redact | supersede | grounding |
|---|---|---|---|---|
| HTTP | ✅ `GET /banks/:id/hygiene` | ❌ none | ✅ `POST .../supersede` | ❌ none |
| CLI | ✅ `memory-wire hygiene` | ✅ `memory-wire redact` | ❌ none | ✅ `memory-wire grounding` |
| MCP | ❌ none | ✅ `memory_redact` (6th tool) | ✅ `memory_supersede` (5th) | ❌ none |

**Two gaps remain, and they are not symmetric:**

- **hygiene has no MCP tool.** An agent on the MCP door cannot run its own
  hygiene scan; it must shell out. Cost: one tool + schema + the 6→7 count
  updates (test, AGENTS, README, mcp_http docs).
- **redact has no HTTP route.** An agent on the HTTP door cannot redact; it has
  MCP and CLI. Cost: one route + the frozen-contract care that goes with a
  write.

**Recommendation:** do **redact's HTTP route** and leave hygiene's MCP tool.
Redact is the capability an agent most often needs mid-session (it just retained
a secret), and the CLI/MCP pair already proves the handler works. Hygiene is a
maintenance scan a human runs. Both would be defensible; one is more valuable.

---

## 5. What to run, and what it costs

**Free and worth it now:**

| run | cost | why |
|---|---|---|
| `cargo test/clippy/doc` gate | ~1 min | after any change |
| LoCoMo dev | ~25 s | the dev set; selection happens here |
| relevance battery | ~22 s | auto-injection junk/gold |

**Not free, and blocked:**

| run | blocker |
|---|---|
| agent-memory-bench (row 7) | not cloned; answering model + Docker = your budget |
| AMB / PrecisionMemBench (rows 4, 8) | `../_audit/amb` gone; needs re-clone + re-patch |
| OAMB / MemBase / memorybench (9, 10) | paid model; OAMB and MemBase also burn the test set |
| LongMemEval-S (row 5) | runnable, but **deliberately deferred** — see §0 |

**On the LongMemEval question specifically:** the dataset is present and the
command works. I would run it at a release boundary, not after a doors-and-docs
batch. If you want it now as a regression check, say so and I will run it once
and report whatever it shows — but it will cost one of five consultations to
confirm what the two free harnesses already show.

---

## 6. Order of work

| # | item | tier | gates | blocked by |
|---|---|---|---|---|
| 1 | §3.1 + §3.2 fix the live stale counts (`mcp_http.rs`, `INTEGRATION_GAPS:53`) | 1 | doc, tests, count gate | nothing |
| 2 | §3.3 mark v1 superseded by v2 | 1 | docs-check | nothing |
| 3 | §3.4 extend the count gate to `src/` doc-comments | 2 | its own mutation battery | #1 |
| 4 | §4 redact HTTP route (and decide hygiene MCP explicitly) | 2 | full gate + frozen-contract test | #1 |
| 5 | §5 run the free battery and record it | 3 | loadavg recorded | nothing |

**Tier 1 is one commit and under twenty minutes.** Tier 2 is the substantive
work. Tier 3 is measurement that should follow, not precede, #1–#4.

---

## 7. What I need from you

1. **The release decision.** The installed binary is 0.9.0, 82 commits behind,
   and lacks `memory_redact`. Cutting one is `scripts/build-release.sh` with the
   three-Linux set; it is your call under AGENTS §11.
2. **The bank decision.** This host resolves to `ishan-parihar-memory-wire`
   (empty) while `omp` holds 4,241. `connect --bank omp` fixes it and rewrites
   five live host configs. Not run.
3. **Whether to spend an agent-model budget on agent-memory-bench.** It is the
   only external benchmark that grades the product's actual job.
4. **Whether LongMemEval is wanted now as a regression check**, knowing it burns
   a consultation to confirm what the free harnesses already show.