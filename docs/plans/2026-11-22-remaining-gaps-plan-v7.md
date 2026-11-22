# Outline plan — remaining gaps, redundancy, and upgrades (sixth pass)

**Status:** **executed 2026-11-22** (see §11). Written from `d6243f2`, seven commits past `v0.11.0`; item 7 was cancelled after its premise failed verification.

Five previous plans were written and executed on 2026-11-21 and 2026-11-22. This
pass was written by **reading the running product against its own documentation**
rather than by reading the code, which is the method that has produced a new
defect every pass.

**Headline: the tree is clean and every gate is green, but three documents
describe a surface the binary no longer has, and the gate that is supposed to
catch that is blind to the exact shape it happened in.** Nothing here is a code
defect. What follows is seven findings: four documentation corrections (§1-§4),
one shell gate (§5), one duplication decision that is **declined** (§6), and one
carried-forward live-config question that is **the user's call** (§7).

**This is not a "find something to do" pass.** Four of the five open register
rows are budget-gated and stay open; the fifth is in another repo we may not
edit. What follows is what a reader of this project would be misled by *today*.

---

## 0. What is not left

- **Plan v6 is executed** (`57066c9`, `3c08e5f`, `a6e82f7`, `92b2ebe`, `d6243f2`),
  with its record in the plan's §9 and `CONSISTENCY.md` §80. The `update` fix
  landed with two mutation-verified tests.
- **681 tests, clippy 0, doc 0, all three doc gates green**, module table current.
- **`v0.11.0` shipped** with three Linux assets. Seven commits are past the tag;
  none of them is a version bump.
- **No code TODO markers.** There is no `todo!()`, `unimplemented!()`, `FIXME` or
  `HACK` in `src/`; the only `XXX` hits are the literal text `\uXXXX` in two
  `render.rs` doc comments. The `TODO` mentions in `src/` are all references to
  register rows (1, 15, 31, 35, 37, 38 — five of them closed), not open work.
  This pass has no code changes in it by design.

**The five open register rows (10, 11, 12, 13, 38) are all blocked, not pending.**
10 and 11 need the user's agent-model budget. 12 is a recorded deferral with a
measured cost. 13 is in the aroute repo, which AGENTS forbids us from editing.
38's remaining half is blocked by the environment guard (a HOME override) and
must not be faked. None of them is this plan's work.

---

## 1. README's MCP tool table lists five tools; the server serves six

**A reader checking the table against the server gets the wrong answer, and the
sentence directly above the table says the right one.**

README `:663` says "The same six tools are served two ways". The table at
`683-689` has **five** rows. `memory_redact` — added 2026-11-20 by TODO #34, the
credential-retraction verb — has no row.

Verified against the live server, not against the source:

```
$ curl -s -X POST http://127.0.0.1:8888/mcp -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
count: 6
 - memory_retain  - memory_recall  - memory_reflect
 - memory_bank_config_get  - memory_supersede  - memory_redact
```

The binary is right. The prose around the table is right. **The table is the
only thing wrong**, which is exactly the shape a reader trusts most.

**Why nothing caught it:** `scripts/count_claim_check.sh` grades the *number* in
prose ("six tools" → 6 tools ✓) and **skips every table row by design**
(`:53-59`, because INTEGRATION_GAPS' comparison table legitimately mixes
projects). A table that *enumerates* the tools is a different claim from a table
that *compares* projects, and the gate cannot tell them apart. See §5.

---

## 2. INTEGRATION_GAPS says two capabilities have "no route" that both have

`docs/INTEGRATION_GAPS.md:471` (redact) and `:475` (supersede):

| capability | HTTP | MCP | CLI | hook |
|---|---|---|---|---|
| redact (W3) | **no route** | yes (6th tool) | yes | no |
| supersede (W7a) | **no route** | yes (5th tool) | no | no |

Both routes exist, and both were verified **routed** on this host today rather
than inferred from the registration:

```
GET  /banks/omp/hygiene                        200
POST /banks/omp/memories/nope/supersede        404 "no active memory `nope` in bank `omp` to supersede"
POST /banks/omp/memories/nope/redact           415 missing field `pattern`
```

