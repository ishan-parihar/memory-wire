# Outline plan — execution order for what is actually left

Written 2026-11-19, after round 4 and its two follow-up commits (`ed8eea4`,
`518903d`). It supersedes `REMAINING_WORK_PLAN.md` as the live ordering; that
file stays as the dated record of the decisions block, and `TODO.md` rows
10–13, 31 stay the implementation register.

**Every row below was checked against the tree, not inferred from a register.**
Where a claim turned out false it says so. Where a register was stale I fixed the
register rather than planning around it.

---

## 0. What this plan is not

It is not a retrieval plan. Per AGENTS §6 and §8, and re-confirmed by the
six-repo audit (`COMPETITOR_GAPS_PLAN.md` §6), there is **no open ranking
lever**: every in-house lever is terminal (`docs/handoff/CLOSED.md`), no incoming
mechanism survived the competitor screen, and the shipped configuration survived
every challenger. Nothing here touches `recall.rs`'s scoring, no parameter is
swept, and no LongMemEval consultation is spent (§34/§35: 5 of 6 spent, the sixth
reserved for release acceptance).

So the remaining work is of three kinds only: **correctness of what shipped**,
**redundancy that has a measured shape**, and **operational surface that is
asymmetric**. The first is what a bug fix is. The second and third are what this
plan is mostly about.

---

## 1. Tier A — do now, no decision needed

These are inside the repo, inside the law, and each has a proof that can be run
before committing.

### A1. The plugin door test is wired to nothing

**This is the highest-value item in the plan and it is small.**

`plugin/integrations/trust_gate_test.sh` exists, runs all three plugin doors, and
is the cited proof for the trust gate (CONSISTENCY §53/§55/§58). I grepped for
its invocation: it is named in `docs/INTEGRATION_GAPS.md`, `STATE.md` and four
CONSISTENCY sections, and in **no** hook, workflow, or script. `.githooks/pre-commit`
runs the secret gates and delegates to the docs gate; `ci.yml` runs `cargo test`
and `cargo clippy`. Nothing runs it.

That matters because it is the only executable check on two of the four doors,
and the two doors it covers are the ones with no compiler and no type system.
Round 3 found the trust gate missing from both; round 4 found a starvation bug
in both; each was caught by a human deciding to run a script by hand. The gate
that would have caught them automatically is a file nobody runs.

**Scope:** add it to `.githooks/pre-commit` behind a `command -v node` guard, so
a clone without node still commits, and add a `Run plugin door tests` step to
`ci.yml` with the same guard. One `if ! command -v node` in two places.

**Proof:** delete the `isUntrusted` line from one plugin, run the hook path, and
observe it refuse. Then restore. This is the same mutation check that was run
per door in round 4 — it now needs to run without me deciding to run it.

**Cost:** ~10 lines across two files. **Risk:** a clone without node silently
skips, which is why the guard prints a line rather than being silent.

### A2. The genome lint is red, and has been for a while

`./scripts/genome lint` reports `E003 register 4: terminal without → [YYYY-MM-DD]
<evidence>`. Row 4 is `done` and its evidence cell still reads as an instruction
to a future agent ("re-verify each item against the `v0.7.0` tree first"). 27 of
the 28 `done` rows carry the `→ [date] <evidence>` grammar; row 4 is the one that
does not, because it was corrected in place on 2026-11-19 without moving the
evidence column.

**Scope:** rewrite row 4's evidence cell into the house grammar, citing the
correction itself (batch 1, CONSISTENCY §54). No claim changes; the cell stops
being an instruction and becomes a record.

**Proof:** `genome lint` exits 0.

**Cost:** one table row.

### A3. `RND_PLAN.md` §6 is titled "the upcoming cycle" for a cycle that closed

§6 is headed "Cycle 2026-10-16→ — the upcoming cycle, outlined" and §6.0 still
lists W1–W3 as pending work. All three are recorded done: W1 in CONSISTENCY §47,
W2 in §48, W3 in §46. The header also says "Status 2026-10-14" on a document
whose §6 was written 2026-10-16.

