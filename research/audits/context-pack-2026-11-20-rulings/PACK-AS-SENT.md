# CONTEXT PACK — context-pack-2026-11-20-rulings

## READING CONTRACT

One message, one file: every input this round needs is below. Do not open repository files to re-read
what is already quoted here — that spends your tool calls and adds nothing.

- Cite by anchor: `# SECTION <NAME>` or `# FILE: <path> (lines a,b)`. A finding that rests on a figure
  must name its anchor and keep the figure's evidence class (`[LIVE]`, `[CERT]`, `[IN-SAMPLE]`,
  `[PRIOR]`, `[UNCONFIRMED]`). A figure with no anchor is an assumption: label it one.
- Lens families, where the pack carries them, are instruments, not vocabulary. Naming a lens earns
  nothing; the move the lens forces is the finding.
- If a number you need is absent, say which one and continue under a stated assumption. The report is
  judged on structure, arithmetic and falsifiers, not on filling gaps with plausible values.


# SECTION A-STATE

# FILE: AGENTS.md (lines 26,69)

## 1. What this project is

`memory-wire` is a single-binary agent memory service. A bank-isolated SQLite
store with FTS5 full-text search, exposed over HTTP and MCP, with hooks that
capture conversation prose into it.

It exists to be **the cheap option**. The competitive position is deliberate and
should not be eroded casually:

| | memory-wire | Hindsight | agentmemory |
|---|---|---|---|
| install | one static binary, no daemon required | Python image + Postgres/pg0, optional ONNX reranker | Node runtime, detached engine + worker |
| idle footprint | ~10.7 MiB RSS | 0.8–1.0 GB | ~6 MB heap at 1k, 316 MB at 50k |
| default build deps | 3 shared libraries | — | — |

Three properties are load-bearing and each has been paid for:

- **The default build must not gain a dependency or grow materially.** HEAD is
  **10.65 MiB** (11,166,048 B, `target/release/memory-wire`, measured 2026-11-20
  at `6a2a125`) with `DT_NEEDED` = 3. `v0.7.0` was 10.25 MiB
  (`eval/baseline/2026-10-13-v0.7.0/BENCH_FOOTPRINT.md`); the 9.7 MiB this line
  used to state was earlier still, and 10.25 was itself a year-old figure this
  line carried until a reviewer checked it against the binary. It happened again on
  2026-11-20: the figure was 24,808 B stale after four commits, and the round-5
  reviewer caught it by recomputing the delta rather than reading it. **Do not
  trust a size quoted here without re-running `stat`** — quote the command, not
  the number. Verify `ldd` after touching anything that links.
- **Retrieval reports are the record.** The competitor whose retrieval number we
  can actually compare against is agentmemory, on LongMemEval-S, and we lead it
  on the honest (unfitted) number. Do not publish a number you cannot trace to
  a committed artifact.
- **Retrieval is a reordering problem, not a coverage problem.** The candidate
  pool contains the gold memory in 500 of 500 LongMemEval questions. Work on
  ordering; do not spend effort on recall/coverage mechanisms. (Pool membership is
  not the same as the *top-k a reader is served*: on LoCoMo, questions with several
  gold sessions miss at k = 10 — `recall_all@10` 83.0% vs `recall_any@10` 94.2% —
  and that is still an ordering problem inside the pool. See `docs/handoff/RND_PLAN.md`.)

The parent projects are `rohitg00/agentmemory` and the Hindsight repositories,
audited read-only under `_audit/`. This is a deliberate Tier-C extraction: the
~3k LOC retrieval core is ported, the ~40k LOC surround is intentionally out.
**Full parent parity is not the goal** — pursuing it means rebuilding the
parents.


# FILE: AGENTS.md (lines 105,150)

## 3. Where things live

`src/`, 33 modules, 39,778 lines. Counts are `wc -l` against this tree;
`scripts/module_table_check.sh` fails when the table and the tree disagree, so a
stale count is a red gate rather than a claim nobody checks.

| module | lines | role |
|---|---|---|
| `store.rs` | 5816 | SQLite: schema, migrations, put/recall/delete, FTS5, redaction, versions |
| `api.rs` | 4724 | HTTP routes, the frozen error contract, the historian prompt |
| `main.rs` | 3379 | clap surface, routing, wiring |
| `connect.rs` | 3410 | agent-host detection and config writing |
| `recall.rs` | 2707 | RRF fusion, candidate pool, budget trim, scoring |
| `hooks.rs` | 2175 | the five lifecycle events; the never-fail hook contract |
| `mcp.rs` | 2299 | MCP server: 6 tools + resources + prompts + completion |
| `daemon.rs` | 1208 | `daemon start/stop/status`, pidfile, start-time identity |
| `doctor.rs` | 1272 | the health report (`doctor`, `doctor --all-hosts`) |
| `connect_plugin.rs` | 1172 | the hermes `MemoryProvider` plugin |
| `vector.rs` | 1058 | dense arm (`embed` feature, OFF by default) |
| `cli_home.rs` | 948 | the content-first bare-invocation view |
| `connect_codex.rs` | 1033 | codex TOML config writing |
| `connect_ext.rs` | 886 | omp and pi: the two hosts installed as a **TypeScript file**, not a config entry |
| `paths.rs` | 826 | bank identity, endpoint, data paths |
| `render.rs` | 683 | human-readable report rendering |
| `update.rs` | 686 | `update`: the five-step self-update swap |
| `seed.rs` | 693 | transcript reading for hook capture |
| `hygiene.rs` | 658 | W4: deterministic hygiene scan (merge/contradiction/date/bloat) |
| `mcp_http.rs` | 542 | stateless streamable-HTTP MCP mount |
| `connect_opencode.rs` | 443 | opencode's plugin dir (v2 layout) |
| `capture.rs` | 363 | hook-based auto-capture: dedup, privacy filter, raw observation |
| `grounding.rs` | 350 | W5: bounded startup grounding capsule |
| `connect_claude.rs` | 362 | Claude Code's MCP entry |
| `connect_copilot.rs` | 330 | Copilot CLI's MCP entry |
| `http.rs` | 296 | dependency-free HTTP/1.1 client for loopback |
| `skills.rs` | 282 | `connect --with-skills`: deploy the agent skill per host |
| `sweep.rs` | 313 | `sweep`: forget what a bank said it would stop remembering |
| `guidelines.rs` | 275 | native agent-rules writer (marker-delimited block) |
| `memory.rs` | 179 | canonical memory types shared by all layers |
| `connect_common.rs` | 174 | helpers the per-host connect modules share |
| `embed.rs` | 185 | ONNX session, vendored MiniLM |
| `lib.rs` | 51 | crate root: re-exports and the module map |

`tests/` — `backup.rs`, `daemon.rs`, `e2e.rs`, `recall_budget.rs`, `scale.rs`.
`examples/` — 15 benchmark and evaluation harnesses.
`eval/amb/` — our provider for vectorize-io's AMB harness, and its committed runs.


# SECTION B-MEASURED

# FILE: research/audits/context-pack-2026-11-20-rulings/measure_rulings.sh (lines 1,59)

#!/bin/sh
# Measure the surface the rulings round cites. Every figure in RULINGS.md is
# produced here, so a reviewer (or a later agent) re-derives a number instead of
# trusting a claim. Run from the subject repo root:
#
#   sh research/audits/context-pack-2026-11-20-rulings/measure_rulings.sh
#
# Nothing here mutates the tree. It reads, counts, and prints.

cd "$(dirname "$0")/../../.." || exit 1

echo "## Subject"
echo "HEAD:        $(git log -1 --format='%h %s')"
echo "branch:      $(git rev-parse --abbrev-ref HEAD)"
echo "tree:        $(git status --porcelain | wc -l | tr -d ' ') modified paths"
echo "version:     $(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)"/\1/')"
echo

echo "## Where the work stands"
printf 'TODO open:        %s\n' "$(awk -F'|' '/^\| *[0-9]+ *\|/ {gsub(/ /,"",$3); if($3=="open"||$3=="in-progress") c++} END{print c+0}' TODO.md)"
printf 'TODO done:        %s\n' "$(awk -F'|' '/^\| *[0-9]+ *\|/ {gsub(/ /,"",$3); if($3=="done") c++} END{print c+0}' TODO.md)"
printf 'modules:          %s\n' "$(ls src/*.rs | wc -l | tr -d ' ')"
printf 'src lines:        %s\n' "$(cat src/*.rs | wc -l | tr -d ' ')"
printf 'release vs tag:   %s commits past v0.10.0\n' "$(git rev-list --count v0.10.0..HEAD)"
echo

echo "## The TS door duplication, measured not estimated"
for f in plugin/integrations/extension/memory-wire.ts plugin/integrations/opencode-plugin/memory-wire.ts; do
  printf '%-52s %s lines\n' "$f" "$(wc -l < "$f" | tr -d ' ')"
done
echo
echo "-- module-level constants (top-level 'var NAME =', uppercase) --"
for f in plugin/integrations/extension/memory-wire.ts plugin/integrations/opencode-plugin/memory-wire.ts; do
  printf '%s:\n' "$f"
  grep -o '^var [A-Z][A-Z0-9_]*' "$f" | sed 's/^var /  /'
done
echo
echo "-- function names per door --"
for f in plugin/integrations/extension/memory-wire.ts plugin/integrations/opencode-plugin/memory-wire.ts; do
  printf '%s:\n' "$f"
  grep -o '^\(export \)\?function [A-Za-z_][A-Za-z0-9_]*' "$f" | sed 's/^export //;s/^function /  /'
done
echo

echo "## The installed binary vs the tree"
printf 'installed:  %s\n' "$("$HOME/.local/bin/memory-wire" --version 2>&1 | head -1)"
printf 'in tree:    %s\n' "$(./target/release/memory-wire --version 2>&1 | head -1)"
printf 'release bin built: %s\n' "$(date -r target/release/memory-wire '+%Y-%m-%d %H:%M' 2>/dev/null || echo unknown)"
echo

echo "## The blank-content guard and the schema it does not enforce"
grep -n "content.trim().is_empty()" src/api.rs | head -3
grep -n "content TEXT NOT NULL" src/store.rs | head -3
printf 'CHECK constraints naming content: %s\n' "$(grep -c "CHECK.*content" src/store.rs)"
echo

echo "## Gate cost, so the reviewer can price a new check"
printf 'trust gate checks:  %s\n' "$(grep -c '^check \|^  check \|^ok \|^  ok ' plugin/integrations/trust_gate_test.sh 2>/dev/null)"
printf 'tests:              %s\n' "$(PATH=/usr/bin:$PATH cargo test --locked 2>/dev/null | grep -o 'test result: ok. [0-9]*' | awk '{s+=$4} END{print s+0}')"

# SECTION C-RULINGS

# FILE: research/audits/context-pack-2026-11-20-rulings/RULINGS.md (lines 1,126)

# RULINGS — the decisions this round exists to settle

The plan (`docs/plans/2026-11-20-remaining-gaps-plan.md`) ends with three
decisions waiting on one person. This sheet lists them with the options on the
table, what each unblocks, and what leaving it undecided costs. It carries the
decisions, not my answers to them — where I already hold a preference it is
marked **[agent-held]** so a recommendation that merely repeats it is visible as
such.

Each row names its owner. Rows an agent can decide itself are **not** here; the
operator's scarce resource is attention, and padding this sheet with agent work
buries the rows that need them.

Evidence class: every figure below is produced by
`research/audits/context-pack-2026-11-20-rulings/measure_rulings.sh`, kept
beside this sheet.

---

## R1 — the TypeScript door duplication: fence it, or remove it?

**Owner: operator.**

Two TypeScript doors — `plugin/integrations/extension/memory-wire.ts` (356
lines) and `plugin/integrations/opencode-plugin/memory-wire.ts` (479 lines) —
carry the same domain core. Measured today: **13 module-level constants
identical in both**, **8 functions identical in both**, zero divergence. The
functions are `endpoint`, `bank`, `clamp`, `hash`, `stripReminders`, `textOf`,
`isUntrusted`, `formatRecall` — the whole policy core. The doors differ only in
`lastAssistantText` (extension) and `setup` (opencode), which is host glue.

**Options:**

- **(a) remove it** — `include_str!` concatenation: one core file, spliced at
  build time by the trust gate. No host restructure, no runtime import, no
  packaging change. Cost: a build step that must be checked in and must itself
  be gated.
- **(b) fence it** — a drift test that fails when the two cores diverge. Does
  not remove the duplication; proves nothing about correctness; purely additive,
  cannot break a live install.
- **(c) leave it** — record it as accepted duplication and do nothing.
- **(d) restructure both hosts** to subdirectories with relative imports, the
  idiomatic fix. **[agent-held: I recommend against this one]** — it rewrites
  live installs on other people's machines, and AGENTS §9 says `connect` owns
  host files.

**What deciding it unblocks:** a change to the recall cap, the tier handling, or
the footer shape can be made once instead of twice.

**What leaving it undecided costs:** every future change to that core is a
two-file edit with no failure signal if one is missed. This has already cost
one real defect: the round-5 footer bug existed in Rust and hermes because a
shape was ported by hand.

**What the pack does not settle:** whether a `include_str!` splice can be made
tamper-evident enough to trust, and whether either host's loader tolerates the
generated file. Both are testable.

---

## R2 — build the cross-door parity suite, or is the current gate enough?

**Owner: operator.**

**Options:**

- **(a) build it** — a suite sourced from `Host::ALL` in `src/connect.rs`
  (not a literal list, so a 7th host fails the check rather than being silently
  absent), driving `hooks.rs`, hermes, extension and opencode against **one
  fixture set**: failed-then-recovered `/recall`, repeated prompts, blank
  content, section-cap overflow, `format=summary`. Lives in `scripts/`, CI-only,
  **0 binary bytes**.
- **(b) extend the current gate incrementally** — the round-5 additions
  (`cache_source_check.mjs`, `cache_rule_test.mjs`) are a down-payment covering
  one door's one bug.
- **(c) leave it** — per-door checks only.

**What deciding it unblocks:** the failure class that produced two of this
session's defects — a check that ran nowhere, or that verified self-authored
bytes rather than the host's behaviour.

**What leaving it undecided costs:** the next cross-door divergence is found by
whoever happens to read all four doors, which is how round 5 found the footer
bug.

**What the pack does not settle:** the true cost. It is estimated at 2-3 days
and has never been scoped against the actual door APIs. An estimate that has
never been checked against the code is a guess.

---

## R3 — the release: measure now and cut when told, or keep deferring?

**Owner: operator. [agent-held: I will not tag or publish unasked — AGENTS §11]**

**Options:**

- **(a) measure now, cut on your word** — footprint, `DT_NEEDED`, the
  three-Linux asset set, all re-derived from a freshly built release binary.
- **(b) keep deferring** until the Tier-1 and Tier-2 work lands, so one release
  carries all of it.

**What deciding it unblocks:** knowing whether the current tree is releasable at
all — the installed binary on this machine is **0.9.0**, 60 commits behind, and
predates `memory_redact`, the doctor banks work, the `connect_common`
extraction, the opencode v2 port, and both round-5 fixes.

**What leaving it undecided costs:** every day of deferral widens the gap
between what the repo says and what runs on the machine. Any measurement taken
against the installed binary measures code I would not defend.