Both non-200 codes are the **domain** answering, not the router: supersede's 404
names the bank and the id, and redact's 415 is the deserializer complaining about
a missing field. A route that did not exist would return the frozen `404` with
neither message.

They were added 2026-11-21 (`34237bf` supersede, `ed96bbd` redact) and recorded
in `CONSISTENCY.md` §74 (supersede) and §75 (redact). **The parity table was never updated**, so the
document whose entire job is to say which doors are asymmetric now understates
the symmetry — the opposite of the failure it was written to prevent.

The table's own header says "Verified per capability against this tree:
**2026-11-19**". That date is the tell: it is a dated addendum, so the fix is
either a correction block or a regenerated table, and the plan should say which.
**Preference: a correction block**, because a dated addendum is a historical
record (AGENTS §7) and rewriting it destroys the record of what was true on
2026-11-19.

`grounding (W5) | no route` (`:472`) is **still correct** and must not be "fixed"
-- there is no grounding HTTP route, verified by grep.

---

## 3. README's route list omits three routes that exist

`README.md:394-396` is the canonical HTTP surface list:

```
`GET /health` · `POST /banks/:id/{retain,recall,reflect}` ·
`GET`/`PUT /banks/:id/config` · `GET /banks/:id/memories[?limit=&offset=]` ·
`GET`/`DELETE /banks/:id/memories/:mid` · `GET /banks/:id/stats`
```

Eleven paths are registered across ten `.route()` calls (`config` is one call
serving `get` and `put`). **Three are missing from the list**: `hygiene`,
`supersede`, `redact`. Each is mentioned elsewhere in the README (the CLI table
at `:449`, the MCP section at `:663-664`) but never in the one place a reader looks
for the HTTP surface.

This is the same underlying fact as §2, one file over — which is the point. The
routes landed through their own items and no document that *enumerates* was
updated, while every document that *mentions* them was.

**Do not "fix" `/banks` (404).** It is not a route and never was; banks are an
implicit namespace, not a listable object. Recorded because probing found it and
the next pass should not re-open it.

---

## 4. Plan v3 says "proposal" and was executed; five plans have no status line

`docs/plans/` held nine files before this one. Status lines:

| file | status line says |
|---|---|
| `2026-11-18-hierarchical-...` | **none** |
| `2026-11-19-execution-plan.md` | **none** |
| `2026-11-20-remaining-gaps-plan.md` | **none** |
| `2026-11-21-remaining-gaps-plan.md` | **none** |
| `2026-11-21-remaining-gaps-plan-v2.md` | **none** |
| `2026-11-21-remaining-gaps-plan-v3.md` | **`proposal`** — and it was executed |
| `2026-11-21-remaining-gaps-plan-v4.md` | EXECUTED |
| `2026-11-22-remaining-gaps-plan-v5.md` | EXECUTED |
| `2026-11-22-remaining-gaps-plan-v6.md` | executed |
| `2026-11-22-remaining-gaps-plan-v7.md` | proposal (this file) |

**v3 is the real defect, not the five without a line.** v4's own body says
"Two previous plans were executed in the same day (`...-plan-v2.md`,
`...-plan-v3.md`)" — so the tree already knows v3 landed, and v3's header still
asks to be approved. A reader who opens v3 cold concludes it is pending work.

This is v5's shape again (`bcf7f56`, `CONSISTENCY.md` §78): **two places
disagree and the register is the one that is stale.** Fix v3's line; leave the
five unlabelled files alone unless
they are also misdescribed (they are not — they simply predate the convention,
and inventing statuses for them is churn, not cleanup).

---

## 5. The gate cannot see the failure in §1 — and that is the item with code in it

`scripts/count_claim_check.sh` is a good gate with one blind spot, and §1 is
what it looks like.

```sh
# Skip a table row: in INTEGRATION_GAPS' comparison table each cell is a
# different project, so "54 tools" there is agentmemory's count and
# correct. A table row is not a claim about this project.
case "$line" in
  *'|'*) continue ;;
esac
```