**Scope:** retitle §6 to the cycle that ran, and mark W1–W3 with their closing
sections instead of leaving them in the imperative. Same stale-register shape as
A2, same class of fix.

**Proof:** a reader reaching §6.0 finds three closed items with pointers, not
three open ones.

### A4. `docs/BENCHMARK.md` §1 is a historical record and says so

Verified while checking a "4 tools" hit that looked like live drift:
`docs/BENCHMARK.md` opens with a blockquote declaring §1–§4 describe `0.1.0` and
are "false of the current tree". Its "4 tools" and "eight subcommands" lines are
inside that fence. **This is correct as-is and needs no change.** It is in this
plan only so the next agent who greps for a tool count does not "fix" a dated
record. AGENTS §7: when a doc and an artifact disagree, the artifact wins,
except a historical record that describes a dated build.

---

## 2. Tier B — redundancy with a measured shape

The round-4 review inferred duplication from line counts. I measured instead of
acting on the inference, and the measurement changes the answer for one of its
two claims.

### B1. `connect_*` — measured, and the refactor is NOT justified

The review said 6,609 lines across 7 files (18.7% of `src/`) and called it
un-audited duplication risk. Measured by the line of `mod tests` (CONSISTENCY
§59):

| | code | tests |
|---|---|---|
| 7 `connect_*` files | 3,217 | 3,392 |
| ratio | 49% | 51% |

A `connect_*` adapter's real job is writing one host-specific config file and
verifying the write, which is why tests outnumber code 1.8:1 in
`connect_ext.rs` (292 / 538). The shared discipline is already one helper —
`paths::write_atomic`, called at `connect.rs:1088`, `connect_codex.rs:188` and
`:421`, `connect_ext.rs:163` — not six copies.

**Verdict: close it.** A "shared config abstraction" would collapse ~3,200 lines
of which half are the per-host assertions that make each adapter safe. Recorded
so the next agent does not re-derive it from the line count.

### B2. TS plugin duplication — real, but extraction is blocked on a fact I cannot settle from here

Measured precisely: 146 code lines in `extension/memory-wire.ts`, 179 in
`opencode-plugin/memory-wire.ts`, **101 identical** (69% of the smaller, 56% of
the larger). The shared 101 are the domain rules — `endpoint`, `bank`, `post`,
`clamp`, `hash`, `stripReminders`, `isUntrusted`, `formatRecall`. What differs is
entirely host API: extension returns `{systemPrompt}` from
`before_agent_start`; opencode pushes onto an array in
`experimental.chat.system.transform` and fetches messages over a client handle.

So the duplication is real and is the same shape round 4 identified. **But the
obvious fix — one `core.ts` the two hosts import — depends on a fact I could not
verify:** both files are installed as *flat single files*
(`~/.omp/agent/extensions/memory-wire.ts`,
`~/.config/opencode/plugins/memory-wire.ts`) and both begin "do not edit, connect
overwrites this file". Neither has an `import`. Whether either host resolves a
relative import, or whether `connect_ext.rs` / `connect_opencode.rs` could
concatenate a shared body via `include_str!`, is a live-host probe.

**Scoped proposal, in order:**

1. **First, cheap and independent of the answer:** extract the 101 shared lines
   into one file, and have both `connect_*` modules embed it and concatenate. If
   the hosts load a single concatenated file, nothing about the install changes
   and the drift surface drops to one file. `include_str!` is already the
   mechanism (`connect_plugin.rs:43-56` embeds the hermes trio).
2. **Only if (1) is impossible:** keep two files and add a *golden-file* test
   that fails when the shared region drifts, so the duplication is at least
   detected rather than silent. This is strictly worse than (1) and should be
   the fallback, not the plan.

**Proof for (1):** `trust_gate_test.sh` must still pass against both installed
shapes, and the version markers (`MEMORY_WIRE_EXTENSION_VERSION 3`,
`MEMORY_WIRE_OPENCODE_PLUGIN_VERSION 2`) must bump, because `connect` uses them
to decide whether to overwrite.