**What the pack does not settle:** whether the 60-commit batch is one coherent
release or should be split. That is a judgement about the history, not a
measurement.

---

## Not on this sheet, deliberately

- **R4 (door parity residue)** — TODO 31. The widest gap is closed; what remains
  is absence, not lockout, and I hold a recommendation. It is a surface
  preference, not a blocked decision.
- **R5 (PersonaMem 37.8%)** — the run output is gitignored because it holds a
  pasted token (AGENTS §14). There is no evidence here to rule on.
- **R6 (agent-memory-bench)** — blocked on an external harness, not on a
  decision. Scoping it is agent work and is not on this sheet for that reason.

# SECTION D-PLAN

# FILE: docs/plans/2026-11-20-remaining-gaps-plan.md (lines 1,311)

# Outline plan — remaining gaps, redundancy, and upgrades (2026-11-20)

Written after `84ba417` (HEAD, tree clean). It answers the question the previous
plan left open: **what is actually left**, now that Tiers A/B/C and the round-5
review fixes have landed.

The short answer is that almost nothing is left in the "cleanup" column, and the
things that *are* left are of three kinds: two defects I can fix without a
decision, four items that need **your** ruling, and one item that is a real
design fork. This plan separates those, because the last two are blocked on you
and the first two are not.

---

## 0. What this plan is not

- **Not a re-litigation of `docs/handoff/CLOSED.md`.** Every closed lever stays
  closed. Nothing here proposes vectors, a reranker, consolidation, or a score
  threshold (AGENTS §8, and §32/§48 on the threshold).
- **Not a reason to touch the evaluation set.** No item here selects a parameter
  against LongMemEval. Items that *measure* it are marked as consultations
  (AGENTS §6; §34/§35 in `CONSISTENCY.md` record the count, currently 5 of 6).
- **Not a release plan.** Tagging and publishing are yours (AGENTS §11). C4
  below is the measurement that would inform that call, not the call.

---

## 1. What I verified before writing this

I read the tree rather than trusting the summary. Each claim below traces to
something I ran or read today.

| check | result |
|---|---|
| `git status` | clean, HEAD `84ba417` |
| TODO rows | 8 open, 30 done |
| module table | 33 modules, 39,778 lines, gate green |
| installed vs repo binary | **`~/.local/bin/memory-wire` is 0.9.0**; repo is 0.10.0, **59 commits** ahead |
| `doctor` | works: names all 9 banks, flags the empty resolved bank, points at `omp` (4,241) |
| `serve.json` | gone — the residue the last plan listed no longer exists |
| debt markers | 4 `ponytail:` comments, all documented simplifications with ceilings, none actionable |
| TS door duplication | **13 module-level constants identical** in both TS doors, **8 identical helper functions** |

### 1.1 A correction to my own last commit

`84ba417` wrote into `docs/handoff/STATE.md` that Tier A shipped "a nightly
check that `.skills_to_install/` still matches `src/skills.rs`". **That is
wrong, and I wrote it.** There is no `.skills_to_install/` directory anywhere in
the repo or its history, and nothing references one. What actually exists is a
`#[cfg(test)]` test, `the_embedded_source_is_the_one_on_disk` (`src/skills.rs`),
which asserts the `include_str!`-embedded skill equals the file on disk. That
test is *stronger* than the thing I described — a change to `SKILL.md` cannot
compile without the file matching — but the description named a mechanism that
does not exist.

This is the same class of error the round-5 review spent most of its length on:
**a claim about a check that is not the check.** It belongs in this plan's first
tier because it is a documentation defect about verification, and because I am
the one who introduced it one commit ago.

---

## 2. Tier 1 — fix now, no decision needed

### 1.1 Correct the STATE entry (§1 above)

Rewrite the `.skills_to_install/` sentence to name the real test. One line, no
behaviour change. Gate: existing suite. Record in `CONSISTENCY.md` §70.

### 1.2 TODO 37 — the cross-door blank-content divergence

**The finding is verified and still open.** The two TypeScript doors skip blank
or whitespace-only content before the take; `src/hooks.rs` and the hermes plugin
count it against the cap and render `- ` for it. I confirmed this by reading all
four.

**Why it is latent, not live:** `src/api.rs:760` rejects blank content on every
retain path, so this build cannot write such a row. But the schema is
`content TEXT NOT NULL` with **no `CHECK`**, so an older build or a direct
`sqlite3` insert can produce one.

**Why I did not fix it last time, and what would change that:** a change made
because a review mentioned it, with no case that reproduces, is a change made on
assertion. The honest fix is to **make it reproducible first**:

1. Add a test that inserts a blank-content row through SQL, bypassing the API.
   If the schema refuses, the gap closes itself and the row is answered.
2. If it inserts, fix all four doors to skip blank rows, and add a `CHECK` if
   the store layer should reject it at the boundary too.

Either outcome is progress. The test is the work; the fix is one line in two
languages.

### 1.3 Rebuild `target/release/memory-wire`

The release binary in the tree is from `6a2a125` and predates every fix in this
session. Not a repo change, but every measurement I take until it is rebuilt is
measured on stale code — including the footprint number I would put in front of
you for C4. `cargo build --release --locked`, then re-verify `DT_NEEDED`.

---

## 3. Tier 2 — the two design forks that need your call

These are the items where I can build either side and the choice is yours.

### 2.1 TODO 36 — the TypeScript door duplication, now quantified

