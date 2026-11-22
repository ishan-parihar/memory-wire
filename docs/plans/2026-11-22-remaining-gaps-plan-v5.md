# Outline plan — remaining implementation gaps, redundancy, and upgrades

**Status:** **EXECUTED 2026-11-22** (fourth pass). Written from `46ceba3`; all
three items landed the same day.

Three previous plans ran the same day (`...-plan-v2.md`, `...-plan-v3.md`,
`...-plan-v4.md`), all executed. This pass was written by looking for defects
rather than re-reading the register, which is the only method that has produced
findings so far — and it found one immediately, in my own work from an hour
earlier.

**Headline: one real defect (mine), one real redundancy with a false
supersession, and one register pair that disagrees about what is open.** Plus
two operator decisions that have been open since the last pass and are not mine
to make.

> **Execution record.** §1 landed as `46ceba3` (the entry-point fix, the
> conditional dead-pid advice, the store guard, and a mutation-checked test
> through `collect_at_with_state`). §2 landed as `bcf7f56` (both plan files
> marked superseded, the self-referential header removed, `STATE.md` given a
> staleness header, and its generated status table corrected with `genome emit`
> to open 6 / done 31). §3 landed as `d7e2ef4` (the note corrected, the two
> contradicting notes merged into one). Recorded in `CONSISTENCY.md` §78.
>
> **One thing the plan got wrong, corrected during execution:** it said
> `STATE.md`'s status table was hand-written and needed hand-editing. The table
> sits inside a `genome:zone` and is generated. Hand-editing it would have been
> overwritten on the next `genome emit`. `genome emit` produced the correct
> numbers on its own.
>
> **And one the plan missed:** `STATE.md` was not merely stale in its table. It
> was titled 2026-10-16, described v0.8.2, and was **133 commits** behind the
> `c919f13` it named as HEAD, while having been modified that day.

---

## 0. What is not left

Recorded so the next pass does not repeat it:

- **Plan v4 Tier 1 is closed.** All four items landed (`9b2782d`, `19d265f`,
  `5950ddf`, `0aaefb5`).
- **The retrieval hypothesis register is resolved.** `RND_PLAN.md` H1.0 is done,
  H1a–H1d are measured with gates recorded, H1 is rejected. No open H-item.
- **The capability surface is complete.** 6 MCP tools, 8 hosts, 7 implicit, and
  the count gate now covers `src/` as well as prose.
- **No unused dependencies.** Checked: `tracing-subscriber` looked unused under a
  `grep 'crate::'` and is not — it is called as `tracing_subscriber::fmt()`
  (`src/main.rs:320`). Recorded so the next pass does not re-file it.
- **Footprint holds.** 10.73 MiB, `DT_NEEDED` 3, matching AGENTS §1 exactly.

---

## 1. The defect: a fix that passed its test and changed nothing

**This is the one item with code in it, and it is a correction to `9b2782d`.**

The doctor reported `server up` while `daemon status` reported `not running:
serve.json points at pid 1371296`. I fixed it on 2026-11-21 by adding a dead-pid
branch to `stale_serve_at` and calling it from `collect`. The unit test passed.
**The CLI printed no finding anyway.**

`doctor::run` — the function the `doctor` subcommand actually calls — reaches
`collect_at`, not `collect`. And `collect_at` set `stale_serve: None`
unconditionally. So the check ran only on a path nothing in the product takes.
Two functions computed the same report and only the unreachable one was fixed.

Three things came out of chasing it:

1. **`collect_at` is now the single site**, `collect` delegates to it. One place
   to be wrong instead of two.
2. **The regression test went through the entry point.** The first version called
   `stale_serve_at` directly and stayed green when `collect_at` was reverted to
   `stale_serve: None` — I reproduced the exact mistake the test existed to
   prevent. Caught by mutating the fix back and watching 24 tests still pass.
   The serve-state path is now a parameter (`collect_at_with_state`) so a test
   can name it.
3. **A dead pid gets different advice from a stale build.** The shared trailing
   sentence said "the old process keeps serving the old bytes", which is nonsense
   when the pid is gone: nothing is holding that port.

`collect_at` also now guards on the state file naming *the store it was asked
about*. Without that, reporting on any store read the ambient daemon's serve
record, and two doctor tests began failing on this host for a reason unrelated
to the code they test.

**Landed:** `46ceba3` (the fix and the module-table refresh, one commit). Mutation-checked.
Verified against the real binary: the finding now prints.

---

## 2. A redundancy: three registers, one of them unmarked

`docs/handoff/` is the pointer AGENTS §4 sends a fresh agent to. Inside it:

| file | lines | what it says it is |
|---|---|---|
| `STATE.md` | 560 | the status table: open 4, in-progress 1, done 24 |
| `REMAINING_WORK_PLAN.md` | 272 | a live list — **no supersession marker** |
| `RND_PLAN.md` | — | the hypothesis register, resolved |

`REMAINING_WORK_PLAN.md` opens by saying it "supersedes that file's §4/§5 as the
live list", but nothing supersedes *it*, and `docs/plans/` does. Two plans carry
headers because I added them; this one was missed. A reader who opens it has no
way to know it is stale, which is the same failure as the missing §-number the
repo has already paid for.

**The fix is a header, not a deletion.** It is a dated record and AGENTS §7 says
those stay with their dates.

**There is also a real disagreement, not just a missing marker.** `TODO.md`
carries 6 open rows (10, 11, 12, 13, 31, 38). `STATE.md` carries 4 (10, 11, 12,
13) — rows **31 and 38 are absent from it entirely**, and its status table says
"open 4". Both cannot be the register. `TODO.md` is the implementation register
per its own header and per `REMAINING_WORK_PLAN.md`'s opening line, so STATE's
table is the wrong one.

**The fix:** add the supersession header to `REMAINING_WORK_PLAN.md`, correct
STATE.md's table to match `TODO.md`, and say in STATE which file is the register
so the next agent does not have to infer it.

---

## 3. An artifact that describes work that never happened

`research/audits/context-pack-2026-11-21-rulings/` holds a 96,978-byte pack
posted to `gemini-4-hydrogen` on page 372 at 06:06:16 (91,464 chars in). Its
`notes/RESPONSE-NOT-COLLECTED.md` says the reply was "transcribed to
`RESPONSE.md` and verified in `VERIFICATION.md`". **Neither file exists.**

The note is right that the round did not complete through the tool — another
session was driving the same browser and recycled the page. But the second
paragraph describes artifacts that were never written, and a reader who trusts
it will look for a verification that is not there.

**Mitigating fact, and it matters:** the pack's subject (`9a61762`) is the
plan whose Tier 1 *did* land (`dc97c40`, `3050ab2`, `303a096`). So the lost
reply cost nothing — the work was executed on its own evidence. That is luck,
not design.

**The fix:** correct the note to say what is true (posted, reply lost, no
transcript exists) and record that the plan it reviewed was executed anyway.
Cheap, and it is the difference between a record and a false one.

---

## 4. What I checked and found clean

So the next pass does not repeat it:

- **The capability surface is honest.** `hygiene` and `supersede` both work from
  the CLI and HTTP respectively, matching TODO 31's claim.
- **Both existing doc gates pass** (`module_table_check`, `count_claim_check`),
  and the new `citation_name_check` is green.
- **Footprint and `DT_NEEDED`** match AGENTS §1 exactly.
- **No unused declared dependencies** (§0 records the one that looked unused).
- **The hypothesis register has no open item.**

---

## 5. Order of work

| # | item | tier | gates | notes |
|---|---|---|---|---|
| 1 | §2 supersession header + STATE/TODO reconciliation | 1 | docs-check | additive, nothing deleted |
| 2 | §3 correct the false "transcribed and verified" note | 1 | docs-check | a record must not claim work that did not happen |
| 3 | §1 — **done**, `46ceba3` | — | — | kept here so the pass is self-contained |

**Two commits, both documentation.** The code work this pass produced is §1 and
it is already in.

---

## 6. What I need from you

Unchanged from plan v4, and still the only real blockers:

1. **Whether to cut a release.** 97 commits past `v0.10.0`, all gates green.
   AGENTS §11 reserves this to you.
2. **The bank decision.** `connect --bank omp` would move hooks off the empty
   `ishan-parihar-memory-wire` onto the bank with 4,249 memories. It rewrites
   five live host configs.
3. **Whether `agent-memory-bench` is worth a model budget.** Still the only
   external benchmark that grades the product's actual job.

Hydrogen's ruling was to do these in that order, and the reasoning holds: each
one gets cheaper once a tag exists.

**One more, now sharper.** The running server on this host is
`~/.local/bin/memory-wire` **0.9.0**, holding 8888 against a `serve.json` naming
a dead pid. The fix in §1 only helps once that binary is replaced — the installed
one predates it. `memory-wire update` exists and is the intended path; I have not
run it, because it stops and restarts a live server.

---

## 7. Definition of done

- `REMAINING_WORK_PLAN.md` names what superseded it and points at `docs/plans/`.
- `STATE.md`'s status table agrees with `TODO.md`, and says which file is the
  implementation register.
- The rulings note no longer claims a `RESPONSE.md` that does not exist.
- All three doc gates green; no source change, so no test gate needed.
- `docs/CONSISTENCY.md` gets the section, including §1's correction of `9b2782d`.