**What I am not proposing:** a shared *runtime* for the two hosts, or a
TypeScript package. That adds a build step and a dependency to a door whose whole
value is being one file with no install.

### B3. The 14 undocumented modules — a documentation task with a mechanical core

AGENTS.md §3's table lists 18 of 32 modules and states "24.8k lines across 22
modules" against a measured **35,299 lines / 32 modules**. The per-row counts are
also stale: `connect.rs` says 1,263 and is 2,841; `hooks.rs` says 925 and is
1,890; `main.rs` says 1,926 and is 2,972.

Missing from the table, with sizes: `update.rs` 654, `hygiene.rs` 645,
`connect_ext.rs` 830, `connect_opencode.rs` 348, `connect_claude.rs` 324,
`connect_copilot.rs` 306, `grounding.rs` 349, `capture.rs` 355, `guidelines.rs`
251, `http.rs` 280, `memory.rs` 181, `skills.rs` 278, `sweep.rs` 276, `lib.rs`
51.

**Why this is not just cosmetic:** AGENTS §3 is the map a fresh agent reads
before touching anything. Six of the fourteen are entire features (W3 redact,
W4 hygiene, W5 grounding, the skills door, the update path, dimensional memory)
that are invisible from the map. Round 4's review opened with exactly this drift
as its first finding.

**Scope:** add the fourteen rows with a one-line role each, and refresh every
stale count from the tree. This is mechanical except for the role text, which
needs reading each file's module doc.

**Proof:** a script check that the table's module set equals `ls src/*.rs`, and
that each count matches `wc -l`. This is worth adding as a permanent gate, not
just a one-time fix — the same class of gate as A1, and the same reason: a map
that drifts is a map nobody trusts.

---

## 3. Tier C — feature and operational upgrades

### C1. `memory_redact` on MCP — the widest custody gap, and the cheapest fix

Verified state (TODO row 31, `docs/INTEGRATION_GAPS.md` 2026-11-19 addendum):

| capability | HTTP | MCP | CLI | hook |
|---|---|---|---|---|
| redact (W3) | no route | no tool | **yes** | no |
| hygiene (W4) | **yes** | no tool | no | no |
| supersede (W7a) | no route | **yes** (5th tool) | no | no |

An agent that retained a credential through MCP cannot retract it through MCP.
The service method already exists (`MemoryService::redact_memory`,
`src/api.rs:1291`), the `read_only` guard already covers it, and the store
implementation already rewrites live + version rows with a validated regex
(`src/store.rs:2242`). The CLI arm is ~12 lines; an MCP tool is the same shape as
`SUPERSEDE` (`src/mcp.rs:393`, `:649`) plus an annotations entry and the pinning
test at `src/mcp.rs:1117` (which will need its expected list extended from five
to six).

**Scope:** one tool, `read_only_hint: false`, `destructive_hint: true` — the
first tool in the server where destructive is honest, and the annotations doc at
`src/mcp.rs:243-252` says so in prose already ("nothing here is destructive"),
so that comment becomes false and must be updated in the same commit.

**Proof:** the existing `tools_list_should_show_five_annotated_tools` becomes six
and asserts the destructive hint; a live MCP call redacts a row and the row is
gone from both tables; `read_only` bank refuses it.

**Why this is Tier C and not A:** it is a surface decision (the addendum says so),
and it changes the MCP contract that clients code against. It is also the single
item in this plan that closes a real capability gap rather than a documentation
one.

**Not in this plan:** hygiene-on-MCP and supersede-on-HTTP. Both are larger and
neither is a custody hole — hygiene is read-only and supersede is reachable from
the door most agents actually have.

### C2. Two live operational defects on this box, from `memory-wire doctor`

Both are real and were observed on this machine today, not inferred:

**(a) The resolved bank is empty and holds nothing.** `doctor` reports: bank
`ishan-parihar-memory-wire` holds no memories, while `omp` holds 4,241; a hook
here resolves to the empty bank and will recall nothing. A second warning says a
bank `memory-wire` already exists and is where pre-switch memories were kept, and
that nothing was moved. This is the documented `BANK_IDENTITY.md` behaviour — it
is working as designed and the design's cost is that a user's memories are
invisible from the default bank.