Last plan deferred this because the probe was unrun. **The probe has since been
run** (recorded in the plan's own §6 and in TODO 36): both hosts resolve
relative imports, but only from a *directory* — omp loads every flat `.ts` as an
extension and errors on a helper, while `index.ts` in a subdirectory loads
cleanly. So extraction is feasible and requires restructuring both installs.

I measured the duplication precisely, which the TODO estimated as "~100 lines":

| | extension | opencode |
|---|---|---|
| module-level constants | 13 | 13 |
| **identical constants** | **13** | **13** |
| functions | 9 | 9 |
| **identical functions** | **8** | **8** |

Zero divergence today. The eight shared functions are `bank`, `clamp`,
`endpoint`, `formatRecall`, `hash`, `isUntrusted`, `stripReminders`,
`textOf` — the entire policy core. The doors differ only in `lastAssistantText`
(extension) vs `setup` (opencode), which is genuinely host-specific.

**Three options, and I now think the TODO's ranking is wrong:**

- **(a) `include_str!` concatenation** — the shared core as one file, spliced in
  at build time by the trust gate. No host restructure, no runtime import, no
  packaging change. The TODO lists this as "option 1", and I agree it is the
  cheapest correct move. Cost: the build step must be *checked in*, because a
  splice that runs nowhere is exactly the failure mode round 5 identified.
- **(b) subdirectory + relative import** — correct and idiomatic, but restructures
  both live installs and needs a migration for existing users. Reversible only by
  the person who owns the machine (AGENTS §9: `connect` owns host files).
- **(c) a golden-file drift test only** — fails when the cores diverge, proves
  nothing about correctness. TODO calls this "costing nothing and proving
  nothing", which is right, but it is also the only option that is *purely*
  additive and cannot break a live install.

**My recommendation: (c) now, (a) later, never (b) unasked.** (c) is a day and
closes the *silent divergence* risk that is the actual maintenance cost; (a) is
the real fix if you want it; (b) touches other people's machines.

**Question for you:** do you want the duplication removed (a), or only fenced
(c)?

### 2.2 TODO 38 — the cross-door parity suite

Rounds 4 and 5 of the review **independently** converged on this from different
evidence, which the record treats as the strongest signal available. The shape:

- source the host list from `Host::ALL` in `src/connect.rs`, not a literal, so a
  7th host fails the check instead of being silently absent (round 4's exact
  finding about the old measure script, which listed 6 of 8);
- drive `hooks.rs`, hermes, extension and opencode against **one fixture set**:
  failed-then-recovered `/recall`, repeated prompts, blank content, section-cap
  overflow, `format=summary`;
- live in `scripts/`, CI-only, **0 binary bytes**.

**This is not blocked on you** — it is Tier 1 work by my own classification
(no decision needed), but it is large enough (a new harness, four drivers) that
it deserves a green light rather than a surprise. The two new trust-gate checks
from round 5 (`cache_source_check.mjs`, `cache_rule_test.mjs`) are a
down-payment: they cover one door's one bug, not the matrix.

**Question for you:** build it, or is the current per-door gate enough for now?

---

## 4. Tier 3 — blocked on you, or on the world

### 3.1 C4 — release readiness, measured but not cut

- `10.65 MiB` at `v0.7.0`'s baseline (per `eval/baseline/…/BENCH_FOOTPRINT.md`);
  the current build's footprint needs re-measuring after §1.3.
- `DT_NEEDED = 3`.
- **59 commits past `v0.10.0`.**
- AGENTS §11: tagging is your call, and `scripts/build-release.sh` refuses a
  dirty tree, so this must land clean first.

**Not doing it unasked.** Say the word and I will measure, then cut.

### 3.2 The installed binary is 0.9.0

`~/.local/bin/memory-wire` is 59 commits behind and predates: `memory_redact`,
the doctor banks block, the `connect_common` extraction, the opencode v2 port,
the module table, and both round-5 fixes. Any agent on this machine that runs
the installed binary is running code I would not defend.

**Installing over it is a write to your live environment** and it will make
`connect`-managed host configs inconsistent with the installed binary until
`connect` re-runs. Reversible, but yours to trigger. The clean sequence is:
install, then `memory-wire connect` for each host, then `doctor` to confirm.

### 3.3 C3 — PersonaMem v2 aggregate is 37.8%

Unchanged from the last plan. The run output is gitignored because it contains
a pasted token (AGENTS §14), so I will not re-read it, and the aggregate is
already recorded. **No action unless you rule on it.**

### 3.4 TODO 10 — `agent-memory-bench` Claude Code arm

The round-5 review's closing argument: this is the **only external benchmark
that grades the product's actual job**, and every ranking in the repo is
currently inference from static retrieval plus unit checks. It is the missing
load-bearing evidence.

It is also the item most likely to be blocked by something outside this repo
(the benchmark's own harness). **Not started, not scoped** — flagging it as the
highest-value open item rather than pretending it is a small one.

### 3.5 TODO 11/12/13 — judged runs, AMB granularity, aroute defects

All three are recorded with their reasoning and none is a cleanup item:

- **11** needs free-tier LLM quota and is slow by nature.
- **12** is decided *against* on the dev set (`eval/LOCOMO_TURNS.md`:
  `recall_all@10` 83.0% → 80.7%), so building it would buy measurability of the
  long corpora at a known ranking cost. It is a measurement instrument, not a
  lever, and is correctly marked as such.
- **13** is an aroute-side defect; AGENTS §4 and `docs/AROUTE_BUGS.md` say
  memory-wire agents do not edit aroute.

### 3.6 TODO 31 — door parity, residue

The widest gap (redact on MCP) is **closed**. What remains is absence, not
lockout: hygiene has no MCP/CLI door, supersede has no HTTP route, grounding has
neither. The row's own text now says this correctly, and records that the
round-2 review's other structural claim was **refuted** (hermes
`is_available()` returning True on a down server is deliberate, with the 2 s
timeout cost documented).

This is a **surface decision**, not a bug fix. Options: leave the doors
asymmetric and documented, or keep adding doors. I lean toward the former —
every added door is another surface to keep in parity, and the audit's own
finding is that asymmetry is what happens when each capability is added through
the door its item needed.

---

## 5. What I am *not* proposing, and why

- **No shared abstraction over `connect_*`.** B1 already extracted the six
  byte-identical helpers; round 5's analysis is that 52% of those files is
  inline tests that cannot be deduplicated without obscuring golden
  expectations, and only 1,793 production lines sit in the six adapters. A Rust
  trait cannot see host-side load failures like the opencode one. The
  *enumerated check* (2.2) is the instrument, not a trait.
- **No new dependency.** The default build must not gain one (AGENTS §1), and
  nothing in this plan needs one.
- **No re-proposal of any closed lever** (`CLOSED.md`): vectors, cross-encoder,
  graph, consolidation, stemming, IDF, coverage stream, page-cache tuning.
- **No touch to the four `ponytail:` simplifications.** Each names a ceiling and
  an upgrade path; that is an accepted pattern, not debt.
- **No change to any count claim that is inside a historical-record fence**
  (e.g. `docs/BENCHMARK.md`'s "4 tools"). Those are dated records and stay.

---

## 6. Order, and what each tier costs

| # | item | tier | ~size | blocked on |
|---|---|---|---|---|
| 1.1 | correct the STATE `.skills_to_install/` claim | 1 | one line | no |
| 1.2 | blank-content: make it reproducible, then fix or close | 1 | ½ day | no |
| 1.3 | rebuild the release binary | 1 | minutes | no |
| 2.1 | TS door duplication: (c) fence now, (a) extract if wanted | 2 | ½ day (c) / 2 days (a) | **your call on (a) vs (c)** |
| 2.2 | cross-door parity suite | 2 | 2–3 days | green light |
| 3.1 | release readiness (measure, then cut) | 3 | hours | **your call** |
| 3.2 | install 0.10.0 over 0.9.0 | 3 | minutes | **your call** |
| 3.3–3.6 | PersonaMem, agent-memory-bench, judged runs, door parity | 3 | — | you, or external |

**Recommended order:** 1.1 → 1.3 → 1.2 → then 2.2 (it is the instrument that
would have caught both round-5 defects) → then 2.1(c) → then stop and ask about
2.1(a), 3.1, 3.2.

That order front-loads the two things that are cheap and that reduce the chance
of the next defect reaching you: correct the record about a check, and build the
check that would have caught the last two.

---

## 7. What I could not determine

Per AGENTS §12, stated rather than guessed:

- **Whether the blank-content row is reachable in practice.** I proved the API
  rejects it and the schema permits it. I have not found an install or a build
  that produces one. §1.2 is designed to settle this rather than assume it.
- **Whether the opencode install on this machine is the restructured kind.** The
  port is committed; I have not verified a live 2.x load since. A live check
  belongs to §3.2's install sequence.
- **Whether 37.8% on PersonaMem is a defect or a property of the corpus.** I
  will not re-read the gitignored rows (AGENTS §14), so I have the aggregate
  and nothing else.
- **How much of the 59-commit lag is user-visible.** I can enumerate what
  changed; I cannot enumerate what a given host's agent actually calls.

---

## 8. Definition of done for this plan

Every item in Tier 1 closed with a gate green and a `CONSISTENCY.md` section;
every Tier 2 item either built or explicitly ruled out by you with the reason
recorded; Tier 3 items either done on your word or recorded as still-blocked
with the blocker named. No item quietly dropped — a dropped item is a phantom
for the next agent, which is the failure `docs/handoff/` exists to prevent.

# SECTION E-LAW

# FILE: AGENTS.md (lines 232,292)

## 6. The evaluation set is not a training set

This is the rule that matters most here, and the one most easily broken by
accident.

`eval/RESULTS.md` holds LongMemEval-S results over 500 questions. Those 500
questions are an **instrument for measuring**, never an objective to optimise
against.

**Never do any of the following and then describe the result as an improvement:**

- sweep a parameter, weight, threshold, or feature configuration against the
  LongMemEval numbers and keep the winner
- add, remove, or reorder a retrieval signal because it moved R@5 / R@10 /
  NDCG@10 on LongMemEval
- re-run a change, observe a delta, and iterate until the delta is positive
- quote a LongMemEval number as evidence that a component is "better" when the
  component was chosen by looking at that number

**A change selected by looking at the evaluation set is fitted, not earned.**
When that has happened — and it has — say so in the commit message and in
`docs/CONSISTENCY.md`, mark the value provisional, and treat re-validation on
independent data as outstanding work.

| Allowed | Not allowed |
|---|---|
| Measuring on LongMemEval and reporting the number | Choosing a value by comparing LongMemEval numbers |
| Fixing a bug that LongMemEval exposed | Tuning until a metric stops complaining |
| Deciding a change on a mechanistic argument, then *observing* the metric | Deciding a change on the metric |
| Selecting against a **development** set that is not LongMemEval | Using the test partition as a dev set |
| Reporting that a change did not help | Reverting a change because a benchmark "regressed" while keeping an equal-or-worse alternative that scored better |

### The three-set discipline

- **dev** — a labelled set that is *not* LongMemEval. All selection happens here.
  `eval/data/locomo/` is the dev corpus (independent source, ~1530 usable
  queries, gold document ids). See `docs/EVALUATION_HYGIENE.md`.
- **test** — LongMemEval-S. Consulted for **measurement and release acceptance
  only**. Never for selection.
- **synthetic** — `examples/bench_recall_curve.rs` and `tests/scale.rs` generate
  their own corpora with fixed seeds. Useful for scaling and regression
  assertions, but we author the distractors, so it is weak evidence about
  ranking quality. Do not tune against it either.
- **measure-only** — every other external set (PrecisionMemBench, BEAM,
  agent-memory-bench, …). Same rule as test: report, never select. Any harness
  that runs LongMemEval (OAMB, MemBase, memorybench) *is* a test consultation.
  Roles are assigned in `docs/BENCHMARK_LANDSCAPE.md` before a set's first number.

**A constant changed in response to a bug is derived from mechanism, then
measured exactly once.** Never iterate until a metric improves — that yields a
better number and a worthless one. `DEFAULT_RECALL_BUDGET` is the worked example:
derived as 3× the measured median LongMemEval session, measured once, and the
result reported whether it moved or not.

### Count the consultations

Every change that is *selected* on the test set burns it. `docs/CONSISTENCY.md`
records the count. If a component's parameters were fitted on LongMemEval, it
carries a `provisional:` marker in the code and in the docs until an independent
set confirms it.


# FILE: AGENTS.md (lines 311,341)

## 8. Rejected work stays rejected

Levers already measured and removed, with the measurement that killed each, are
listed in `docs/RERANKING_PLAN.md` §3 and `docs/PERFORMANCE_PLAN.md`. Do not
re-propose one without new evidence. If you find yourself re-litigating a
rejected lever, the answer is in those documents, not in your intuition.

Currently closed, each with its measurement recorded: vectors as a default,
a cross-encoder, a graph stream, consolidation and extraction, Porter stemming,
IDF in the overlap stream, a distinct-term-coverage stream, page-cache and
mmap tuning, and the token-overlap stream's fitted weight.

## 9. Interface rules

- **The hook contract is never-fail.** A hook that cannot reach the server must
  exit 0 silently: a memory server that is down must not put an error in front
  of a model. See the module doc at the top of `src/hooks.rs`. Silent swallowing
  is *required* there and is a finding everywhere else.
- **The HTTP error contract is frozen.** Unknown enum value → `400 invalid
  content`; wrong type → `422` from the deserializer; duplicate `document_id`
  under `append` → `409`; a pinned-bank mismatch → refusal naming the pin.
  Clients depend on these. Changing one is an API break.
- **Write config defensively.** A malformed host config is refused, not
  overwritten. A moved binary's hook is replaced, not duplicated. Every write
  is atomic with re-read verification.
- **Do not hand-edit an installed agent host's files** to make something work.
  `connect` and `connect --uninstall` own those entries.
- Default JSON responses are part of the contract. Pagination totals ride in the
  `X-Total-Count` header, and `?truncate=` / `?format=toon` are opt-in, because
  changing the body shape breaks every existing client.


# FILE: AGENTS.md (lines 357,396)

## 11. Git and releases

- **Do not tag, publish, or cut a release unless asked.** Cutting a release is
  the user's call.
- Do not force CI to run. `ci.yml` triggers on `pull_request`,
  `workflow_dispatch` and `push: tags: ['v*']` only — never on branch pushes —
  because **GitHub Actions quota is a metered exhaustible resource and is
  currently exhausted**. Publishing a release triggers CI once, via the tag.
- Releases are cut locally with `scripts/build-release.sh`, which publishes
  through the `gh` REST API and therefore bills no Actions minutes. It refuses a
  dirty tree, a tag that does not name `HEAD`, an incomplete target set, and
  clobbering a release that already has assets.
- Disable CI around a tag push — **both `ci.yml` and `release.yml`** trigger on
  `push: tags: ['v*']` (`gh workflow disable ci.yml release.yml` … re-enable) so
  the cut costs nothing.
- The complete Linux set from this host is
  `--only "linux-x86_64 linux-x86_64-musl linux-aarch64"` — a quoted word-list;
  the script splits `--only` on spaces. No macOS asset has ever shipped, so the
  three-Linux set is not a partial release. `linux-x86_64` is a glibc build and
  carries the build host's floor (needs ≥ 2.39, built on 2.44);
  `linux-x86_64-musl` is plain-cargo static-pie, `DT_NEEDED=0`, no floor —
  live-verified on the Debian 12 / glibc 2.36 racknerd VPS, where the
  installer's probe rejects the gnu asset and completes on musl;
  `linux-aarch64` is a `cargo-zigbuild` cross.
- Version bump first: edit `Cargo.toml`, run one unlocked `cargo check` to
  refresh the own-version line in `Cargo.lock`, then the `--locked` gates pass.
  Two commits — feature, then bump — and the tag names the bump commit.
- `install/get-memory-wire.sh --bank <id>` forwards the pin to every `connect` it
  runs, including the `--uninstall` that unwires hosts before the binary is removed.
  `connect` keeps a pin already baked into a host when `--bank` is absent (TODO #1),
  so a plain re-run does not drop it.
- macOS assets cannot be cross-built from Linux — `libsqlite3-sys` links
  CoreFoundation. The script skips them with that reason.
- Delegate only with the `space-bunny` model.
- `scripts/build-release.sh` runs `scripts/secret-scan.sh --all` before it builds
  anything, `--check` included, and refuses a tracked secret. A tag publishes the
  tree GitHub serves, so this gate is the last one before a credential goes public.
- A tag moves only in a history rewrite, and only with the owner's go-ahead. The
  procedure, with its lease-checked push and the SHA map, is `docs/SECRETS.md` §2.


# SECTION F-CLOSED

# FILE: docs/handoff/CLOSED.md (lines 1,45)

# Closed levers — measured and rejected (do not re-propose without new evidence)

AGENTS §8 forbids re-litigating these. Each row names the measurement that closed it;
if you think new evidence reopens one, state what is new **before** you spend anything.
Detail lives in the cited documents; this is the index a cheaper agent can scan.

| lever | verdict | closed by |
|---|---|---|
| dense vectors **as a fusion-weight lever** (H1d) | measured 2026-10-15, gate FAILED: `recall_all@10` +1.8pp best (vec 0.75) bought at −9.9pp R@1; deep-miss rescue 4/99; below-gate at low weights. Re-opening needs a new mechanism, not a re-run (`CONSISTENCY.md` §41) | `eval/SELECTION_VECTOR_AXIS_ALL10.md` |
| dense vectors **as a default** | declined: no coverage failure to fix on LongMemEval (R@20 99.6%, pool 500/500); costs ~3× artifact size, 4–7× idle RSS, 19–30× write path. The arm exists behind `--features embed`, weight 0.0. Reopening is the **user's** call — now informed by the H1d measurement above | `docs/PERFORMANCE_PLAN.md` "Explicitly not doing"; `docs/EXCEED_PLAN.md` Phase D |
| cross-encoder reranker | disqualified: breaks the single-binary position, unmeasured even by Hindsight who ship it optional | `docs/RERANKING_PLAN.md` §3.2 |
| graph stream; consolidation / extraction ladder | both competitors measure with it off; no benchmark win available | `docs/EXCEED_PLAN.md` §3 |
| Porter stemming (E2) | flat on top of E1; temporal-reasoning −2.26 pp; removed | `docs/NEXT_ITERATION.md` E2 |
| IDF-weighted overlap (E3) | the hypothesis it would fix the recall curve was disproved; removed | `docs/NEXT_ITERATION.md` E3 |
| distinct-term-coverage stream (E4) | algebraically identical to raising the overlap weight; removed | `docs/NEXT_ITERATION.md` E4 |
| `cache_size` / `mmap` tuning | no gain, +15–17% RSS; refused | `docs/PERFORMANCE_PLAN.md` |
| the token-overlap stream's *fitted weight* as evidence | `overlap 0.25` was chosen on LongMemEval (46 configs): fitted, not earned. Not "closed" as a value, but closed as a claim | `docs/EVALUATION_HYGIENE.md` §2.1 |
| a **hardcoded** synonym table | maintenance + licensing; the user-supplied `recallSynonyms` config is the supported form | `docs/EXCEED_PLAN.md` §3 |
| score thresholds on auto-injection | measured; the levers are composition, caps, labels, tags — not a cut-off | `docs/CONSISTENCY.md` §32; `docs/OPEN_HOOK_RECALL_RELEVANCE.md` |
| `agreement` prior (`FusionWeights::agreement`) | moved **0 of 1,531** LoCoMo questions at 0.50 (and 0 of 500 on LongMemEval) | `eval/SELECTION.md` reading 1 |
| `k` in RRF (`k = 60`, inherited from Hindsight) | flat: R@5 spans 86.0–86.1% across k ∈ {5…120} at the shipped overlap | `eval/SELECTION.md` reading 3 |
| `bm25_magnitude` as a big win | does not collapse (the competitor warning does not transfer) but R@5 only 86.0→86.3% at 4.00 — small. *`recall_all@10` was not measured then*; H1c reopens only that metric — **re-opened 2026-10-14 and closed again**: `recall_all@10` 83.0→82.8% (4 up / 7 down), multi-hop R@1 −3.4pp (n 1,531) | `eval/SELECTION.md` reading 2; H1c: `eval/LOCOMO_LEVERS.md`, `docs/CONSISTENCY.md` §37 |
| `keyword_scope: ExactAndPrefix` (H1a) | best single-lever R@1 gain on the dev set (R@1 +3.0pp, MRR +2.2pp, NDCG +1.7pp) but `recall_all@10` +0.5pp < the +1pp gate and single-hop R@5 −0.4pp — real, deterministic. Gate failed as pre-registered; no LongMemEval consultation | `eval/LOCOMO_LEVERS.md`, `docs/CONSISTENCY.md` §37 |
| `overlap_scope: BestSentence` (H1b) | `recall_all@10` −0.3pp, R@1 −3.4pp overall, multi-hop R@1 −11.3pp (n 1,531). Concentrating overlap on one dense segment loses more than it gains | `eval/LOCOMO_LEVERS.md`, `docs/CONSISTENCY.md` §37 |
| recency 1.0 `TemporalQueriesOnly` on the dated corpus (H3) | falsified: temporal `recall_all@10` −7.8pp, temporal R@1 −33.4pp; control exact (1,025/1,025 detector-negative lists bit-identical). LoCoMo temporal questions ask *when an event happened*, not *what is newest*. Recency stays 0.0; the dated corpus (H0) remains for date-*aware* mechanisms that are not a recency prior | `eval/LOCOMO_LEVERS.md`, `docs/CONSISTENCY.md` §37 |
| intermediate write granularity, windows of W=2 turns (H2) | R@1 −1.1pp, `recall_all@10` −0.4pp vs sessions, and R@pool 100.0→98.7% — windowing pushes banks past the candidate-pool ceiling, so some gold is unreachable at any K. With `Turns` (§35) also worse, granularity is closed: sessions stay | `eval/LOCOMO_WINDOWS.md`, `docs/CONSISTENCY.md` §37 |
| **turn-level writes, session scored at best turn** (this cycle) | worse on every aggregate on LoCoMo at the shipped config: `recall_all@10` 83.0→80.7%, R@1 60.4→54.6%, R@5 86.0→83.0%, NDCG@10 73.6→69.7% (n 1,531); per-query `recall_all@10` 42 up / 76 down. One run, not tuned | `eval/LOCOMO_TURNS.md`, `docs/CONSISTENCY.md` §35 |
| a recency stream *measured on the old LoCoMo ingest* | invalid, not rejected: every LoCoMo `created_at` was the harness clock (0.006 s spread), so R@1 24.8% at `recency 1.0` is an ingest-order artifact. Re-test only after H0 | `eval/SELECTION.md` "recency caveat" |
| cross-bank retrieval | banks are isolated by design; Hindsight's accuracy comes from ~1000 candidates *inside* one bank | `docs/EXCEED_PLAN.md` §3 |
| targeting Hindsight's 92.0% | a different instrument (LLM-judged accuracy, third-party judge, ~1000-deep pool) | `docs/EXCEED_PLAN.md` §0.2 |

## Things that look like findings but are instrument artifacts

Do not cite these as memory-wire properties:

- **17.0% LongMemEval `answer_quality`**: a small-reader floor, undated prompt (STATE §2).
  *The dated re-measure (2026-10-14, harness fixed per TODO #16) reads 18.6% conditioned /
  6.0% closed-book — a different instrument (dates in the prompt), recorded beside, never
  compared as better/worse (`CONSISTENCY.md` §39). Neither number is comparable to any
  published LongMemEval figure: judge, prompt and truncation are all this harness's own.*
- **84.1% AMB LoCoMo**: free `small-stack` reader and judge; not comparable to any
  published LoCoMo score; the date handling predates the `created_at` round-trip.
- **Latency / RSS / throughput in the baseline**: taken at load 11–28 on 24 cores.
- **Relevance-battery junk-injected 0/9**: the live bank `omp` grew; the code did not change.
- **PrecisionMemBench 14/77**: 77 cases, measure-only — *superseded 2026-10-14 by the H6 capability change: with scope-tag filters honoured the adapter measures 19/77, precision 0.24 (`eval/amb/PRECISIONMEMBENCH-2026-10-14-h6-filters.json.gz`). The pre-change 14/77 stays here as the historical no-filter reading.*

# SECTION G-DOORS

# FILE: src/hooks.rs (lines 752,811)

/// **The trust gate lives here.** Rows whose tier is `untrusted_external` are
/// dropped before rendering, because this function's output is the only path by
/// which a recalled memory reaches a model without a reader asking for it.
///
/// The ordering matters: filtering happens before the [`MAX_LINES`] take, so an
/// untrusted row does not consume a slot that a trusted row would have filled. A
/// gate applied after the take would silently *reduce* recall to fewer than
/// `MAX_LINES` trusted rows whenever the ranking put untrusted rows on top —
/// which is exactly the adversarial case the gate exists for, where fetched text
/// is shaped to rank first.
fn recall_section(bank: &str, hits: &[ScoredHit]) -> String {
    let hits: Vec<&ScoredHit> = hits
        .iter()
        .filter(|h| !is_untrusted(h.trust.as_deref()))
        .collect();
    if hits.is_empty() {
        return String::new();
    }
    let mut out =
        format!("\n### memory-wire recall — `{bank}`\n(recalled by relevance, unverified)\n\n");
    let mut left = SECTION_CAP_CHARS;
    let mut shown = 0usize;
    for hit in hits.iter().take(MAX_LINES) {
        // The date prefix is the serving-shape change H12 measured: a reader
        // resolving "what did we decide last week" is blind without it, and the
        // wire carried the occurrence time all along. The prefix renders only
        // when the wire did — an old server's bare strings degrade to the exact
        // pre-2026-10-17 rendering, byte for byte.
        let date_prefix = hit
            .created_at
            .as_deref()
            .filter(|t| t.len() >= 10)
            .map(|t| format!("[{}] ", &t[..10]))
            .unwrap_or_default();
        let entry = format!("- {date_prefix}{}\n", clamp(&hit.content, ENTRY_CAP_CHARS));
        if entry.chars().count() > left {
            break;
        }
        out.push_str(&entry);
        left -= entry.chars().count();
        shown += 1;
    }
    // The count is `hits.len() - shown`, not `hits.len() - MAX_LINES`: the loop
    // can stop on the *character* cap before it reaches `MAX_LINES`, and the
    // footer used to claim only the line-cap remainder was dropped. Found by the
    // round-5 review (2026-11-20), which read the early-`break` path this line
    // had never been exercised against. The two caps stop the loop for different
    // reasons and only the first is `MAX_LINES`.
    if hits.len() > shown {
        out.push_str(&format!("- …and {} more\n", hits.len() - shown));
    }
    out
}

/// Whether a tier string names the one tier the injection block refuses.
///
/// Only a positive match drops a row. `None` (the bare wire shape, or a server
/// too old to send the field) keeps it, and so does any tier this build does not
/// recognise — including one a *newer* server invents. Both are the same
/// judgement: a row is withheld on evidence, never on the absence of evidence.

# FILE: plugin/integrations/hermes/memory-wire/__init__.py (lines 440,931)

class MemoryWireProvider(MemoryProvider):
    """memory-wire as a Hermes memory provider."""

    @property
    def name(self) -> str:
        return "memory-wire"

    def is_available(self) -> bool:
        """Whether this provider could serve a session. No network call.

        Hermes calls this during agent init and inside
        ``discover_memory_providers()``, which runs for *every* candidate
        plugin on the box -- a probe here would put a 2s timeout in front of
        starting the agent. So this answers the question it can answer without
        the network: is there a URL to talk to, and is it one we can build a
        path from. A server that is down still reports available, and the hooks
        then say nothing, which is the documented never-fail contract.
        """
        return _valid_url(_endpoint())

    def initialize(self, session_id: str, **kwargs: Any) -> None:
        """Bind per-session state. No network call, and no failure mode.

        Hermes passes ``hermes_home`` and ``platform`` in every case, and may
        pass ``agent_context`` (``primary`` / ``subagent`` / ``cron`` / ``flush``)
        for some. The context matters: a cron employee replaying a system prompt
        would otherwise write the system prompt into a user representation, which
        the base class warns about by name, so writes are skipped for anything
        that is not the primary agent.
        """
        self._session_id = session_id
        self._base = _endpoint()
        self._bank = _bank()
        self._home = kwargs.get("hermes_home") or _hermes_home()
        self._platform = kwargs.get("platform", "")
        self._context = kwargs.get("agent_context", "primary")
        self._identity = kwargs.get("agent_identity", "")
        self._cwd = kwargs.get("cwd", "")
        self._started = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())

    def _writable(self) -> bool:
        """Whether this session should write, per Hermes' own context rule."""
        return self._context in ("primary", "")

    # -- lifecycle -----------------------------------------------------------

    def system_prompt_block(self) -> str:
        """The historian framing, plus which bank this session is bound to.

        Static, per the base class: "This is for STATIC provider info
        (instructions, status)." ``MemoryManager.build_system_prompt`` calls
        this inline while assembling the system prompt, so a network call here
        would sit on the critical path of every session start.

        Note the base class also says prefetched recall is injected *separately*
        -- through ``prefetch`` -- so this block is instructions, not content,
        and must not try to smuggle recalled memories in through it.
        """
        if not self._base:
            self._base = _endpoint()
        return (
            f"## memory-wire — bank `{self._bank}`\n\n"
            f"{HISTORIAN_PREAMBLE}\n"
            "Memories in this bank are read with `memory_recall`, written with\n"
            "`memory_retain`, and cited with `memory_reflect`. The store is local\n"
            f"and unauthenticated; it is served at `{self._base}`.\n"
        )

    def prefetch(self, query: str, *, session_id: str = "") -> str:
        """Recall for the upcoming turn, as injected context.

        The signature is keyword-only on ``session_id`` because that is what
        ``MemoryManager._prefetch_provider`` calls
        (``provider.prefetch(query, session_id=session_id)``). A provider that
        took it positionally would be called correctly and then be wrong about
        which argument it received.

        Returns "" for a blank query, an unreachable server, an empty bank, and a
        server that errored. Hermes wraps this in its own thread with an 8s
        deadline and logs a warning if we exceed it, so one TIMEOUT is the whole
        cost of a down server.

        The top ``MAX_LINES`` are injected. Each carries its fused RRF ``score``
        — see :func:`_recall` for what it is and why nothing branches on it —
        and the ranking is *relative*, so this emits the best of whatever set
        the bank offered, relevant or not. That is the defect
        ``docs/OPEN_HOOK_RECALL_RELEVANCE.md`` records. The score reaches this
        function, but the policy half stays closed on determinism grounds
        (CONSISTENCY §32/§48) rather than for want of data: AGENTS §6 designates
        ``eval/data/locomo/`` as the dev set, ~1,530 queries.

        One thing *is* filtered, and it is not relevance: rows marked
        :data:`UNTRUSTED_TIER` are dropped, because this function's output is
        how a recalled memory reaches a model without anyone asking for it. An
        explicit ``memory_recall`` still returns those rows. See
        :data:`UNTRUSTED_TIER` for why this is a boundary and not a ranking rule.
        """
        if not _valid_url(self._base):
            return ""
        hits = _recall(self._base, self._bank, _strip_reminders(query), EXCLUDE_TAGS)
        if not hits:
            return ""
        # Before the take, not after: a poisoned row at rank 0 must not consume
        # a slot a trusted row would have filled. Ranking untrusted text first
        # is exactly the adversarial case this exists for.
        hits = [h for h in hits if h.get("trust") != UNTRUSTED_TIER]
        if not hits:
            return ""
        # The shape every auto-injection surface emits (audit GAP-1): the bank
        # header, the provenance label — a best-of-irrelevant set must not read
        # as a verified one — and at most MAX_LINES entries, each clamped, the
        # whole block under SECTION_CAP_CHARS.
        out = (
            f"### memory-wire recall — `{self._bank}`\n"
            "(recalled by relevance, unverified)\n\n"
        )
        left = SECTION_CAP_CHARS
        shown = 0
        for hit in hits[:MAX_LINES]:
            # The date prefix renders only when the wire carried one, so a
            # server older than the field degrades to the exact pre-2026-11-19
            # rendering, byte for byte. Same shape as `src/hooks.rs`:
            # `[<YYYY-MM-DD>] ` before the clamped content, day precision
            # because that is what an occurrence time is for.
            stamp = hit.get("created_at") or ""
            prefix = f"[{stamp[:10]}] " if len(stamp) >= 10 else ""
            entry = f"- {prefix}{_clamp(hit['content'], ENTRY_CAP_CHARS)}\n"
            if len(entry) > left:
                break
            out += entry
            left -= len(entry)
            shown += 1
        # `len(hits) - shown`, not `len(hits) - MAX_LINES`: the loop can stop on
        # the *character* cap before it reaches MAX_LINES, and the old footer
        # claimed only the line-cap remainder was dropped. Same defect as
        # `src/hooks.rs`, fixed the same day (round-5 review, 2026-11-20).
        if len(hits) > shown:
            out += f"- …and {len(hits) - shown} more\n"
        return out

    def sync_turn(self, user_content: str, assistant_content: str, *, session_id: str = "", messages: Optional[List[Dict[str, Any]]] = None) -> None:
        """Retain the turn that just completed.

        Runs on Hermes' background sync worker, so blocking here costs the drain
        deadline rather than the turn.

        Retention discipline (audit GAP-6): reminder envelopes are stripped; a
        turn whose answer is shorter than :data:`MIN_ANSWER_CHARS` is a bare
        acknowledgement and is not retained — retaining prompt-only rows is
        how a bank fills with duplicate question texts that recall then
        injects as if they were memory. The pair is **head-anchored**: the
        user half is clamped and always present, and only the answer half is
        tail-cut — the question is the retrieval key, and the old whole-body
        tail-cut could drop it exactly when the answer was long enough to be
        worth keeping. ``document_id`` keys the row on the question, so a
        repeated ask replaces its row rather than piling up twins.
        """
        if not self._writable():
            return
        user = _strip_reminders((user_content or "").strip())
        if not user:
            return
        answer = (assistant_content or "").strip()
        if len(answer) < MIN_ANSWER_CHARS:
            return
        head = f"turn {self._session_id}"
        if self._identity:
            head += f" [{self._identity}]"
        lead = f"{head}\nuser: {_clamp(user, USER_CAP_CHARS)}"
        # The answer keeps its tail — the end of a working turn is the
        # decision — in whatever room the head-anchor leaves it.
        room = max(0, FLUSH_CHARS - len(lead) - len("\nassistant: "))
        body = f"{lead}\nassistant: {answer[-room:]}"
        _retain(
            self._base,
            self._bank,
            body,
            "hook:sync-turn",
            document_id=f"turn-{_djb36(user)}",
        )

    def on_session_end(self, messages: List[Dict[str, Any]]) -> None:
        """Retain the conversation's own words as the session ends.

        The counterpart of `memory-wire hook session-end`, and it stores the same
        thing: prose, not a pointer to a file. The file-based hook has to be
        handed a path in its payload; here the conversation is already in memory,
        so the words are retained directly.

        Positional-only, one argument, because that is how
        ``MemoryManager.end_session`` calls it
        (``provider.on_session_end(messages)``).
        """
        if not self._writable():
            return
        prose = _prose(messages)
        if not prose.strip():
            return
        head = f"session-end session {self._session_id}"
        _retain(
            self._base,
            self._bank,
            f"{head}\n{prose}"[-FLUSH_CHARS:],
            "hook:session-end",
        )

    def on_pre_compress(self, messages: List[Dict[str, Any]]) -> str:
        """Retain the conversation's prose, and return what to keep of it.

        This is the contract the other provider plugins on this box get wrong.
        ``MemoryManager.on_pre_compress`` collects each provider's **return
        value** and hands it to the compressor as ``memory_context``
        (``agent/conversation_compression.py``, "The provider's on_pre_compress()
        may return a string of insights it wants surfaced inside the compression
        summary"). The `messages` list is the transcript the compressor is about
        to read anyway, so mutating it in place buys nothing and risks writing a
        synthetic turn into the very transcript being summarised.

        So: the retain is the work, and the return is a short truthful marker --
        what was stored, into which bank, under which session -- so the
        compressor's summary can preserve the fact instead of dropping it.

        Returns "" when there was nothing to store, which the base class and
        `MemoryManager` both treat as "no contribution".
        """
        if not self._writable():
            return ""
        prose = _prose(messages)
        if not prose.strip():
            return ""
        kept = f"pre-compact session {self._session_id}\n{prose}"[-FLUSH_CHARS:]
        if not _retain(self._base, self._bank, kept, "hook:pre-compact"):
            return ""
        return (
            f"[memory-wire] The {len(prose)} characters of conversation above "
            f"were retained to bank `{self._bank}` (session {self._session_id}) "
            "before this context was compressed. That record is outside this "
            "conversation; a later session can recall it with `memory_recall`."
        )

    def on_memory_write(self, action: str, target: str, content: str, metadata: Optional[Dict[str, Any]] = None) -> None:
        """Not supported. Says so, once, out loud.

        The sixth event on the competitor's manifest. Mirroring it is not
        something this build can do honestly:

        * ``action='remove'`` has no counterpart. memory-wire's HTTP surface
          deletes by memory id, and a retain of the same content is a content
          hash no-op rather than a handle we were given -- so a remove either
          cannot be expressed or would silently delete the wrong row.
        * ``target`` is ``'memory'`` or ``'user'``, which are two files
          (MEMORY.md, USER.md) in Hermes' store. memory-wire has banks, not two
          files, and inventing a MEMORY.md-to-bank mapping would be a namespace
          decision this crate has not made.
        * Retaining on ``add`` alone would be half the contract under a name
          that promises all of it.

        So the hook says nothing to the model, does not write, and prints one
        line to stderr so the gap is visible in a log rather than inferred from
        a silence. See README.md, "The sixth event".
        """
        global _memory_write_warned
        if _memory_write_warned or not content:
            return
        _memory_write_warned = True
        print(
            "memory-wire: Hermes wrote to its built-in "
            f"{target!r} store ({action}) and that write was not mirrored. "
            "memory-wire has no supported mapping from MEMORY.md/USER.md to a "
            "bank, and cannot express a remove. Use `memory_retain` instead.",
            file=sys.stderr,
        )

    def shutdown(self) -> None:
        """Nothing to drain. Every call here is synchronous and bounded."""
        return

    # -- tools ---------------------------------------------------------------

    def get_tool_schemas(self) -> List[Dict[str, Any]]:
        """The same four tools ``memory-wire mcp`` serves over stdio.

        Names, descriptions, parameter objects and annotations are copied from
        ``Server::tools()`` in ``src/mcp.rs`` rather than restated, so a Hermes
        agent and an MCP client see one surface. Hermes wraps each entry as
        ``{"type": "function", "function": schema}`` after
        ``normalize_tool_schema`` drops anything that is not a dict, so the
        ``annotations`` block rides along harmlessly and is honest about what
        each tool does.

        None of the four names is in Hermes' reserved core set -- which is
        ``memory`` (singular), not ``memory_recall`` -- so none of these is
        silently dropped at registration.
        """
        return [
            {
                "name": "memory_retain",
                "description": (
                    "Retain a memory in a bank. Content and context are redacted "
                    "server-side before the write. Creates the bank if it is new."
                ),
                "parameters": {
                    "type": "object",
                    "properties": {
                        "bank": {"type": "string", "description": "Bank id; defaults to the session's bank."},
                        "content": {"type": "string", "description": "What to remember. Required."},
                        "context": {"type": "string", "description": "Optional capture context; redacted too."},
                        "tags": {"type": "array", "items": {"type": "string"}, "description": "Tags for this memory."},
                        "document_id": {"type": "string", "description": "Re-write this document instead of adding another memory."},
                        "update_mode": {"type": "string", "enum": ["replace", "append"], "description": "Whether a document_id write overwrites or extends it. Defaults to replace."},
                    },
                    "required": ["content"],
                },
                "annotations": _annotations(False),
            },
            {
                "name": "memory_recall",
                "description": (
                    'Recall memories from a bank, most relevant first, as a JSON '
                    'array of content strings. `budget` is a hard token cap, '
                    '`tags` restricts the search to memories carrying any of '
                    'them, and `format: "full"` returns {id, score, content} '
                    "objects instead so the caller can cite what it got."
                ),
                "parameters": {
                    "type": "object",
                    "properties": {
                        "bank": {"type": "string", "description": "Bank id; defaults to the session's bank."},
                        "query": {"type": "string", "description": "Search text. Required."},
                        "budget": {"type": "integer", "description": "Token cap; falls back to the bank's recallMaxTokens, then 2000."},
                        "tags": {"type": "array", "items": {"type": "string"}, "description": "Only memories carrying any of these tags."},
                        "format": {"type": "string", "enum": ["full"], "description": "Return {id, score, content} objects instead of bare strings."},
                    },
                    "required": ["query"],
                },
                "annotations": _annotations(True),
            },
            {
                "name": "memory_reflect",
                "description": (
                    "Answer a question from a bank, citing the memory it came "
                    "from. Currently top-hit citation, not synthesis — there is "
                    "no LLM in the loop."
                ),
                "parameters": {
                    "type": "object",
                    "properties": {
                        "bank": {"type": "string", "description": "Bank id; defaults to the session's bank."},
                        "query": {"type": "string", "description": "The question. Required."},
                        "tags": {"type": "array", "items": {"type": "string"}, "description": "Only consider memories carrying any of these tags."},
                    },
                    "required": ["query"],
                },
                "annotations": _annotations(True),
            },
            {
                "name": "memory_bank_config_get",
                "description": (
                    "Read a bank's stored config object (recallMaxTokens, "
                    "retainTags, and any keys this build does not act on, served "
                    "verbatim)."
                ),
                "parameters": {
                    "type": "object",
                    "properties": {
                        "bank": {"type": "string", "description": "Bank id; defaults to the session's bank."}
                    },
                    "required": [],
                },
                "annotations": _annotations(True),
            },
        ]

    def handle_tool_call(self, tool_name: str, args: Dict[str, Any], **kwargs: Any) -> str:
        """Run one tool and return a JSON string.

        A string is mandatory, not a convenience: Hermes stores the return value
        as the tool result ``content`` in the session history, and
        Anthropic-protocol providers reject non-string content with a 400 on the
        *next* request. So a dict returned from here would break the turn after
        the one that called it.

        Bank resolution is the same as everywhere else: an explicit ``bank``
        wins, else the session's. A present-but-blank ``bank`` is refused rather
        than treated as absent, because silently writing to a different bank than
        the caller named is the failure this guards against.
        """
        args = args or {}
        bank = args.get("bank")
        if bank is None:
            bank = self._bank
        if not isinstance(bank, str) or not bank.strip():
            return json.dumps({"error": "invalid bank id"})
        bank = bank.strip()

        if tool_name == "memory_retain":
            content = args.get("content")
            if not isinstance(content, str) or not content.strip():
                return json.dumps({"error": "invalid content"})
            body: Dict[str, Any] = {"content": content}
            if isinstance(args.get("context"), str):
                body["context"] = args["context"]
            if isinstance(args.get("document_id"), str):
                body["document_id"] = args["document_id"]
            if isinstance(args.get("update_mode"), str):
                body["update_mode"] = args["update_mode"]
            tags = args.get("tags")
            if isinstance(tags, list):
                body["tags"] = [t for t in tags if isinstance(t, str)]
            out = _request(self._base, f"/banks/{bank}/retain", body, method="POST")
            if out is None:
                return json.dumps({"error": "memory-wire server unreachable"})
            return json.dumps(out)

        if tool_name == "memory_recall":
            query = args.get("query")
            if not isinstance(query, str) or not query.strip():
                return json.dumps({"error": "invalid query"})
            body = {"query": query}
            if isinstance(args.get("budget"), int):
                body["budget"] = args["budget"]
            if args.get("format") == "full":
                body["format"] = "full"
            tags = args.get("tags")
            if isinstance(tags, list):
                body["tags"] = [t for t in tags if isinstance(t, str)]
            out = _request(self._base, f"/banks/{bank}/recall", body, method="POST")
            if out is None:
                return json.dumps({"error": "memory-wire server unreachable"})
            return json.dumps(out)

        if tool_name == "memory_reflect":
            query = args.get("query")
            if not isinstance(query, str) or not query.strip():
                return json.dumps({"error": "invalid query"})
            body = {"query": query}
            tags = args.get("tags")
            if isinstance(tags, list):
                body["tags"] = [t for t in tags if isinstance(t, str)]
            out = _request(self._base, f"/banks/{bank}/reflect", body, method="POST")
            if out is None:
                return json.dumps({"error": "memory-wire server unreachable"})
            return json.dumps(out)

        if tool_name == "memory_bank_config_get":
            out = _request(self._base, f"/banks/{bank}/config")
            if out is None:
                # The route 404s a bank that was never created, which is the
                # documented rule, and a config read is not worth an error for.
                return json.dumps({})
            return json.dumps(out)

        return json.dumps({"error": f"Unknown tool: {tool_name}"})

    # -- configuration -------------------------------------------------------

    def get_config_schema(self) -> List[Dict[str, Any]]:
        """Fields for ``hermes memory setup``.

        Both carry an explicit ``env_var``, which is what lets ``save_config``
        stay a no-op for the environment half and is the arrangement the base
        class asks for when a provider uses env vars.
        """
        return [
            {
                "key": "url",
                "description": "memory-wire server URL",
                "default": DEFAULT_ENDPOINT,
                "env_var": "MEMORY_WIRE_URL",
            },
            {
                "key": "bank",
                "description": "Bank id this Hermes session reads and writes",
                "default": DEFAULT_BANK,
                "env_var": "MEMORY_WIRE_BANK",
            },
        ]

    def save_config(self, values: Dict[str, Any], hermes_home: str) -> None:
        """Write the non-secret fields to ``$HERMES_HOME/memory-wire.json``.

        Keys this plugin does not act on are preserved, so a value written by a
        newer build is not destroyed by an older one re-saving the bank.
        """
        path = Path(hermes_home) / "memory-wire.json"
        doc = _read_config_file(path)
        for key in ("url", "bank"):
            if key in values:
                doc[key] = values[key]
        path.write_text(json.dumps(doc, indent=2) + "\n", encoding="utf-8")



# FILE: plugin/integrations/extension/memory-wire.ts (lines 205,356)

var UNTRUSTED_TIER = "untrusted_external";

/** A row a mount may not read back as instructions. Absent means keep. */
function isUntrusted(item) {
  if (!item || typeof item !== "object") return false;
  return item.trust === UNTRUSTED_TIER;
}

/** The block appended to the system prompt, or `""` when there is nothing worth
 *  saying. Empty is the signal the caller uses to inject nothing at all.
 *
 *  `items` are the server's `{id, score, content}` entries, so each carries the
 *  fused RRF score the ranking used. It is *relative* — the top hit of an
 *  irrelevant set scores well by construction, because BM25 is min-max
 *  normalised per query — and nothing branches on it here, and nothing will: a
 *  score threshold on the injection path was measured and rejected twice
 *  (CONSISTENCY §32/§48) on determinism grounds. The reason is not a shortage
 *  of labelled data — AGENTS §6 designates `eval/data/locomo/` as the dev set,
 *  ~1,530 queries. A cut would be fitted on one corpus and would move when the
 *  ranking moved. The top `RECALL_ENTRIES` are injected unconditionally; see
 *  `docs/OPEN_HOOK_RECALL_RELEVANCE.md`. */
function formatRecall(items) {
  var lines = [];
  for (var i = 0; i < items.length; i++) {
    var item = items[i];
    // Filtered **before** the take, and the take is `lines.length` below —
    // not `i`. Looping on `i < RECALL_ENTRIES` would bound the *input* scan,
    // so five untrusted rows ranked above one trusted row would consume the
    // whole budget and the trusted row would never be reached. That is a
    // starvation bug, not a cap: `src/hooks.rs` and the hermes plugin both
    // filter first and take second, and this file has to agree with them.
    if (isUntrusted(item)) continue;
    // The default recall response is a bare array of content strings; a
    // `format: "full"` server answers with `{id, score, content}` instead. Both
    // are accepted so this file does not depend on which one it is talking to.
    var content = typeof item === "string" ? item : item && item.content;
    if (typeof content !== "string" || !content.trim()) continue;
    if (lines.length >= RECALL_ENTRIES) break;
    // The occurrence time, when the wire carried one. `src/hooks.rs` prefixes
    // it for the same reason H12 measured: a reader resolving "what did we
    // decide last week" is blind without it. It renders only when the field is
    // there, so a server older than it degrades to the exact pre-2026-11-19
    // rendering, byte for byte. Day precision -- that is what an occurrence
    // time is for.
    var stamp = (item && item.created_at) || "";
    var datePrefix = stamp.length >= 10 ? "[" + stamp.slice(0, 10) + "] " : "";
    lines.push(lines.length + 1 + ". " + datePrefix + clamp(content, ENTRY_CAP_CHARS));
  }
  if (!lines.length) return "";
  var block =
    "## memory-wire long-term memory\n" +
    "(recalled by relevance from bank `" + bank() + "`, unverified)\n" +
    lines.join("\n");
  return block.length <= INJECT_CAP_CHARS ? block : block.slice(0, INJECT_CAP_CHARS - 1) + "…";
}

/** memory-wire's memory extension: recall injected before each turn, the turn
 *  retained after it.
 *  @param {{ on: (event: string, handler: Function) => void }} pi */
export default function memoryWireExtension(pi) {
  var lastPrompt = "";
  var lastAnswer = "";
  var lastRetained = "";

  pi.on("before_agent_start", async function (event) {
    var prompt = event && typeof event.prompt === "string" ? stripReminders(event.prompt) : "";
    if (!prompt) return undefined;
    // Recorded before the await, so a recall that fails still leaves the prompt
    // available to `agent_end`.
    lastPrompt = prompt;

    // A prompt this short cannot rank anything on its own: "Continue" matches
    // every memory equally and the top of that tie is noise. The query borrows
    // the previous turn's answer, so what ranks is the work being continued,
    // with the prompt still steering inside it. A mechanism, not a filter --
    // nothing is cut on a score (docs/OPEN_HOOK_RECALL_RELEVANCE.md).
    var query = prompt;
    if (prompt.length <= SHORT_PROMPT_CHARS && lastAnswer) {
      query = clamp(lastAnswer, QUERY_CONTEXT_CHARS) + "\n" + prompt;
    }

    var items = await post(
      "/banks/" + encodeURIComponent(bank()) + "/recall",
      // `format: "full"` is what puts `score` on the wire; the default shape is a
      // bare array of content strings. `formatRecall` reads both, so this stays
      // compatible with a server that does not know the field.
      // `MEMORY_WIRE_FORMAT=summary` asks for H8 capsules instead of whole
      // memories. It comes from the environment rather than the bank's
      // `injectionFormat` config key because this file already reads
      // `MEMORY_WIRE_URL` and `MEMORY_WIRE_BANK` from there, so it costs no
      // extra request; reading the config key would add one per turn, which
      // `src/hooks.rs` documents as already costing one. The Rust hook reads
      // both and the bank wins there. Anything but "summary" reads as default.
      {
        query: query,
        budget: RECALL_BUDGET,
        format:
          (process.env.MEMORY_WIRE_FORMAT || "").trim().toLowerCase() === "summary"
            ? "summary"
            : "full",
        exclude_tags: EXCLUDE_TAGS,
      },
      RECALL_TIMEOUT_MS
    );
    if (!Array.isArray(items)) return undefined;
    var block = formatRecall(items);
    if (!block) return undefined;

    // Returning `systemPrompt` REPLACES it, so the harness's own prompt has to be
    // carried through or its instructions are silently erased. A host may hand
    // it over as a string or as a list of segments; both are joined rather than
    // stringified, which would collapse a list into one comma-riddled line.
    var own = event.systemPrompt;
    if (Array.isArray(own)) own = own.join("\n\n");
    return { systemPrompt: [own, block].filter(Boolean).join("\n\n") };
  });

  pi.on("agent_end", async function (event) {
    var prompt = lastPrompt;
    // Cleared before the await: a turn that starts before this one finishes must
    // not have its prompt consumed by the earlier retain.
    lastPrompt = "";
    var answer = lastAssistantText(event && event.messages);
    // The next turn's short-prompt query composition borrows this answer.
    lastAnswer = answer;
    // An unanswered or bare-acknowledgement turn is a question with no knowledge
    // in it: retaining prompt-only rows is how the bank filled with duplicate
    // question texts ("asked: continue", asked three times over) that recall
    // then injected as if they were memory. The pair is the memory.
    if (!prompt || !answer || answer.length < MIN_ANSWER_CHARS) return undefined;

    var content = [
      "asked: " + clamp(prompt, RETAIN_PART_CAP_CHARS),
      "answered: " + clamp(answer, RETAIN_PART_CAP_CHARS),
    ].join("\n");
    // An auto-retry re-submits the same turn, and the server dedupes identical
    // content anyway -- but not before paying for the write, so it is checked here.
    if (!content || content === lastRetained) return undefined;

    // `document_id` keyed on the prompt makes a repeated ask replace its row
    // (the server's default update mode) rather than add a twin.
    var stored = await post(
      "/banks/" + encodeURIComponent(bank()) + "/retain",
      { content: content, document_id: "turn-" + hash(prompt) },
      RETAIN_TIMEOUT_MS
    );
    // Only a confirmed store updates the marker, so a retain that failed is
    // retried on the next turn rather than dropped for the rest of the session.
    if (stored) lastRetained = content;
    return undefined;
  });
}

# FILE: plugin/integrations/opencode-plugin/memory-wire.ts (lines 176,479)

var UNTRUSTED_TIER = "untrusted_external";

/** A row a mount may not read back as instructions. Absent means keep. */
function isUntrusted(item) {
  if (!item || typeof item !== "object") return false;
  return item.trust === UNTRUSTED_TIER;
}

/** The block appended to the system prompt, or `""` when there is nothing
 *  worth saying. Empty is the signal the caller uses to inject nothing at all.
 *
 *  `items` are the server's `{id, score, content}` entries, so each carries
 *  the fused RRF score the ranking used. It is *relative* -- the top hit of an
 *  irrelevant set scores well by construction, because BM25 is min-max
 *  normalised per query -- and nothing branches on it here, and nothing will: a
 *  score threshold on the injection path was measured and rejected twice
 *  (CONSISTENCY §32/§48) on determinism grounds. The reason is not a shortage
 *  of labelled data -- AGENTS §6 designates `eval/data/locomo/` as the dev set,
 *  ~1,530 queries. A cut would be fitted on one corpus and would move when the
 *  ranking moved. The label line exists instead: it says what these rows are,
 *  so an irrelevant set cannot read as a verified one. The top
 *  `RECALL_ENTRIES` are injected unconditionally either way; see
 *  docs/OPEN_HOOK_RECALL_RELEVANCE.md. */
function formatRecall(items) {
  var lines = [];
  for (var i = 0; i < items.length; i++) {
    var item = items[i];
    // Filtered **before** the take, and the take is `lines.length` below —
    // not `i`. Looping on `i < RECALL_ENTRIES` would bound the *input* scan,
    // so five untrusted rows ranked above one trusted row would consume the
    // whole budget and the trusted row would never be reached. That is a
    // starvation bug, not a cap: `src/hooks.rs` and the hermes plugin both
    // filter first and take second, and this file has to agree with them.
    if (isUntrusted(item)) continue;
    // The default recall response is a bare array of content strings; a
    // `format: "full"` server answers with `{id, score, content}` instead.
    // Both are accepted so this file does not depend on which one it is
    // talking to.
    var content = typeof item === "string" ? item : item && item.content;
    if (typeof content !== "string" || !content.trim()) continue;
    if (lines.length >= RECALL_ENTRIES) break;
    // The occurrence time, when the wire carried one. `src/hooks.rs` prefixes
    // it for the same reason H12 measured: a reader resolving "what did we
    // decide last week" is blind without it. It renders only when the field is
    // there, so a server older than it degrades to the exact pre-2026-11-19
    // rendering, byte for byte. Day precision -- that is what an occurrence
    // time is for.
    var stamp = (item && item.created_at) || "";
    var datePrefix = stamp.length >= 10 ? "[" + stamp.slice(0, 10) + "] " : "";
    lines.push(lines.length + 1 + ". " + datePrefix + clamp(content, ENTRY_CAP_CHARS));
  }
  if (!lines.length) return "";
  var block =
    "## memory-wire long-term memory\n" +
    "(recalled by relevance from bank `" + bank() + "`, unverified)\n" +
    lines.join("\n");
  return block.length <= INJECT_CAP_CHARS ? block : block.slice(0, INJECT_CAP_CHARS - 1) + "…";
}

/** The newest assistant text of the session, via `ctx.session.context`.
 *
 *  In v2 the context *is* the client (`docs/build/plugins/server`), so there
 *  is no separate `client.session.messages` to reach for -- the v1 code called
 *  that and it no longer exists. `context()` takes `{ sessionID }` and returns
 *  the session's messages, verified live against 2.3.2 (it schema-validates
 *  the id before doing any work).
 *
 *  `""` whenever any of it is missing, refused, or slow: a retain that cannot
 *  read the answer is skipped, and the turn is simply not remembered. Nothing
 *  here may throw into the agent.
 *
 *  @param {{ context?: Function }} session the v2 session domain */
async function lastAssistantTextFor(session, sessionID) {
  if (!session || typeof session.context !== "function" || !sessionID) {
    return "";
  }
  var list;
  try {
    var bounded = Promise.withResolvers();
    var timer = setTimeout(bounded.resolve, RETAIN_TIMEOUT_MS);
    try {
      var res = await Promise.race([session.context({ sessionID: sessionID }), bounded.promise]);
      // The v2 client answers `{ data }`; a bare array is accepted too, so a
      // shape change costs nothing.
      list = Array.isArray(res) ? res : res && typeof res === "object" ? res.data : undefined;
    } finally {
      clearTimeout(timer);
    }
  } catch {
    return "";
  }
  if (!Array.isArray(list)) return "";
  for (var i = list.length - 1; i >= 0; i--) {
    var entry = list[i];
    var info = entry && typeof entry === "object" ? entry.info : undefined;
    if (!info || info.role !== "assistant") continue;
    var text = textOf(entry.parts);
    if (text) return text;
  }
  return "";
}

/** memory-wire's memory plugin: recall injected before each turn, the turn
 *  retained after it.
 *
 *  **OpenCode v2, not v1.** Until 2026-11-20 this file exported the v1 shape --
 *  `export default async (input) => ({...hooks})` -- and opencode 2.x refuses it
 *  at load with `PluginModule.LoadError: Plugin must export a default
 *  definition with an id and an effect or setup function`. The door was
 *  therefore *silently dead* on every 2.x install: the file is copied to disk,
 *  `connect` reports `wired`, and nothing runs it. The plugin is not listed by
 *  `opencode plugin list`, and its memory injection never happens.
 *
 *  The v2 contract (`opencode.ai/docs/build/plugins/migrate-v1`): a default
 *  object with `id` and `setup(ctx)`, hooks registered through `ctx` rather
 *  than returned as a string map. Local plugins must live in a **directory**,
 *  so the install path is `plugins/memory-wire/index.ts` -- a bare file now
 *  only loads if configured, and `connect` writes no such entry (see
 *  `src/connect_opencode.rs`).
 *
 *  The three hook ports that matter:
 *    `chat.message`                                    -> `session.hook("prompt")`
 *    `experimental.chat.system.transform`              -> `session.hook("context")`
 *    `event` + `session.idle`                          -> `ctx.event.subscribe()`
 *  In v2 the callbacks take **one mutable event**, not `(input, output)`, and
 *  `ctx.session` replaces the old `client.session`. Everything below the host
 *  boundary -- the caps, `post`, `formatRecall`, the trust gate -- is the same
 *  code as the extension door, unchanged.
 *
 *  The `id` is not cosmetic: plugin storage is scoped by it and diagnostics
 *  name it, so it has to stay stable across versions or a user's stored
 *  plugin state stops being reached.
 *  @param {{
 *    session: { hook: Function, messages: Function },
 *    event: { subscribe: Function },
 *    location: object,
 *    options: object,
 *    storage: object
 *  }} ctx the v2 plugin context */
export default { id: "memory-wire", setup: setup };

function setup(ctx) {
  var session;
  var event;
  var subscribe;
  // v2 groups these on the context; v1 put them on a `client` argument. Guarded
  // rather than destructured, because a plugin that throws inside `setup`
  // fails to load rather than failing to recall -- the door would be dead
  // again, which is the bug this port exists to undo.
  session = ctx && typeof ctx === "object" ? ctx.session : undefined;
  event = ctx && typeof ctx === "object" ? ctx.event : undefined;
  subscribe = event && typeof event.subscribe === "function" ? event.subscribe : undefined;
  var lastPrompt = "";
  var lastAnswer = "";
  var lastRetained = "";
  // A multi-step turn calls the hook once per step; one recall per distinct
  // query keeps step N from paying (or duplicating) step 1's recall.
  var injectedQuery = "";
  var injectedBlock = "";

  if (session && typeof session.hook === "function") {
    // The user message text for the turn in flight. The hook hands over the
    // event; reading its parts is the whole capture -- no fetch needed for
    // what arrives in the event.
    session.hook("prompt", function (ev) {
      var parts =
        ev && typeof ev === "object" && Array.isArray(ev.parts) ? ev.parts : [];
      var text = stripReminders(textOf(parts));
      if (text) lastPrompt = text;
    });

    // The injection point. Pushing onto `ev.system` reaches the model as
    // system content.
    session.hook("context", async function (ev) {
      if (!(ev && typeof ev === "object" && Array.isArray(ev.system))) return;
      var prompt = lastPrompt;
      if (!prompt) return;

      // A prompt this short cannot rank anything on its own: "Continue"
      // matches every memory equally and the top of that tie is noise. The
      // query borrows the previous turn's answer, so what ranks is the work
      // being continued, with the prompt still steering inside it. A
      // mechanism, not a filter -- nothing is cut on a score
      // (docs/OPEN_HOOK_RECALL_RELEVANCE.md).
      var query = prompt;
      if (prompt.length <= SHORT_PROMPT_CHARS && lastAnswer) {
        query = clamp(lastAnswer, QUERY_CONTEXT_CHARS) + "\n" + prompt;
      }

      if (query === injectedQuery) {
        if (injectedBlock) ev.system.push(injectedBlock);
        return;
      }
      var items = await post(
        "/banks/" + encodeURIComponent(bank()) + "/recall",
        // `format: "full"` is what puts `score` on the wire; the default shape
        // is a bare array of content strings. `formatRecall` reads both, so
        // this stays compatible with a server that does not know the field.
        // `MEMORY_WIRE_FORMAT=summary` asks for H8 capsules instead of whole
        // memories. It comes from the environment rather than the bank's
        // `injectionFormat` config key because this file already reads
        // `MEMORY_WIRE_URL` and `MEMORY_WIRE_BANK` from there, so it costs no
        // extra request; reading the config key would add one per turn, which
        // `src/hooks.rs` documents as already costing one. The Rust hook reads
        // both and the bank wins there. Anything but "summary" reads as default.
        {
          query: query,
          budget: RECALL_BUDGET,
          format:
            (process.env.MEMORY_WIRE_FORMAT || "").trim().toLowerCase() === "summary"
              ? "summary"
              : "full",
          exclude_tags: EXCLUDE_TAGS,
        },
        RECALL_TIMEOUT_MS
      );
      var block = Array.isArray(items) ? formatRecall(items) : "";
      // Only cache a recall that SUCCEEDED. A failed `post` yields a
      // non-array, so `block` is "" -- and caching that under `query` meant
      // the next turn with the same prompt hit the `query === injectedQuery`
      // short-circuit and injected nothing, forever, until the user happened
      // to type something different. A transient outage froze recall for that
      // query. (Found by the round-5 review of this file, 2026-11-20, which
      // read the failure path this rewrite had never exercised: the rewrite
      // verified that the door LOADS and that injection works, not what
      // happens when the server is down.)
      if (Array.isArray(items)) {
        injectedQuery = query;
        injectedBlock = block;
      } else {
        // Drop the cache rather than keeping it. Clearing `injectedQuery` means
        // the next turn with the same prompt re-asks, so a recovered server is
        // picked up on the very next turn instead of after the user retypes.
        injectedQuery = "";
        injectedBlock = "";
      }
      if (block) ev.system.push(block);
    });
  }

  if (subscribe) {
    var controller = new AbortController();
    void (async () => {
      try {
        for await (var ev of subscribe({ signal: controller.signal })) {
          // Turn end. `session.idle` still exists in 2.3.2 and still carries
          // `{sessionID}`, but it does NOT fire under `opencode run` --
          // measured, over a real turn, 31 other event types delivered and
          // idle not among them. `session.execution.succeeded` does, and it
          // is the narrower claim anyway: the execution completed rather
          // than the UI going quiet. Verified live as
          // `{"type":"session.execution.succeeded", ...}` with the
          // `sessionID` on the event, not in `properties`.
          if (!(ev && typeof ev === "object" && ev.type === "session.execution.succeeded")) {
            continue;
          }
          var sessionID = typeof ev.sessionID === "string" ? ev.sessionID : "";
          var prompt = lastPrompt;
          // Cleared before the await: a turn that starts before this one
          // finishes must not have its prompt consumed by the earlier
          // retain.
          lastPrompt = "";
          var answer = await lastAssistantTextFor(session, sessionID);
          // The next turn's short-prompt query composition borrows this
          // answer.
          lastAnswer = answer;
          // An unanswered or bare-acknowledgement turn is a question with no
          // knowledge in it: retaining prompt-only rows is how the bank fills
          // with duplicate question texts that recall then injects as if they
          // were memory. The pair is the memory.
          if (!prompt || !answer || answer.length < MIN_ANSWER_CHARS) continue;

          var content = [
            "asked: " + clamp(prompt, RETAIN_PART_CAP_CHARS),
            "answered: " + clamp(answer, RETAIN_PART_CAP_CHARS),
          ].join("\n");
          // An auto-retry re-submits the same turn; the server dedupes
          // identical content anyway -- but not before paying for the write.
          if (!content || content === lastRetained) continue;

          // `document_id` keyed on the prompt makes a repeated ask replace
          // its row (the server's default update mode) rather than add a
          // twin.
          var stored = await post(
            "/banks/" + encodeURIComponent(bank()) + "/retain",
            { content: content, document_id: "turn-" + hash(prompt) },
            RETAIN_TIMEOUT_MS
          );
          if (stored !== null) lastRetained = content;
        }
      } catch (_e) {
        // The connection closing is the harness going away, not a memory
        // fault. Never break the agent: nothing here propagates.
      }
    })();
  }

  // v2 replaces the v1 `dispose()` hook with whatever `setup` returns. Only
  // what this plugin owns -- the event subscription -- needs undoing; the two
  // session hooks are scoped and unloaded with the plugin.
  return function cleanup() {
    if (controller) controller.abort();
  };
}

# SECTION H-REGISTER

# FILE: TODO.md (lines 20,56)

## Open

R&D rows (6, 9, 10, 11, 12, 20) are sequenced in `docs/handoff/RND_PLAN.md` —
§5 the closed survey cycle, **§6 the upcoming cycle's outline**; start there.

| # | status | item | why | where | acceptance |
|---|---|---|---|---|---|
| 1 | done | ~~`connect` preserves a surface's baked bank when `--bank` is absent~~ — **done 2026-10-15** (shipped v0.8.0): `apply` reads the pin from the host's own config before any edit — hooks from the command strings, MCP from the argv, extensions/plugins from the installed file's `DEFAULT_BANK` — and rewrites with it; only a config with no pin anywhere stays runtime-resolved (`src/connect.rs`, `bank_from_extension`; tests: bare-rewire-preserves-pin on hooks + MCP + no-pin cases) → [2026-10-15] `src/connect.rs` | kills the installer's `--no-connect` dance on pinned machines (README §Installation documents it); today a bare re-connect silently re-defaults to `memory-wire` | `src/connect.rs` + test | ~~against a temp `HOME` pinned to a second bank, a bare `connect <host>` re-wires with the same bank, unchanged~~ — asserted |
| 2 | done | ~~`doctor` reports a serve running an older binary than the installed one~~ — **done 2026-10-15** (shipped v0.8.0): `Report.stale_serve` — reads the daemon state file's pid, `/proc/<pid>/exe` (the `(deleted)` marker stripped), and **byte**-compares against the running binary; a copy at a different path is not stale. Renders the finding with the restart command; quiet when down, absent state, or identical bytes (`src/doctor.rs`; 3 tests incl. the byte-identical-copy case) → [2026-10-15] `src/doctor.rs` | nothing today detects the "serve keeps old behavior until restarted" window — the exact gotcha from the v0.7.0 deploy on both machines; the running binary outlives the swap by an unbounded time | `src/doctor.rs`, tail of `install/get-memory-wire.sh` | ~~a stale serve produces a finding line naming the restart command; a current serve produces none~~ — asserted |
| 3 | done | ~~`memory-wire update` subcommand~~ — **done 2026-10-15** (shipped v0.8.0): `update` drives `gh` (the binary stays dependency-free plain-HTTP; a missing `gh` refuses with the install hint) — newest-tag compare, `gh release download`, **sha2 in-process verification before anything is touched**, extract, stop-if-live, atomic swap keeping `<exe>.old` for a one-command rollback, start-on-the-new-binary, bare `connect` (pins preserved via #1). Order asserted over a recording fake; live network never in tests (`src/update.rs`; 7 tests). MW_REPO/MW_ASSET overrides → [2026-10-15] `src/update.rs` | one command for check-latest → sha256-verify → swap → re-connect installed hosts (banks preserved via #1) → serve restart (via #2); today all five update steps are manual and one of them (hermes) is invisible to the installer entirely | `src/main.rs`, composing `connect.rs`/`doctor.rs`/`daemon.rs` | ~~end-to-end test against a fake releases API served from a temp dir; the live network check stays behind a flag and never runs in tests~~ — asserted over the effects seam; live mode is `gh`-gated and smoke-verified against the real repo |
| 4 | done | ~~INTEGRATION_PLAN Phase 0, starting with P0.1~~ — **re-verified 2026-10-15 against the binary**; the register is now corrected in place. P0.1 **done** (`paths::endpoint()` reads `serve.json` behind `MEMORY_WIRE_URL`, `src/paths.rs`, 5 regression tests). P0.2 **resolved as keep-8888 + probe** (`daemon start` binds-or-refuses; `connect` takes no probe by decision). P1.1 **done** (`src/connect_codex.rs`). P1.2 **done 2026-10-15, both halves** (`src/connect_claude.rs` for `~/.claude.json`; `src/connect_copilot.rs` for `~/.copilot/mcp-config.json` — a user definitions file whose schema was verified from GitHub's own Copilot CLI reference and the github-mcp-server install guide, so the earlier "JSONC, unverified" blocker is discharged). P1.3 **done 2026-10-15** (`edit_mcp` refuses a foreign server under our name in both directions; claude URL-identity ownership). P2.1 **done 2026-10-15** (`connect::tests::golden_install_shapes`, `src/connect.rs` — a pristine `HOME` plus one `connect` must equal the golden bytes of every file the host parses, seven hosts; shape drift fails at the diff). P2.2 **done 2026-10-15** (`doctor --all-hosts` + G6.9 liveness). P3.1 **recorder done 2026-10-15** (DT_NEEDED row in `bench_footprint`, `readelf -d`); the v0.5.0 byte-compare archaeology stays open. P4.1 **done 2026-10-15** — the slot binds by *directory name* per hermes' own `find_provider_dir`, so a directory stripped to residue is not `wired` even though it exists (`src/connect.rs:535-541`; the audit-era state was a dead slot, the live one empty, and the false-positive `wired` check is fixed). P4.2 **done** (this correction) — → [2026-11-19] re-verified against the binary: every item above is closed by the `file:line` in this cell, and the only residue is the v0.5.0 published-asset byte-compare, which is archaeology rather than work (batch 1, `e0f1bb4`; CONSISTENCY §54). The evidence cell used to hold a to-do for a future agent, which is what E003 was reporting. | re-verify each item against the `v0.7.0` tree first — the plan was written 2026-09-30 and its blocking decisions may have been answered by the retrieval-efficacy waves | `docs/INTEGRATION_PLAN.md` | ~~the plan's own gates~~ |
| 5 | done | ~~GAP-8: name the `hook:stop` marker producer~~ — **closed 2026-10-15 as stop-looking, by evidence, not fatigue**: the producer was characterized 10/10 probe runs (execs the installed binary's `hook stop` by absolute path, once per opencode session end, ~15 ms after the plugin's idle retain) and is absent from every visible binary, config and package on the box; further identification means syscall-tracing live sessions, out of all proportion to a residual risk that is already neutralized (its markers are auto-tagged and excluded from auto-injection since v0.7.0). The 2026-10-08 addendum in `docs/INTEGRATION_AUDIT.md` is the record; a name arriving by accident is the only thing that would reopen it → [2026-10-15] `docs/INTEGRATION_AUDIT.md` | effect already closed — it execs the installed binary, so its markers are tagged and auto-excluded since `v0.7.0`; only the identity is unknown (absolute-path invocation, absent from every visible binary, config and package on the box) | start at the 2026-10-08 addendum in `docs/INTEGRATION_AUDIT.md` | ~~a named producer with evidence, or a recorded decision to stop looking~~ — the recorded decision is this row |
| 6 | done | ~~dev-set growth before any score gating~~ — **done 2026-10-16**: the battery is frozen (`eval/relevance/freeze_battery.py`; 1,429-row snapshot sha256 `c780939b…` gitignored, seed → deterministic 1,413-row bank) and grown 10 → 21 (11 verified turn-row harvests); frozen marks gold 16/17 auto, junk 0/9, search-reachable 17/17; D-case re-measured — passes under the shipped config; H8 gate re-confirmed. Precision levers now registrable (one at a time, junk@5 down / gold@5 flat, never a threshold — CONSISTENCY §32/§48) → [2026-10-16] `eval/relevance/freeze_battery.py` | the standing decision is tag hygiene, no threshold (CONSISTENCY §32); gating may only be revisited after the labelled set grows well past today's 10 cases — and the D-case judgment (cross-project turn rows ranking in a single-user bank) is the first thing to re-measure | `eval/relevance/`, `docs/EVALUATION_HYGIENE.md` | a gated change carries its dev-set numbers and states its fitted-ness, per AGENTS §6 |
| 7 | done | ~~PrecisionMemBench precision: 0.17 at recall 0.95 (14/77)~~ — done 2026-10-14 (shipped v0.8.0): the adapter ingests dataset tags and honours the filter's `any` scope groups via the store's include-any recall; **19/77, precision 0.24, recall unchanged 0.95** (`eval/amb/PRECISIONMEMBENCH-2026-10-14-h6-filters.json.gz`, `CONSISTENCY.md` §37). The number moved because of the provider capability change, not ranking (filters are pre-fusion SQL narrowing). `narrow_any` identity clauses remain unhonoured (Hindsight-native fuzzy tag resolution), so misspelled-name cases still over-return — residue named, no threshold picked off this 77-case table → [2026-10-14] `eval/amb/PRECISIONMEMBENCH-2026-10-14-h6-filters.json.gz` | first independent public measurement of the over-return `docs/OPEN_HOOK_RECALL_RELEVANCE.md` diagnosed; 563 of 567 failures are an unexpected belief. Candidate levers must be argued by mechanism (e.g. the provider honouring the dataset's caller-supplied `scope:` tags, which memory-wire can already filter on) and decided on the dev set — never a threshold picked off this 77-case table | `eval/amb/memory_wire.py`, `src/recall.rs`; `docs/BENCHMARK_LANDSCAPE.md` §5.1 | ~~a change decided on LoCoMo/relevance, then PMB re-run once and committed as a new `eval/amb/PRECISIONMEMBENCH-*.json.gz`, whatever it shows~~ |
| 8 | done | ~~LongMemEval: emit the 470-question (abstention-excluded) slice~~ — done 2026-10-13: `examples/longmemeval.rs` prints the row; first reading 84.0% R@1 / 97.2% R@5, n 470 (`eval/baseline/2026-10-13-v0.7.0/LONGMEMEVAL.md`) → [2026-10-13] `eval/baseline/2026-10-13-v0.7.0/LONGMEMEVAL.md` | upstream's retrieval protocol skips the 30 `_abs` questions; ours and agentmemory's both score 500 (agentmemory's filter tests `question_type` and never matches), so neither number sits beside the paper's tables | `examples/longmemeval.rs` (the generator, not a hand computation) | `eval/RESULTS.md` carries both the 500 and the 470 row, from one run, counted as one consultation |
| 9 | done | ~~turn-level gold for BEAM (and/or PersonaMem) → a second dev set~~ — **done 2026-10-16**: pair-level labels for BEAM-100k (`eval/beam_label.py` + digest `eval/beam_labels_100k.json`; 313/400 resolved, 13/15 review agreement) and the pool-scale re-test (`eval/BEAM.md`, CONSISTENCY §47) — **stream not deleteable** (0.25 > 0.00 everywhere; 0.00 loses R@pool 100→95.5%), **unfitted 1.00 leads 0.25** (R@5 +3.5pp, opposite sign to LoCoMo's +1.4pp) — weight question stays open and user-gated → [2026-10-16] `eval/BEAM.md` | AMB's copies cannot be scored retrieval-only: BEAM gold ids never resolve (0 of 1,800 at 100k/500k/1m), PersonaMem's gold is the whole bank. A labelled BEAM would add 2,000 questions over 27–50-doc banks — the pool size LoCoMo under-measures (CONSISTENCY §14.2) — and feeds #6 | `eval/download.sh`, a new example; upstream `mohammadtavakoli78/BEAM` | labels derived from upstream source-turn data, not hand-picked; resolve rate reported | and gold/bank share reported as in `docs/BENCHMARK_LANDSCAPE.md` §3 |
| 10 | open | agent-memory-bench (GiulioDER) Claude Code arm | the only external benchmark that grades the product's actual job — task success with hooks+MCP wired — and it has a harm suite (superseded/contradictory/adjacent) | an `adapters/memory_wire/` in their repo, wiring `memory-wire connect claude-code` | needs the user's agent-model budget; result reported as measure-only beside their `claude_md` baseline |
| 11 | open | LLM-judged runs: AMB `rag` (BEAM/PersonaMem/LifeBench), OAMB LME-60, memorybench ConvoMem, MemBase RealMem/MobileMem | answer-level evidence on corpora we have nothing on; OAMB's answer-visible-token axis is where a budget-trimmed recall should show | `docs/BENCHMARK_LANDSCAPE.md` §4, §7 | user-approved budget first; OAMB and every LME run counted as a test consultation |
| 12 | open | AMB provider writes at the product's granularity (turn pairs, as the hooks do), not 25k-token sessions | with whole-session documents, AMB's top-10 is the entire bank on BEAM 100k / PersonaMem 32k and ~250k tokens on BEAM 500k/1m — those corpora measure the reader's context window, not memory-wire (`eval/baseline/README.md`). Decided by mechanism (mirror the real write path), not by score | `eval/amb/memory_wire.py` | BEAM / PersonaMem / LifeBench added to `run_baseline.sh llm`; top-10 tokens per question reported, as in the README table. **Measured cost, 2026-10-14:** on LoCoMo (dev) turn-level writes pooled to sessions (`locomo --turns`, `eval/LOCOMO_TURNS.md`) lose to whole sessions on every aggregate — `recall_all@10` 83.0% → 80.7%, R@1 60.4% → 54.6%, n 1531 — and windows of W=2 turns lose too (`eval/LOCOMO_WINDOWS.md`: R@1 59.3%, `recall_all@10` 82.6%, R@pool 98.7% — windowing passes the candidate-pool ceiling). Every granularity finer than the session measured so far is worse; H2 is closed. This row still buys measurability of the long corpora at a ranking cost, and is not a ranking lever |
| 13 | open | aroute: 200-with-`{"error"}` logged `outcome=ok` with no failover (B1); cline replies forwarded inside a `{"success","data"}` envelope (B2); cline token/cost headers always 0, so spend limits miss cline (B3); a 403 provider gets `cooldown_ms=0` and is retried on every request (B4) | B1/B2 break every OpenAI client; `eval/llm_shim.py` papers over them and should be deleted once aroute is fixed | `agentic-utility/proxy/artificial-route` (a different repo) — handoff in **`docs/AROUTE_BUGS.md`** | in `llm-calls.jsonl`: no `enveloped: true`, no retries, non-zero `tokens_in` for cline |
| 14 | done | ~~re-measure `bench_concurrency` on a quiet box~~ — **done 2026-10-14, PASS** (shipped v0.8.0): load 0.98→4.33, **1.85×** at 64 clients vs the 1.50× floor, RSS flat 12.9 MB, 0 panics (`eval/baseline/2026-10-14-bench-concurrency/BENCH_CONCURRENCY.md`, `CONSISTENCY.md` §40). The 2026-10-13 failure (1.30×) was load noise at 17→22 — the baseline artifact stays as the dated record of that regime → [2026-10-14] `eval/baseline/2026-10-14-bench-concurrency/BENCH_CONCURRENCY.md` | the 2026-10-13 baseline failed the 2-client scaling gate (1.30× vs 1.50×) at load 17→22 while absolute ops/s rose against the committed artifact — unreadable as a regression or a pass | `examples/bench_concurrency.rs` | ~~one run with `loadavg` < 4, recorded either way~~ |
| 15 | done | ~~an occurrence time on HTTP `retain` (and in `format: full` recall)~~ — done 2026-10-14 (shipped v0.8.0): HTTP and MCP retain take `created_at` (RFC 3339 or `YYYY-MM-DD`, UTC-normalized, else the frozen 400), `full` recall serves it, and `eval/amb/memory_wire.py` round-trips `Document.timestamp` through it; tests in `src/main.rs` and `src/mcp.rs` → [2026-10-14] `src/main.rs` | a memory's *event* date cannot round-trip through the product: `retain` has no field for it and `full` recall returns no date, so LoCoMo/LongMemEval temporal questions reach a reader undated; `eval/amb/memory_wire.py` and `examples/answer_quality.rs` have to supply or omit it client-side (found 2026-10-13, `docs/CONSISTENCY.md` §34) | `src/api.rs` retain/recall bodies; `Memory.created_at` already exists in the store | an AMB document's `timestamp` round-trips through retain → recall with no client-side table |
| 16 | done | ~~`answer_quality` passes no session dates to its reader~~ — done 2026-10-14 (shipped v0.8.0): harness fixed (sessions at `haystack_dates`, served memories prefixed `[<date>]`, `question_date` in the prompt; ranking untouched) **and re-measured once**: dated instrument reads **18.6%** conditioned (93/499 scored, 1 unscored), closed-book 6.0%, delta +12.6pp — beside the undated 17.0% as a different instrument, no retrieval conclusion drawn (`CONSISTENCY.md` §39, `eval/baseline/2026-10-14-v0.7.0-dated-answer-quality/`; test consultation #3) → [2026-10-14] `eval/baseline/2026-10-14-v0.7.0-dated-answer-quality/` | LongMemEval ships `haystack_dates` / `question_date`; without them temporal-reasoning (5.3%) and knowledge-update (14.1%) are close to unanswerable by construction. Fix the harness, then re-measure once — a harness fix, not a retrieval change (§34) | `examples/answer_quality.rs` | ~~the reader's prompt carries each session's date and the question date~~ |
| 17 | done | ~~served-context precision for small readers~~ — **closed 2026-10-14 as not reproduced** (shipped v0.8.0): H4 built the dev set this row demanded — fixed 200-q LoCoMo subset, every gold inside the shipped top-3 (`eval/h4_subset.json`, harness `examples/locomo_h4_subset.rs`), served at k = 3/5/10 via the adapter's `MEMORY_WIRE_TOPK` cap — and the drowning failure did not reproduce: judge accuracy **92.0% / 94.0% / 94.0%** at median context **4.3k / 7.2k / 14.6k** tokens, zero refusal-styled wrong answers at any k (`eval/amb/h4_locomo_rag_topk{3,5,10}.json.gz`, `CONSISTENCY.md` §38). Serving 3.4× the context at fixed gold coverage moved nothing the judge can see, so the LME 10→20 dip attaches to that instrument's 0.5B reader, not to memory-wire's served set. No precision lever was ever built; per AGENTS §6 none was ever selected on the LME observation either → [2026-10-14] `eval/h4_subset.json` | 2026-10-13: on 10 LongMemEval refusals the free reader answered from the gold session alone 6/10 and from gold + 2 other sessions 1/10, with gold ranked first in 419 of 454 served contexts — ordering is solved, *what else is served* is what a small reader drowns in. Same failure shape as PMB's precision 0.17 (row 7) and the turn-granularity row 12. Select any lever on LoCoMo / the battery, never on this LME observation (§34) | recall budget/cap composition, turn-level writes | ~~a dev-set (LoCoMo AMB `rag`) accuracy gain at equal or better R@k~~ — the dev set was built and the failure is absent from it; reopens only with a *new* dev set that exhibits the failure |

| 30 | done | ~~read-time trust gate on auto-injection~~ — **done 2026-11-19, and it exposed a deeper defect**: `Trust::tier` accepted three tiers and mapped `untrusted_external` to `agent_inferred`, so the tier the gate refuses could never be *written* — a gate built on it would have been dead code with every test green. Fixed the ladder (`UntrustedExternal` added below `AgentInferred`, `downgrade` bottoms out there), put `trust` on the recall wire, and dropped that tier from `hooks::recall_section` **before** the `MAX_LINES` take so a poisoned rank-0 row cannot cost a trusted row its slot. An explicit `memory_recall` still returns those rows: a reader who asked consented. The same hole is closed in the hermes plugin's `prefetch` (`UNTRUSTED_TIER`). Load-bearing proof: removing the filter fails four tests, including a full-chain test through the real router → [2026-11-19] `src/api.rs`, `src/hooks.rs`, `plugin/integrations/hermes/memory-wire/__init__.py`; `CONSISTENCY.md` §51 | round-2 review's strongest finding (its §4a, verified): W2 only downgrades on write, so a row marked `untrusted_external` was injected into context verbatim. Not a relevance threshold — a deterministic allow/deny on a stored tier with no fitted constant, so CONSISTENCY §32/§48 (no score threshold, ever) does not apply | `src/api.rs`, `src/hooks.rs`, hermes plugin | ~~an untrusted row is absent from the injected block while an explicit recall still returns it; no score threshold introduced~~ — asserted, incl. a full-chain test and a negative check |
| 31 | open | door parity: hygiene has no MCP/CLI door, supersede has no HTTP route, grounding has neither. **Narrowed 2026-11-20**: the widest gap (redact had no MCP tool) is **closed** by row 34 — `memory_redact` is the 6th tool. **The round-5 review corrected this row's own text**: it said redact had "no MCP tool or HTTP route" and there has never been an HTTP redact route either; the row conflated `MemoryService` with a route. What remains is absence, not lockout. Round 4 also confirmed the same review's other structural readings and **refuted one**: hermes `is_available()` returning True on a down server is documented and deliberate (a probe would put a 2 s timeout in front of every Hermes init, `plugin/integrations/hermes/memory-wire/__init__.py:447-458`), not a silent fail-open | round-2 review's sharpest structural finding (its §3a, verified): "one service, three doors" is true of `MemoryService` and false of parity — every door exposes a different subset, so a capability can be unreachable from the surface an agent is actually on. Recorded with the verified table in `docs/INTEGRATION_GAPS.md` (2026-11-19 addendum); not silently broken, just absent | `docs/INTEGRATION_GAPS.md` addendum; `src/mcp.rs`, `src/api.rs`, `src/main.rs` | a surface decision, not a bug fix: either a documented ruling that the doors stay asymmetric, or redact on MCP — the widest gap, since an agent that retained a secret through MCP cannot retract it through MCP |
| 32 | done | ~~recall failures are indistinguishable from an empty bank~~ — **done 2026-11-19**: `recall_at_fmt` returned `None` for three different reasons — connection refused, non-2xx, and a 200 body this build could not parse — and all three produced byte-identical silence (exit 0, empty stdout). Each now names itself on stderr via `tracing::warn!`, so a hook that recalled nothing says which of the three happened. `RUST_LOG`-gated and stderr-only, so §9's never-fail contract is intact and `RUST_LOG=off` is total silence. The empty-query path stays silent because it is not a failure → [2026-11-19] `src/hooks.rs`; `CONSISTENCY.md` §52 | round-2 review had this as "empty framing violates AGENTS §10"; its verification pass refuted that and named the real defect — the agent and the user cannot tell "no memories matched" from "the server is down". §9 (never-fail silence) and §10 (an empty result says so) genuinely conflict here; §10 governs output that claims to be an answer, and a hook's empty stdout claims nothing | `src/hooks.rs`, `recall_at_fmt` | ~~each failure mode produces a distinct stderr line, none reaches stdout, exit stays 0, `off` is silent~~ — observed live against three real servers, plus a test pinning the modes differ and stdout stays empty |
| 33 | done | ~~the remaining-work plan's four batches~~ — **all done 2026-11-19** (register corrections `e0f1bb4`; hermes trust-gate proof `db57850`; dates on three doors `0b518de`; capsule knob on the plugin surfaces `1f93bf0`). Batch 1 corrected four stale open rows (P2.1, P4.1, P3.1, G6.8) whose work had already landed. Batch 2 gave `Trust::UntrustedExternal`'s fourth door an executable check. Batch 3 served `created_at` in hermes and both TypeScript plugins, which requested `format: "full"` and dropped the field. Batch 4 decided the capsule knob by cost: the plugin surfaces read `MEMORY_WIRE_FORMAT`, not the bank config key, because the env costs no request and the config would add one per turn → [2026-11-19] `plugin/integrations/`, `docs/`, `CONSISTENCY.md` §54–§57 | the round-3 context-pack review and the follow-up register audit kept finding one shape — a policy enforced in N places with a check in N-1, and a list that stopped being maintained. These four batches close that shape for the four doors and leave a runner that enumerates them | `docs/handoff/REMAINING_WORK_PLAN.md` | ~~every door honours the same serving shape and is covered by the same runner; every claim in the register matches the tree~~ — asserted, with per-door mutation checks and live runs |
| 34 | done | ~~door parity, widest gap only: **redact is reachable from the CLI but not from MCP**~~ — **done 2026-11-20** (`memory_redact` is the 6th tool): an agent that retained a secret through MCP can retract it through MCP. `MemoryService::redact_memory` already existed and the `read_only` guard already covered it, so the work was a tool registration, a dispatch arm and three tests. **Corrected by the round-5 review**: this row first said "CLI and HTTP" and there is no HTTP redact route -- the row conflated `MemoryService` with a route. The test deliberately uses content that *survives* the retain-side PII redactor — `sk-live-deadbeef` is caught at retain time by the OpenAI rule, so a test built on it would pass a tool that never had to work. Mutation-checked: a fake implementation returning `rows = 1` without writing fails the test. Tool counts corrected in `AGENTS.md` §4 + §3, `README.md` ×3, `src/main.rs`, `src/mcp.rs` (HISTORIAN doc, two `four tools above`), `src/mcp_http.rs`, `docs/INTEGRATION_GAPS.md` ×2 → [2026-11-20] `src/mcp.rs`, `src/mcp_http.rs`, docs; `CONSISTENCY.md` §65 | the one door parity hole where the *retraction* of a credential was impossible from the surface an agent is on; the other doors' asymmetries (hygiene has no CLI, supersede has no HTTP) are absence, not lockout | `src/mcp.rs` (`McpServer` tool list), `src/api.rs` `redact_memory`, `src/store.rs:2242` | ~~an MCP client can redact a row it can read, and the tool count test expects six~~ — done, with the live surface test and the readOnly refusal both passing |
| 35 | done | ~~the shipped opencode plugin does not load on opencode 2.x~~ — **done 2026-11-20** (`3376b5c`): found live while probing B2. The plugin shipped a v1 bare-function export (`PluginModule.LoadError` on 2.3.2, which requires `{id, setup}`) and was installed as a flat file, which opencode 2.x skips (it needs a subdirectory with `index.ts`). Both fixed, the version marker moved 2→3, legacy flat files are removed on install and uninstall, and a stub-ctx drive proved injection and the retain round-trip. `session.idle` does **not** fire under `opencode run`, so the capture hook is `session.execution.succeeded`. This was the same door the round-4 review looked at and called healthy — it was dead on the host it named | a door that has never loaded on its named host is worse than a missing door: `connect` reports success and the host silently has no memory. The review's trust-gate work correctly covered what reaches a model and could not see that nothing did | `plugin/integrations/opencode-plugin/memory-wire.ts`, `src/connect_plugin.rs`, `src/connect.rs`, `plugin/integrations/trust_gate_test.sh` | ~~opencode 2.x loads the plugin, the prompt hook injects, the retain hook writes~~ — live on 2.3.2, plus an export-shape check in the trust gate |
| 36 | open | the TypeScript doors (extension/omp, opencode) still carry a hand-copied ~100-line domain core, and the opencode port changed the file enough that a straight extract is now the wrong first move. Probing found both hosts resolve relative imports *if the door is a directory* — omp loads every flat `.ts` as an extension and errors on a helper (`does not export a valid factory function`), while `index.ts` in a subdirectory loads cleanly. So extraction is feasible but it means restructuring both installs, and the hermes door is a Python package with its own packaging. Option 1: concatenate the shared core via `include_str!` (no host resolves it, no restructure). Option 2: subdirectory + relative import. Option 3: a golden-file drift test that fails when the cores diverge, costing nothing and proving nothing about correctness | measured, not assumed: 101 lines of genuine duplication (markers, cap derivation, tier handling), and the drift is a real maintenance cost — a change to the cap shape must be made in two places or the doors diverge silently | `plugin/integrations/hermes/memory-wire/__init__.py`, `plugin/integrations/extension/memory-wire.ts` | a mutation in one door's core is caught in the other, or the core exists in exactly one place |
| 37 | open | **cross-door blank-content handling differs**: the TS doors skip blank/whitespace rows before the take (`extension/memory-wire.ts:241`, `opencode-plugin:213`), while `src/hooks.rs` and `hermes/__init__.py` count them against the cap and render `- ` for them. **Verified as latent, not live**: `src/api.rs:760` rejects blank content on every retain path, so this build cannot write one — but the schema is `content TEXT NOT NULL` with no `CHECK`, so an older build or a raw `sqlite3` insert can. Fix is to skip blank rows in Rust and Python too, matching TS. Not done yet because a change made on a review's mention, with no reproducing case, is a change made on assertion (AGENTS §8); a `CHECK` constraint or a test that inserts a blank row through SQL would make it reproducible, and then it is one line | round-5 review (2026-11-20) found the divergence by reading all four auto-injection doors side by side; narrowed by me with the write-path check. Four doors implementing one policy in three languages is the shape where this recurs | `src/hooks.rs` (`recall_section`), `plugin/integrations/hermes/memory-wire/__init__.py` (`prefetch`), `src/api.rs:760`, `src/store.rs` schema | ~~all four doors agree on blank rows~~ — Rust + Python skip blank content before the take, and a test inserts a blank row through SQL to prove the path is reachable |

| 38 | open | **the enumerated cross-door parity check both review rounds asked for** — rounds 4 and 5 independently reached the same instrument from different evidence: a check *sourced from `Host::ALL`* in `src/connect.rs` that asserts every host wires the same hooks/MCP/env contract, and a cross-door suite that drives `hooks.rs`, hermes, extension and opencode against one fixture set (failed-then-recovered `/recall`, repeated prompts, blank content, section-cap overflow, `format=summary`). Round 5's argument for this over a shared abstraction: 52% of `connect_*` is inline tests that cannot be deduplicated without obscuring golden expectations, only 1,793 production lines sit in the six per-host adapters, and a Rust trait cannot see host-side load failures like the opencode one. Round 5's falsifier: if all four doors and all declared host layouts pass such a suite with zero silent-dead-door failures across a release cycle, the verification bottleneck is closed | two independent reviews converging on the same instrument is the strongest signal in `research/audits/` — stronger than either round alone. The `trust_gate_test.sh` extensions in this session are a partial down-payment: they check export shape and the opencode cache rule, not the full matrix | `scripts/` (new, CI-only, 0 binary bytes), `src/connect.rs` (`Host::ALL`), `plugin/integrations/trust_gate_test.sh` | ~~a change to any one door fails a check unless every door got it~~ — a parity suite that enumerates doors from `Host::ALL` and drives each against one fixture set |



# SECTION I-GAPS

# FILE: docs/INTEGRATION_GAPS.md (lines 460,530)

## Addendum 2026-11-19 — the doors are asymmetric, and that is the honest state

Recorded because round-2 review read "one service, three doors" as a promise of
parity. It is not. It is a claim about `MemoryService`: there is **one**
implementation, reached through doors that each expose a **different subset**.
Verified per capability against this tree:

| capability | HTTP | MCP | CLI | hook |
|---|---|---|---|---|
| redact (W3) | no route | **yes** (6th tool, 2026-11-20) | **yes** (`Cmd::Redact`) | no |
| grounding (W5) | no route | no tool | **yes** (`Cmd::Grounding`) | SessionStart, default off |
| hygiene (W4) | **yes** (`GET /banks/:id/hygiene`) | no tool | no | no |
| recall | yes | `memory_recall` | no | yes |
| supersede (W7a) | no route | **yes** (5th tool) | no | no |

This is not a defect in any one door; it is what happens when each capability
was added through the door its own item needed. It is recorded here rather than
fixed because closing it is a surface decision, not a bug fix:

- **redact on MCP and HTTP is the widest gap.** Redacting a secret is a custody
  operation, and today the only way to reach it is the CLI on the machine that
  holds the store. An agent that retained a credential through MCP cannot
  retract it through MCP.
- **hygiene has no MCP or CLI door at all**, which the W4 record already notes
  as a deviation from the plan (`memory_hygiene` was the planned 5th tool;
  `memory_supersede` shipped instead).
- **supersede has no HTTP route**, so a non-MCP, non-local caller cannot retract
  a row.

None of these is silently broken. Each door that does not implement a capability
is simply absent, and the CLI's `--help` lists what the CLI can do. The risk is
a reader assuming a capability is reachable everywhere because the service is
shared.

### A related count, corrected

`src/mcp.rs` has seven `pub const …: &str = "memory_…"` constants, but only
**six** are tools: `HISTORIAN` (`memory_historian`) is a **prompt**, advertised
through `prompts/list` and served by `prompts/get`, and `tools()` does not
register it. The server exposes six tools and one prompt. Round-2 review read
six constants as six tools and concluded the README overcounted; the README's
prose ("five tools") was right at the time, and its table had been the thing
missing `memory_supersede`. `memory_redact` arrived 2026-11-20 (CONSISTENCY
§65), so the constants/tools offset is unchanged at one: the next count will
find seven constants and six tools, which is correct. The `HISTORIAN` doc
comment says this in place, so the next count does not re-add it.

### A fourth door asymmetry, found the same round: the trust gate was Rust-and-hermes only

`Trust::UntrustedExternal` was added in `1f5ee17` and both `src/hooks.rs` and the
hermes plugin gate on it. The **two TypeScript plugins did not**: both called
`formatRecall` on everything a recall returned, so an untrusted mount's web
content was injected verbatim into a model's system prompt. The weakest door
decided what the bank's untrusted rows could reach, and it was the two files with
no Rust test suite to catch it.

Fixed by carrying the same gate into both plugins, with the same evidence-on-
absence rule (a row with no `trust` field is kept, because a server old enough
not to send one is not evidence). Proof: `plugin/integrations/trust_gate_test.sh`
executes each file's real `formatRecall` in `node` and asserts that
`untrusted_external` is withheld while an absent field is not; removing either
gate makes it fail with `untrusted_external reached the block`.

Why this is recorded here rather than closed silently: three doors implementing
the same gate in three languages is already a maintenance surface, and
`measure_surface.sh` did not count `plugin/` at all — the count this repo uses to
describe its own surface stopped at `src/`, which is precisely how 1,622 lines
of injection-path code stayed unexamined. `measure_surface.sh` now reports
`plugin/` and `install/` **separately** rather than folding them into a total,
because a TypeScript plugin is not a Rust module and one misleading sum would
hide exactly that.