The comment's reasoning is sound for *comparison* tables. It is wrong for
**enumeration** tables — a table whose rows are `| `memory_retain` | yes | ... |`
is a list of our own tools, and dropping a row from it is exactly the drift the
gate exists to catch.

**The proposal:** teach the gate the difference, in the narrowest way that does
not resurrect the `54` and `4` false positives the comment records. A table row
is an *enumeration of our surface* when its first cell is a backticked
`memory_*` name. Such a row must name a tool the binary actually serves; a
`memory_*` row naming something `src/mcp.rs` does not register is a finding.

**Direction matters and must be decided, not guessed:**

- *Every tool has a row* (catches §1). This is the half that fixes the observed
  defect.
- *Every `memory_*` row names a real tool* (catches a row for a tool that was
  removed). Cheap, and the same parse.

Both are one pass over README's table against `Server::tools()`. Neither
reintroduces the competitor-count problem, because a competitor's tool count
never appears as a backticked `memory_*` row.

**Do not** write a test that greps README from `src/` — a Rust test asserting on
prose in another file is the shape the count gate was invented to replace. This
belongs in the shell gate, beside the two that already do this job.

**Mutation battery required**, as with `citation_name_check.sh`: deleting the
`memory_redact` row must fail; adding a `memory_nope` row must fail; the real
tree must pass. Three cases, all cheap. A gate that has never been shown to
fail is a comment.

---

## 6. Redundancy and cleanup: one real duplication, and why it is not worth it

`entry_url` is defined identically in `src/connect_claude.rs:171` and
`src/connect_copilot.rs:190`:

```rust
fn entry_url(entry: &Value) -> Option<String> {
    entry.get("url")?.as_str().map(String::from)
}
```

`prune_empty_dirs` is near-duplicated in `connect_ext.rs:241` and
`connect_opencode.rs:252` — **same intent, different stop condition**
(`extensions` vs `plugins`) and different loop shape.

**Recommendation: consolidate `entry_url`, leave `prune_empty_dirs`.** The
first is a one-line accessor with no policy in it; a second copy is pure noise.
The second has *policy* in it (which directory name ends the walk) and the two
copies encode different policies deliberately. Merging them means inventing a
parameter for a difference that is the whole content of the function — and the
modules are per-host on purpose (`connect_*`), so a shared helper would be the
only cross-host dependency in a file family that deliberately has none.

Recorded as **considered and declined**, so the next pass does not re-propose it.
That is a result, not a deferral.

---

## 7. The `omp` live config's `http` entry — **verified LIVE and working, kept**

Not a defect. Carried from v6 §2, where the entry was recorded as one omp
"ignores". **That premise was checked on 2026-11-22 and it is wrong**, which is
the whole value of re-verifying a carried-forward claim rather than inheriting
it:

- `memory-wire` **is** in `enabledServers` — it is not a stray key.
- `POST /mcp/omp` answers with **six tools**, verified live. The URL pins the
  `omp` bank, so this is the door omp uses and it works.
- bank `omp` holds **4250** memories, reachable only through this door.

So the entry is **load-bearing, not dead weight**, and it is **not removed**.
Removing it would take memory away from omp, which is the exact opposite of
what this section says it was doing.

**The shape is deliberate, and the code already protects it.** `connect` writes
`stdio`; this entry is `http`. It was written by hand (or by an earlier config
pass), not by `connect`, and `is_ours()` is false for it — a URL carrying the
bank id `omp` does not contain the literal `memory-wire`. So a future bare
`memory-wire connect omp` **refuses** and names both shapes, rather than
destroying a working door. That is the v6 refusal-message fix doing its job on
the live config it was written for.

**Nothing to do, then.** The section stays in the plan as a verification record,
not an action: the claim was wrong, the check was cheap, and the conclusion is
*keep it*. Re-verify after any `connect` run, since `connect` would refuse
rather than overwrite — but a *forced* rewrite would still change the shape.

**A note for the future:** if omp ever needs memory-wire through `connect`, the
resolution is a `stdio` entry (what `connect` writes) or an explicit hand-edit
that keeps whatever the operator settled on. Both are decisions, not cleanup.

---

## 8. What this plan deliberately does not do

