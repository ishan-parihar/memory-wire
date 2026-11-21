# CONTEXT PACK — context-pack-2026-11-21-blockers

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


# SECTION B-DELTA

# FILE: research/audits/context-pack-2026-11-21-blockers/B-DELTA.md (lines 1,63)

# DELTA — what landed since the 2026-11-21 rulings round

Sixteen commits after `303a096` (the last rulings round's base). Verbatim
subjects, newest first; the round should read these as **done**, not proposed.

```
55994e3 docs: move a measurement date out of prose and into the register
7069325 Outline plan, third pass: the register is empty because I keep closing it
2155adf CONSISTENCY §75 and STATE: the v3 execution record
ed96bbd Redact gets its HTTP route, and that exposed a frozen-contract gap
c425f1e Teach the count gate to read src/, where the last stale count hid
1fcde7d Fix the live stale counts, and mark both superseded plans
6ab79d6 Outline plan for the remaining gaps, with the work log the question asked for
790987d STATE: full-scale execution recorded, including what was deliberately not run
b7bee48 CONSISTENCY §74: what shipped, what was measured, and the LongMemEval run I chose not to spend
59a2919 Close TODO 36 as a decision, and record TODO 38's block with its evidence
34237bf supersede gets an HTTP route and hygiene gets a CLI, closing the asymmetry
6df26dd Doctor names the input that produced an empty bank, not just the flag
d86d200 Outline plan for the remaining gaps, after checking the ground truth
303a096 Record the execution: round-trip test, count gate, parity ruling
3a4b1a6 The round-trip test: does a retained memory reach the injected block?
3050ab2 TODO 38: pin the row-inclusion invariant, which is what actually broke
```

**Three of these change what the ASK section can claim.**

1. **`ed96bbd` — redact got its HTTP route, and testing it found a
   frozen-contract gap.** `StoreError::BadInput` (a redact `pattern` that is not
   a regex) fell through `http_error`'s catch-all to an opaque `500 storage
   error`. It now maps to `400 invalid content`. Reachable before through the CLI
   and MCP, so it is a contract correction, not a new-route detail.

2. **`c425f1e` — the count gate now scans `src/`.** The defect that motivated it
   (`mcp_http.rs` saying "four tools" in three places while serving six) survived
   review because its test asserted against `Server::tools()` rather than a
   literal: the assertion was right, only the name and the docs were wrong.

3. **`59a2919` and `34237bf` — two TODO rows closed as decisions, not as work.**
   TODO 36 (extract the TypeScript door core) was closed as *deliberately not
   extracted* with the measured reason; TODO 38's remaining half is recorded as
   **blocked** with its evidence.

**Measured after all of it (CONSISTENCY §74 and §75):**

| run | result | load |
|---|---|---|
| LoCoMo dev (free) | R@5 86.0%, R@10 94.2%, NDCG@10 73.6%, MRR 71.8%, R@pool 100.0% | 23.71 on 24 cores |
| Relevance battery (free) | H8 gate held; ranking IDENTICAL; 42% fewer auto-injected chars (16,034 vs 27,766) | — |

Both are **unchanged within rounding** from the batch before, which is the
expected result for a batch that touched doors, a doctor line, docs and a gate,
and no ranking code.

**Footprint after all of it:** release build **10.73 MiB** (was 10.65 at the
start of today), `DT_NEEDED` still **3** (`libgcc_s`, `libm`, `libc`). No
dependency added. The competitive position in AGENTS §1 holds.

**Not run, both days:** LongMemEval-S. The dataset is present and the command
works; it is a counted consultation (count = **5**) and both days' changes could
not have moved a ranking number.

**The one thing that got worse:** the installed binary is still 0.9.0 and is now
**89 commits** behind `v0.10.0` (it was 68 when this session began and 87 at the last plan).

# SECTION A-ASK

# FILE: research/audits/context-pack-2026-11-21-blockers/ASK.md (lines 1,169)

# Ruling sheet — five decisions waiting on the operator

Each row: the ruling id, the options (yours marked), what deciding unblocks, and
what leaving it undecided costs. **No row carries my recommendation** — the
reviewer supplies that.

Operator-only facts are marked **[OPERATOR]**: the reviewer can answer "not in
evidence" about them and should.

---

## R1 — The release

**Question.** Cut a new binary release from `main`, or keep waiting?

**State.** `Cargo.toml` is 0.10.0. The newest tag is `v0.10.0`, cut 2026-10-27.
`main` is **89 commits** past that tag. The *installed* binary on the operator's
machine is **0.9.0**, dated Oct 20, and provably lacks `memory_redact`
(`git show v0.9.0:src/mcp.rs | grep -c memory_redact` → 0; the tree → 3).

**Options.**
- (a) Cut now from `main` — three-Linux set via `scripts/build-release.sh`.
  **[MINE: I lean here but have not been told to]**
- (b) Keep waiting until the remaining work is final.
- (c) Cut a patch release of only the fixes, leaving features unreleased.

**Deciding unblocks:** the hosts already wired to the installed binary can use
`memory_redact` and the doctor bank block; it also makes "newest release" and
"newest code" the same thing again.

**Deferral costs:** the drift between installed and tree keeps growing (89 → N),
and every host wired to 0.9.0 silently lacks capabilities. It is **not**
irreversible either way — a later cut supersedes an earlier one.

**Owner: operator.** AGENTS §11 reserves this to the user.

---

## R2 — The bank

**Question.** Move this host's hooks onto the bank that holds the memories?

**State.** `memory-wire doctor` on the operator's machine:

```
finding  bank `ishan-parihar-memory-wire` holds no memories; `omp` holds 4246.
         A hook here resolves to `ishan-parihar-memory-wire` and will recall nothing.
```

Every hook firing in that directory recalls nothing. `omp` holds 4,246 memories.
There are 9 banks on the store.

**Options.**
- (a) `memory-wire connect --bank omp` — forwards the pin to every `connect` it
  runs, including the `--uninstall` that unwires hosts. **Rewrites five live host
  configs** (`~/.claude.json`, `~/.copilot/`, `~/.cursor/`,
  `~/.config/opencode/`, `~/.hermes/config.yaml`). **[MINE: I lean here]**
- (b) Leave it, and set `MEMORY_WIRE_BANK=omp` per shell.
- (c) Migrate the memories into the resolved bank instead, and keep the current pin.

**Deciding unblocks:** hooks on this host actually recall, which is the product's
whole job.

**Deferral costs:** it is **silent** — a hook that recalls nothing looks exactly
like a hook that found nothing. It has been true since at least `bde4b6a`.

**Owner: operator**, because (a) edits five files outside the repo.

---

## R3 — agent-memory-bench

**Question.** Spend an answering-model budget on the Claude Code arm?

**State.** `docs/BENCHMARK_LANDSCAPE.md` calls it "the product question": the only
external benchmark that grades **task success with hooks+MCP wired** rather than
retrieval quality. It is **not cloned**, and needs an answering model plus
Docker. It is measure-only.

**Options.**
- (a) Fund it and run it. **[MINE: I lean here — it is the only thing that
  grades the actual product]**
- (b) Defer until the release and bank decisions land.
- (c) Drop it; the retrieval harnesses are enough.

**Deciding unblocks:** the first external evidence about the product's job rather
than its retriever.

**Deferral costs:** none technically. It is the largest remaining *evidence*
gap, not a correctness one.

**Owner: operator** — it spends money.

---

## R4 — The unadopted server

**Question.** Add a `daemon adopt` command for a server started outside
`daemon start`?

**State.** Measured today:

```
$ memory-wire doctor
  server     up
$ memory-wire daemon status
  state      not running
  pid        1371296  (no such process)
  state file /home/ishanp/.local/share/memory-wire/serve.json  (stale)
```

Both are correct. A server has been up four days (pid 1295, started outside
`daemon start`); the daemon's recorded pid (1371296, Oct 17) is gone, so
`serve.json` describes a server that no longer exists and nothing rewrote it.
`daemon` has Start/Stop/Status and **no `adopt`**, so that server can never be
managed. `doctor`'s `stale_serve_at` returns `None` when the recorded pid is
dead (`let running = exe_of(pid)?`), so the doctor stays quiet in exactly the
case where the state file is maximally wrong.

**Options.**
- (a) `daemon adopt` — make an unadopted server manageable. **[MINE: I lean
  against, as a proposal rather than a bug fix]**
- (b) Fix only the doctor's silence (name the stale state file + the unadopted
  server; offer nothing).
- (c) Do nothing; the operator restarts by hand.

**Deciding unblocks:** the tooling stops contradicting itself.

**Deferral costs:** `doctor` and `daemon status` keep disagreeing, and a reader
trusting either one is misled about whether memory-wire is running.

**Owner: split.** (b) is agent work; (a) and (c) are operator.

---

## R5 — The benchmark battery

**Question.** Run LongMemEval-S now as a regression check?

**State.** The dataset is present (277 MB, `eval/data/longmemeval_s_cleaned.json`)
and the command works. Running it is a **counted consultation** — the count is at
**5** (`docs/CONSISTENCY.md` §74). The last two batches changed doors, a doctor
line, docs and a count gate; **nothing touched ranking**, and both free harnesses
(LoCoMo R@10 94.2%, relevance battery H8 gate held, 42% less auto-injected) are
unchanged within rounding.

**Options.**
- (a) Run it now, as a release-boundary regression check. **[MINE: I lean
  against]**
- (b) Run it at the release, not before.
- (c) Never re-run; the retrieval core is closed.

**Deciding unblocks:** a test-set number on the current build.

**Deferral costs:** low — the two free harnesses already cover ranking, and §6's
rule is that a consultation is for measurement or release acceptance.

**Owner: operator** decides whether to spend a consultation.

---

## Operator-only facts, so the reviewer can say "not in evidence"

- The money available for R3.
- Whether the five host configs in R2 may be rewritten.
- Whether a release is wanted at all before the remaining work is final (R1).
- Whether the four-day-old server in R4 is load-bearing (it serves live traffic).

Do not infer these from the pack.

# SECTION C-STATE

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


# SECTION D-LAW

# FILE: AGENTS.md (lines 360,409)

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

## 12. Honesty rules for reports

- Report what a measurement shows, not what you hoped it would show.
- If an agent before you made a claim wrong, correct it by name.
- If you cannot determine which of two conflicting values is right, leave both
  and say so. Do not invent a reconciliation.
- A null result is a result. Record it.
- Verify before reporting. Run the gate yourself; do not cite a subagent's
  summary as if it were a measurement.


# SECTION E-LAW2

# FILE: AGENTS.md (lines 235,295)

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


# SECTION F-EVIDENCE

# FILE: docs/CONSISTENCY.md (lines 5610,5610)

## §74. 2026-11-21 — full-scale execution: what shipped, what was measured, what is blocked

# FILE: docs/CONSISTENCY.md (lines 5668,5668)

## §75. 2026-11-21 — v3 execution: three defects found by looking, one by testing

# SECTION G-ANALYSIS

# FILE: docs/plans/2026-11-21-remaining-gaps-plan-v4.md (lines 1,186)

# Outline plan: remaining gaps, cleanup, and upgrades — third pass

**Status:** proposal, 2026-11-21 (third pass). Written from `2155adf`, 87 commits
past `v0.10.0`.

Two previous plans were executed in the same day (`...-plan-v2.md`, `...-plan-v3.md`;
both now carry supersession headers). This pass was written the way the last two
were not: **by looking for defects rather than by re-reading the register.** The
register is nearly empty — six rows, four blocked on you or the world — and it is
nearly empty because I keep closing it. The material below is what re-reading the
*tree* produced.

**Headline: one real tooling defect, one real doc contradiction, and one set of
audit findings that have gone stale without a status column.** Nothing else.

---

## 0. What is not left, and why saying so is the useful part

These were on earlier plans and are **done**, so they are not repeated here:

- **P0.1 / P0.2 / P1.1 of the integration plan are all landed.** P0.1 is exactly
  what `paths::endpoint()` does today (env → daemon state file → `DEFAULT_ADDR`),
  P0.2's recommendation became the shipped behaviour, and codex now gets an HTTP
  MCP entry (`connect_codex.rs` writes `[mcp_servers.memory-wire] url = .../mcp`).
  The plan says so itself in a re-verification block. **The two "decisions I need
  from you" it still lists are therefore answered by the code**, not open.
- Door parity is closed for everything except hygiene's MCP tool, which is a
  recorded deliberate deferral, not an oversight.
- The count-claim gate now covers `src/` as well as the prose.

---

## 1. A real defect: `doctor` looks away when the daemon state file is stale

**Measured on this machine, just now:**

```
$ memory-wire doctor
  server     up
  finding    bank `ishan-parihar-memory-wire` holds no memories; `omp` holds 4246.

$ memory-wire daemon status
  state      not running
  pid        1371296  (no such process)
  state file /home/ishanp/.local/share/memory-wire/serve.json  (stale)
```

Two surfaces, one contradiction. Both are individually correct:

- The server **is** up — pid 1295 has been serving 8888 for four days.
- The daemon **is not** running — pid 1371296, recorded on Oct 17, is gone.

`serve.json` describes a server that no longer exists, and because the live
server was started outside `daemon start`, nothing ever rewrote it. `daemon`
has Start/Stop/Status and **no `adopt`**, so that server can never be managed.

**Where the doctor goes quiet.** `stale_serve_at` is designed for the
swap-the-binary case: it reads the state file, looks up the pid, and compares the
running executable against this one. Line 397 is `let running = exe_of(pid)?;` —
when the pid is **dead**, `?` returns `None`, the whole function gives up, and
doctor reports nothing. So the exact case where the state file is maximally
wrong is the case where the doctor is silent.

**The fix is small and is the honest version of what the doctor already tries to
say.** When the endpoint answers, the daemon state file names a pid that is
gone, and that pid is not the process holding the port: report that the state
file is stale and that the serving process is an unadopted one. Not "restart it" —
that would kill a four-day-old server holding 6,200 memories' worth of live
traffic — but *name the situation*, which is what `doctor` is for and what AGENTS
§10 says its output should be.

Cost: a `daemon` verdict branch or a `doctor` helper, plus tests. **Not** a
supervisor, not an `adopt` command, not touching the running server.

---

## 2. A real doc contradiction: the correction block sits above text it contradicts

`docs/OPEN_HOOK_RECALL_RELEVANCE.md` is marked **OPEN** in AGENTS §4 and is
unchanged since 2026-09-30.

At line 27 there is a correction block:

> **CORRECTION 2026-11-18 — the code block below is STALE. Do not file a bug
> against it.** ... The score is **no longer discarded** ... **The open defect is
> narrower and still real:** `recall_section` ... never reads `score`.

Eight lines below, §"Mechanism" item 2 still says:

> The recall response is a scored structure; `src/api.rs:1903`
> (`recall_should_score_with_the_fused_rrf_value`) pins that the fused RRF score
> is on the wire. `recall_at` deserialises to `Vec<String>` — content only. **The
> score is dropped inside memory-wire's own code**, not by an external caller.

**The correction block is right and the paragraph under it is wrong.** I checked
the code: `recall_at` returns `Option<Vec<ScoredHit>>`, `ScoredHit` carries
`score`, and the only non-test consumer is a `tracing::debug!` at `src/hooks.rs:701`
whose own comment says nothing branches on it.

And the citation itself rots twice over:

| citation | claim | reality |
|---|---|---|
| `src/api.rs:1903` | pins the fused score | the test is at **3132**; 1903 is a bare `}` |
| `src/api.rs:1903` at its own pinned commit `36bb947` | — | **2168**. Wrong even when written. |

The `src/hooks.rs:660` / `recall_at` citations are correct *at their stated
commit* and drifted since, which is the normal kind. `src/api.rs:1903` was never
right, which is the kind that matters.

**The fix.** Repair item 2 to match the correction block, and re-point the
citations. This is a live document a future agent will act on; the "Where the fix
belongs" section says "Stop discarding the score in `recall_at`" — which has
already happened — so the document currently recommends work that is done.

---

## 3. Stale audit findings with no status column

`docs/INTEGRATION_AUDIT.md` (2026-09-30) ranks seven findings F1–F7. Every
heading carries `[V]` or `[C]`, and I initially read those as status. They are
not — the legend at line 29 says **`[V]` verified by a command, `[C]` verified by
reading**. They are *evidence strength*, not resolution state. **The document has
no status column at all.**

So several findings read as live defects when they are fixed:

| finding | as written | today |
|---|---|---|
| F1 | "8888 is not memory-wire" / hook misroute hazard | `ss` shows `memory-wire` pid 1295 holding 8888; `doctor` reports `server up` |
| F4 | "our port is a flag, not a constant ... no single place to fix it" | `paths::DEFAULT_ADDR` exists and is the single place, and §11's rule that every clap default reads from it holds |
| F2 | daemon and hooks disagree on the endpoint | P0.1 landed: `endpoint()` resolves env → state file → default |

**The fix is not to delete the findings.** They are the audit of record, dated,
and AGENTS §7 says a historical record "deliberately describes a specific dated
build, stays and keeps its date." The fix is a **status line** at the top saying
which findings are resolved by commits since 2026-09-30 and which still stand,
with the evidence-strength legend clarified so nobody reads `[V]` as "open" again.

---

## 4. What I checked and found clean

Recorded so the next pass does not repeat it:

- **70 in-tree `file:line` citations**, all within range. Only two name a
  function and point at the wrong one (§2). Citations into other projects' trees
  are frozen audit evidence and must never be checked — those repos are not here.
- **Module table and every count** in AGENTS match the tree; the count gate now
  covers `src/` too.
- **`Host::ALL` = 8, `IMPLICIT` = 7**, matching the prose.
- Both free harnesses were re-run earlier today and are unchanged.
- Footprint: 10.73 MiB, `DT_NEEDED` 3.

---

## 5. Order of work

| # | item | tier | gates | notes |
|---|---|---|---|---|
| 1 | §1 doctor names a stale state file + an unadopted server | 1 | tests, clippy, doc, trust gate, module table | the only real code |
| 2 | §2 repair the OPEN_HOOK item-2 contradiction and its citations | 1 | docs-check | cite by name, not just line |
| 3 | §3 audit status line, `[V]`/`[C]` legend clarified | 1 | docs-check | additive, nothing deleted |
| 4 | §1 optional: a gate that flags a doc contradicting its own correction block | 3 | its own battery | cheap version: assert every `file:line` citation naming a function actually holds it |

**Tier 1 is three commits and well under an hour.** #4 is the only one that
needs a mutation battery, because a check that does not fire on the defect it was
written for is worse than none.

---

## 6. What I need from you

1. **Whether to touch the running server.** §1's finding names it and offers
   nothing. If you want `daemon adopt` — a command that makes an unadopted server
   manageable — that is a proposal, not a bug fix, and it should not be smuggled
   into a doctor fix.
2. **Whether `agent-memory-bench` is worth a model budget.** Still the only
   external benchmark that grades the product's actual job; still needs an
   answering model and Docker.
3. **Whether to cut a release.** 0.9.0 is installed, 87 commits behind, and
   §11 leaves it to you.
4. **The bank decision**, unchanged: `connect --bank omp` would move hooks off the
   empty `ishan-parihar-memory-wire` onto the bank with 4,246 memories. It rewrites
   five live host configs and I have not run it.

# SECTION H-ANALYSIS

# FILE: docs/plans/2026-11-21-remaining-gaps-plan.md (lines 3,3)

> **SUPERSEDED 2026-11-21 by

# FILE: docs/plans/2026-11-21-remaining-gaps-plan-v3.md (lines 1,276)

# Outline plan: remaining gaps, cleanup, and upgrades — measured, with the work log

**Status:** proposal, 2026-11-21 (second pass). Written from `790987d`.

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

# SECTION I-LENSES

# FILE: research/audits/context-pack-2026-11-21-blockers/LENSES.md (lines 1,85)

# LENSES — the instruments this round must apply

The request named four defect classes plus one slogan. Each lens below must
produce **at least one finding**, and every finding must name **which artefact
and which anchor** it came from. A lens that returns "no finding" must say what
it looked at and why the class is absent — that is a result, not a failure.

## L1 — Bottleneck

Where does the system's throughput or its operator's attention actually stop?

Not "what is slow" — the pack carries no latency figures, and AGENTS §7 says a
number without a load reading is a range at best. Look for **structural**
bottlenecks instead:

- the one step every path must pass through, and what it costs
- a decision that cannot proceed without one person, and what queues behind it
- a resource that is exhausted and cannot be replenished (an attention budget, a
  counted consultation, a metered CI quota)

Name the step, name what queues behind it, and say whether the queue is
growing.

## L2 — Redundancy

Where does the same thing exist twice, and which copy is the authority?

Distinguish three cases and treat them differently:

- **Duplicated mechanism** — the same logic in two places, with a drift cost.
- **Duplicated record** — the same fact in two documents, one of which is stale.
- **Deliberate duplication** — a copy kept on purpose, with a named ceiling and
  an upgrade path (`ponytail:` / `shortcut:` markers in this codebase).

The third is not a finding. If the pack's own text claims a duplication is
deliberate and the evidence agrees, say so and move on.

## L3 — Fragmentation

Where is one capability split across surfaces so that no single caller can reach
all of it?

This codebase's own recurring shape is **one service, N doors** (HTTP, MCP, CLI,
and per-host plugins). A capability reachable from some doors and not others is
fragmentation, and the pack's `INTEGRATION_GAPS.md` records the pattern. Check
whether the fragmentation is:

- **absence** — the door was never built (a gap, but a documented one), or
- **drift** — the door exists and has fallen behind the others (a defect), or
- **deliberate asymmetry** — recorded with a reason (not a finding).

## L4 — Integration gap

Where two things that must agree do not, and nothing checks that they do?

Prefer cases where **both sides are individually correct** and the contradiction
only appears when they are compared. Those are the ones a reader cannot discover
from either artefact alone. `doctor` saying "server up" while `daemon status`
says "not running" is the pack's worked example — find others of that shape, or
say there are none.

## L5 — The slogan, as a per-item test

The request asks to *"address and operationalize the optimal-agentic-experience
effectively."* That is not testable as written. Convert it:

> For **every** recommendation you make, name **which defect class from L1-L4
> it reduces**, and **which it adds**. A recommendation that removes a
> bottleneck by adding a seam is a trade — the operator should see the trade,
> not the slogan.

If a recommendation reduces nothing and adds nothing, it is not a
recommendation; drop it. If it adds a class it does not reduce, say so plainly —
that is the most useful thing a review can produce.

## Output shape per lens

```
L<n> — <name>
  findings: <count>
  <finding>: <artefact path + anchor> — <one sentence> [class: L1|L2|L3|L4]
  adds: <which class this finding's implied fix would add, if any>
```

A lens with zero findings writes `findings: 0` and one line on what it examined.