**(b) Wired hosts point at the installed binary, not the running one.** A
`doctor --all-hosts` finding: entries point at `~/.local/bin/memory-wire` while
the binary being run is `target/debug/memory-wire`. `connect` fixes this, and the
finding is exactly what `G6.9` was built to surface.

**Scope:** (b) is one `connect` run and a re-check; (a) is a user decision about
which bank is authoritative, and the honest fix is what `doctor` already prints
— a pointer to `docs/BANK_IDENTITY.md` and the two ways to reach the data. **The
one thing worth adding is a single command that names both banks and their counts
side by side**, so the choice is one read instead of three. That is a doctor
output change, not a migration tool.

**What I am not proposing:** an automatic bank migration. Moving 4,241 rows
because a directory was renamed is not reversible by a user who does not know it
happened, and `BANK_IDENTITY.md`'s whole point is that the bank is derived, not
stored.

### C3. PersonaMem v2 — scored, uncommitted, and blocked on your call

Not a run: the run finished 2026-10-17 with 4,954 answers carrying `is_correct`,
and the aggregate is **37.8% (1,872/4,954)**. `TODO.md` row 18 says
"named-not-scheduled"; it is run-and-unread.

It is blocked on a ruling, not on work: the output directory is `.gitignore`d
deliberately, because the run wrote memory rows verbatim and one held a pasted
token (AGENTS §14). Options are in `REMAINING_WORK_PLAN.md` §1.1; the cheapest
honest one is recording the aggregate with the artifact path named as uncommitted.

### C4. Release — 45 commits past `v0.10.0`, footprint intact

Measured just now: `target/release/memory-wire` is 10.63 MiB with `DT_NEEDED=3`,
against the v0.7.0 baseline of 10.25 MiB and 3. No dependency was added. The
competitive position in AGENTS §1 is intact.

45 commits have landed since `v0.10.0`, including every item in §1–§3 above that
is marked done. Per AGENTS §11 a release is your call, not mine; the prepared
path is `scripts/build-release.sh` with the three-Linux set, after disabling
`ci.yml` and `release.yml` around the tag push.

---

## 4. Order

```
Tier A  (no decision, each with a runnable proof)
  A1  wire trust_gate_test.sh into pre-commit + ci     <- highest value, smallest
  A2  genome lint E003 (TODO row 4 evidence cell)
  A3  RND_PLAN §6 retitle + W1-W3 status
  A4  nothing (BENCHMARK.md is a correct historical record)

Tier B  (redundancy, evidence-shaped)
  B2  TS plugin shared-region extraction, option 1 then fallback
  B3  AGENTS §3 table: 14 rows + refreshed counts + a drift gate
  B1  close: connect_* refactor not justified (recorded)

Tier C  (your call, or a live host)
  C1  memory_redact on MCP                <- my pick: real gap, ~1 commit
  C2  doctor bank side-by-side output     <- (b) is one connect run
  C3  PersonaMem v2 aggregate             <- your ruling on the gitignored rows
  C4  release                             <- after C1
```

**Sequencing rationale.** A1 first because it is the only item that makes every
later plugin change self-checking; a gate wired after the work protects nothing.
B2 before B3 because the plugin extraction changes files B3 would otherwise
describe. C1 before C4 because a release should carry the custody fix rather than
promise it.

---

## 5. What I could not determine

Per AGENTS §12, stated rather than guessed:

- **Whether either TS host resolves a relative import.** This is the one fact
  that decides B2's option 1 vs its fallback, and it needs a live probe against
  omp/pi and opencode. I have not run one; the files' headers say flat-file load
  and neither has an import today.
- **Whether the PersonaMem v2 rows contain anything else sensitive.** I checked
  the run's status files and the one known pasted-token case recorded in
  `.gitignore`; I did not scan all 4,954 rows and will not without being asked.
- **Whether the empty-resolved-bank state is one user's misconfiguration or a
  pattern.** I observed it on this box. I have no evidence about other installs,
  and `doctor`'s warning suggests the code anticipates it.