- **No code behaviour changes.** There is no code defect here. §5 is a gate
  (shell), and §1-§4, §7 are docs and one live config.
- **No version bump, no tag, no release.** v0.11.0 is current and §1-§4 are
  documentation. Cutting a release for a README table is not a thing.
- **Does not open TODO 10/11/12/13/38.** All five are blocked for reasons
  recorded in their own rows; re-opening one without its unblocking condition is
  the "phantom work" failure AGENTS §13 warns about.
- **Does not fake TODO 38's remaining half.** Its block is the environment
  guard and the live-config hazard; §38 already says "Do not fake it" and that
  stands.
- **Does not rewrite dated addenda.** §2's fix is a correction block, not an
  edit to the 2026-11-19 table (AGENTS §7).

---

## 9. Definition of done

- `README.md`'s MCP table has one row per served tool, and the row for
  `memory_redact` names its arguments and its `Writes?` value as **yes**.
- `README.md`'s route list names `hygiene`, `supersede` and `redact`, or the
  list says explicitly that it is the core surface and names where the rest are.
- `docs/INTEGRATION_GAPS.md` carries a correction block (dated, appended) saying
  redact and supersede now have HTTP routes, with `grounding` explicitly still
  without one.
- `docs/plans/2026-11-21-remaining-gaps-plan-v3.md` says **executed**, not
  `proposal`.
- `scripts/count_claim_check.sh` fails when a `memory_*` enumeration row names
  a tool the binary does not serve, and when a served tool has no row — with a
  three-case mutation battery proving both directions, and the real tree passing.
- All gates green; `CONSISTENCY.md` gets the section.

## 10. What I need from you

1. **§7: the `omp` live config.** One line, live file, yours to remove or keep.
2. **v0.11.1 for v6's §1/§4?** Still open from the last plan and now seven
   commits stale. `update` is broken in the field and the fix is unreleased.
   §1-§4 here do not change that answer either way.

---

## 11. Execution record — items 1-5 done, 6 done, 7 cancelled, 2026-11-22

| § | item | commit | outcome |
|---|---|---|---|
| 1 | README tool table | `5c48e96` | six rows; `memory_redact` added |
| 2 | INTEGRATION_GAPS | `5c48e96` | dated correction block appended, 2026-11-19 table untouched |
| 3 | README route list | `5c48e96` | all three routes named, plus an explicit "that is every route" |
| 4 | plan v3 status | `5c48e96` | `proposal` → **EXECUTED**, with the false v4 claim recorded |
| 5 | the gate | `5c48e96` | `scripts/tool_table_check.sh`, wired into pre-commit, battery 4/4 |
| 6 | redundancy | — | **declined** as specified: `entry_url` noted, `prune_empty_dirs` declined by reason |
| 7 | omp live config | `5c48e96` (plan text) | **CANCELLED — premise false.** See below. |

**§7 is the finding worth carrying forward.** The plan said to delete the
`memory-wire` entry from omp's `mcpServers`. Checked before deleting, it is
load-bearing: `memory-wire` is in `enabledServers`, `POST /mcp/omp` serves six
tools live, and bank `omp` holds 4250 memories reachable **only** through that
door. Deleting it would have taken omp's memory away. The plan's §7 is
corrected in place and the entry is **kept**.

**Two things the plan did not predict.**

1. **A second defect in the README table, caller-visible.** The
   `memory_supersede` row documented `memory_id`; the schema says `id`. A client
   following the table got an error. Found only by reading the live schema
   while writing the replacement row — and the gate as specified would not have
   caught it, so a third assertion (required args per row) was added.
2. **`cargo test --offline` broke mid-session** on `base64`/`winapi` missing from
   the local cache. `cargo fetch --locked` fixed it. Recorded in §81.

**DoD:** README table has one row per served tool ✓; route list names all three ✓;
INTEGRATION_GAPS carries a dated correction ✓; v3 says executed ✓; the gate
fails on both directions with a 4-case battery ✓; gates green ✓; CONSISTENCY §81 ✓.

**What was NOT done, by decision:** the omp entry (kept, not removed), and the
v0.11.1 cut (the user asked for a build, which is the next step).
