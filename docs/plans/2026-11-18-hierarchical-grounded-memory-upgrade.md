# Plan — hierarchical grounded memory for memory-wire

Dated-frozen: published **2026-11-18**. A plan that changes is a new plan.

**Read first:** `docs/audit/2026-11-18-surface-audit.md`. This plan is derived
from it; every item below names the finding (F1–F10) or the "genuinely unbuilt"
row it rests on. If the audit's facts are wrong, this plan is wrong.

---

## 0. What this plan is, and what it is not

Three upgrade proposals arrived (Claude hierarchical memory + Dreaming;
a `ponytail-audit` red-team of that; and an `operant` × `memory-wire`
integration blueprint). They were written from the README's top half at
`b478d15` and **describe a `memory-wire` that does not exist**: 54 MCP tools
that are 4, `WAL`/auth work that shipped in v0.7.0, and a promise to lift a
benchmark metric that its own design structurally cannot touch.

This plan keeps what survives contact with the code:

- the **dimensional memory model** (`scope` × `aspect` × `lifecycle` × `trust`),
  which is genuinely absent and genuinely useful;
- **mount-level read-only**, a real prompt-injection boundary;
- **retroactive redaction**, a real privacy gap;
- **deterministic hygiene** (duplicate / contradiction / relative-date /
  bloat), pure Rust, cursor-based — the "Dreaming" substrate without an LLM;
- a **bounded startup grounding capsule**, opt-in and budgeted;
- **`connect --with-skills`**, a real one-line gap.

It drops:

- "activate the four inert ranking mechanisms" — **closed by measurement** (F2);
- "lift `single-session-preference` R@1 from 43.3% to ≥80% via hierarchy" —
  **structurally impossible** on that instrument (F3);
- "add `w_tier · TierPrior` to the RRF score" — **algebraically wrong** (F4);
- "audit and prune the 54 tools" — there are 4 (F5);
- "WAL + busy_timeout + BEGIN IMMEDIATE + loopback bind" — **shipped** (F1).

### Non-negotiable invariants (carried, not restated)

1. **Zero daemon, low RSS.** The default build stays one static binary,
   ~8.8 MiB, ~10.7 MiB idle RSS, `DT_NEEDED` = 3, 0 background processes.
   Any item that would add a dependency, a shared library, or a mandatory
   process is rejected before it is designed.
2. **No LLM in the substrate.** Dreaming/semantic synthesis is the caller's
   job (`operant`/host agent). `memory-wire` exposes deterministic primitives
   only. No `reqwest`, no HTTP client, no subagent runner in the default build.
3. **Eval-gated, three-set discipline** (`AGENTS.md` §6). Selection on LoCoMo
   and the frozen relevance battery only. Every LongMemEval-touching run is a
   counted consultation (next would be #6). **No item in this plan is
   selected on a LongMemEval number.**
4. **Do not re-propose `CLOSED.md`.** Every ranking item here is additive to
   the schema or the serving path, not a weight change.

---

## 1. The shape of the work

Six capability items (W1–W6), ordered by risk and dependency, plus two
correctness fixes that must land first because they make every later gate
meaningful.

```
W0  correctness fixes (test hermeticity, redactor test gap)   [hours, unblocks gates]
W1  dimensional metadata (scope/aspect/lifecycle/trust)       [schema + write + read]
W2  mount profiles (read_only, trust ceiling)                 [config + 2 guards]
W3  retroactive redaction (id/version + pattern)              [version table] ✅
W4  deterministic hygiene scan + apply                        [new module + verb] ✅
W5  startup grounding capsule (bounded, opt-in)               [new read path] ✅
W6  connect --with-skills                                     [one flag]
```

W1 is the foundation: W2, W4 and W5 all read its columns. W3 is independent
and can run beside it. W6 is independent of everything.

**No item changes `FusionWeights::SHIPPED`.** If a later cycle wants a ranking
lever, it is a new hypothesis with its own pre-registration, not a line here.

---

## 2. Items

### W0 — Correctness fixes (do first; gates depend on them)

Rests on **F9** (non-hermetic golden test) and **F6** (redactor test gap).

**W0.1 — make `golden_install_shapes` hermetic.** The test calls `run()`,
which uses the real `binary_on_path`; on a host without `copilot` it fails
with `CopilotCli: unexpected outcome Skipped("not detected on this machine")`.
The seam already exists and nine other tests in the same module use it:
`apply(host, exe, home, uninstall, on_path, data_root, bank)` takes the PATH
probe as an argument, and `no_path` is defined at `src/connect.rs:1309` for
"detection cannot depend on what happens to be installed". Change the golden
scenario to call `apply(..., no_path, ...)` with a home that carries each
host's config dir, so detection is a function of the fixture.

**W0.2 — pin the redactor against code identifiers.** Add cases to
`src/capture.rs` tests: UUIDs, git SHAs, file paths, semver, URLs without
credentials, bare `user@host` strings. Assert what each should and should not
become. Rests on **F6**.

**Gate:** `PATH=/usr/bin:$PATH cargo test --locked` is green on a host with no
agent harnesses installed. `cargo clippy --all-targets --all-features -- -D
warnings` clean.

> **Current state, so nobody misreads the baseline:** on this machine the bin
> suite is **345 passed / 1 failed** and the four integration suites are
> **239 passed / 0 failed**. The one failure is this item's subject. The suite
> is red *now*, so "green" in the gate above is a target, not a description of
> today.

---

### W1 — Dimensional metadata (the foundation)

Rests on **"genuinely unbuilt" row 1**; avoids the **uniform-default trap**
(audit §4) and **F5** (4 tools, so extend not add).

**Schema.** Add to `memories`, via the existing `ALTER TABLE … ADD COLUMN`
migration path (`src/store.rs` `migrate()`, the `has_column` block), all
defaulted so a legacy database reads unchanged:

| column | type | meaning |
|---|---|---|
| `scope` | `TEXT NOT NULL DEFAULT 'local'` | `policy` \| `global` \| `domain` \| `local` \| `agent` \| `session` |
| `aspect` | `TEXT NOT NULL DEFAULT 'episodic'` | `preference` \| `invariant` \| `decision` \| `procedure` \| `trap` \| `fact` \| `map` \| `episodic` |
| `lifecycle` | `TEXT NOT NULL DEFAULT 'active'` | `candidate` \| `active` \| `quarantined` \| `superseded` \| `archived` |
| `trust` | `TEXT NOT NULL DEFAULT 'agent_inferred'` | `manual_policy` \| `human_approved` \| `agent_verified` \| `agent_inferred` \| `untrusted_external` |
| `parent_id` | `TEXT` | nullable FK-ish pointer to another memory (no FK: a dangling pointer must not fail a write) |
| `superseded_by` | `TEXT` | nullable pointer; the row stays readable but is filtered by default |

**Write path.** `Memory` gains the four fields; `retain` gains optional
parameters. `retain_doc_at` already carries **seven** arguments under an
`#[allow(clippy::too_many_arguments)] // one more optional field on the same
write` (`src/api.rs:541`), and the codebase's stated policy is to raise
`too-many-arguments-threshold` rather than keep adding allows — so an eighth
positional parameter is the wrong move. **Introduce a `MemoryMeta` struct**
carrying `(scope, aspect, lifecycle, trust, parent_id, superseded_by,
scope_globs)` and thread it through `retain_doc_at`. That is the same shape the
W1 recall filters and the W5 capsule need, so it is one type, not two.

**Read path.** `recall` gains optional filters (`scopes`, `aspects`,
`lifecycles`, `include_history`) that narrow *before* ranking, in SQL, exactly
as `exclude_tags` does today (`recall_filtered`). `superseded` and
`quarantined` are excluded by default; `include_history` opts in.

**Uniform-default guard.** A test asserts that across the fixture corpus the
`(scope, aspect, lifecycle)` triple is **not** constant — otherwise every
downstream filter is a silent no-op. This is the anti-pattern the audit calls
out by name.

**Gate:** unit + integration tests for migration from a pre-column database,
default-path filters, `include_history` round-trip, and the non-uniformity
assertion. No ranking change; recall output byte-identical when no filter is
passed (asserted).

---

### W2 — Mount profiles (read-only boundary)

Rests on **"genuinely unbuilt" row 2** and **F7**.

**Config.** `BankConfig` (the per-bank JSON object behind
`GET|PUT /banks/:id/config`) gains `read_only: bool` (default `false`) and
`max_trust: string` (default `untrusted_external`, i.e. no ceiling).

**Guard 1 — write refusal.** `retain_doc_at` (and the MCP `memory_retain`, and
the HTTP `retain`) refuse with the frozen error contract's existing
`400 invalid content` shape when the resolved bank is `read_only`. Rests on
`AGENTS.md` §9: the HTTP error contract is frozen; a new refusal must be a new
documented variant, not a silent behavior change.

**Guard 2 — trust ceiling.** A write whose `trust` exceeds the bank's
`max_trust` is refused or downgraded (choose downgrade, and document it: a
silent refuse breaks the never-fail hook path).

**Anti-uniformity.** Test that an untrusted mount cannot write
`scope=policy|global` or `aspect=invariant` — the exact escalation the
proposals name.

**Gate:** `read_only` mount rejects writes on all three surfaces; downgrade
path tested; zero change to a bank with no config.

---

### W3 — Retroactive redaction ✅ DONE (commit 052f3a6)

Rests on **"genuinely unbuilt" row 3** and **F6**.

**Schema.** `memory_versions(id, memory_id, op, content, content_sha256,
actor, created_at)` — append-only, written on every create/update/supersede/
delete/redact. This is also what makes `superseded_by` auditable.

**Operation.** `redact(memory_id, pattern)` rewrites the *current* content and
every historical version's content, replacing matches with
`[REDACTED:<name>]`, preserving the audit skeleton (`op`, `created_at`,
`actor`, `content_sha256` recomputed). A `redact` over a *pattern* reuses
`capture::redact_pii`'s regex set (one source of truth, so the two cannot
drift).

**Delete vs redact.** `delete_memory` stays as-is (hard delete). `redact` is
the history-preserving path.

**Gate:** a secret retained, then redacted by pattern, is gone from
`memories` **and** `memory_versions`, while the version rows and their
timestamps survive; `content_sha256` of a redacted version differs from the
original; a second `redact` is idempotent.

---

### W4 — Deterministic hygiene scan ✅ DONE (commit ad8472a)

Rests on **"genuinely unbuilt" row 4** and **F8**. This is the "Dreaming
substrate" — **no LLM, no new dependency, no new process**.

**New module `src/hygiene.rs`** with a cursor:

`hygiene_scan(bank, cursor_id, opts) -> HygieneReport`

- **delta**: rows created/updated since the cursor (the `cursor` is a
  `created_at` watermark, stored in `banks.config` as `hygieneCursor`).
- **near-duplicates**: Jaccard token similarity above a threshold within the
  same `(scope, aspect)` — candidate `MERGE`.
- **contradictions**: same `scope` + overlapping token set + differing
  `created_at` and a negation/contrast marker — candidate `SUPERSEDE`.
- **relative dates**: regex for `yesterday|today|tomorrow|last week|next
  week|currently|now|this sprint` on rows that lack an ISO date — candidate
  `ANCHOR` (a *proposal*, never an automatic rewrite).
- **promotion candidates**: the same `(scope='local'|'session')`+`aspect`
  pattern appearing in ≥2 sessions — candidate `PROMOTE`.
- **bloat**: rows over `max_record_bytes`, and banks over
  `max_hot_index_lines/bytes` — candidate `SPLIT`.

**Apply** is a separate, explicit verb — `hygiene_apply(batch,
expected_epoch)` — so the scan is read-only and cheap and the mutation is
reviewable. Rests on `AGENTS.md` §8: `ponytail` discipline says the smallest
complete change; scan and apply are two verbs because they fail differently.

**Determinism.** Every detector is a pure function of the rows; no clock, no
randomness. A test runs the scan twice and asserts byte-identical reports.

**Surface.** One MCP tool `memory_hygiene` (a **5th** tool, justified: it is
the only read that is not a recall), or an extension of `memory_reflect`.
Choose the 5th tool and document why 4→5 is correct here while 4→54 would not
be.

**Gate:** synthetic fixture with one planted duplicate, one contradiction, one
relative date, one oversized row → the report names exactly those four and
nothing else; scan is deterministic across runs; scan touches no ranking path.

---

### W5 — Startup grounding capsule ✅ DONE (commit 4540b19)

Rests on **"genuinely unbuilt" row 5** and **F10**.

**What it is.** A per-bank, bounded materialization of the `active` rows whose
`aspect` is `preference|invariant|trap|map` and whose `scope` is
`policy|global|domain`, one line each:

```
[global/preference] m_01a [2026-10-14] Never use em dashes in prose or chat replies
[domain:eng/memory-wire/trap] m_08f [2026-11-02] LongMemEval harness passes a 100k budget; tests/recall_budget.rs guards the 8k trim
```

**Bound.** `max_lines` default **40**, `max_bytes` default **2048** (~500
tokens). **Not** the proposals' 200/25 KiB: the audit's F10 shows the dominant
cost is already the 8,000-token recall budget, and a 25 KiB index is 2–3× that
before a single memory is served. The bound is derived, and a test pins it to
the serving budget rather than to a chosen constant.

**Budget accounting.** The capsule is **subtracted from** the recall budget
when both are served in one injection, so the total never exceeds
`DEFAULT_RECALL_BUDGET`.

**Path conditioning.** Rows may carry a `scope_globs` list (new, nullable,
comma-separated). A row with a non-empty glob set is **excluded from the
capsule** unless an `active_paths` argument matches one — the "conditional
rule" behaviour, without a new store.

**Opt-in.** Per-bank config flag, default **off**. Nothing injects it until a
bank asks.

**Gate:** a bank with 200 preference rows yields ≤40 lines and ≤2048 bytes;
the capsule subtracts from the budget (asserted sum ≤ budget); a glob-scoped
row is absent without a matching path and present with one; a bank with the
flag off serves byte-identical output to today.

---

### W6 — `connect --with-skills` ✅ DONE (commit 4793326)

Rests on **F5** and audit §4.

`connect` writes hooks and MCP entries and **never deploys
`plugin/skills/memory-wire/SKILL.md`** (`grep -c SKILL` = 0 across `src/`).
Add `--with-skills` that copies the skill into each wired host's skills
directory, idempotently, refusing to overwrite a modified copy (same
defensive-write discipline as every other `connect` write: atomic, re-read
verified, never clobbers a foreign file).

**Also update `SKILL.md`** to state the new model once W1–W5 land: what
`scope`/`aspect`/`lifecycle` mean, the "verify against the workspace before
acting" framing, and the L0→L1→L2 read order. Apply the proposals' "what, not
how" principle: describe what each scope is *for*, do not prescribe a capture
checklist.

**Gate:** a fresh `HOME` with one host; `--with-skills` writes the file once,
a second run is a no-op, and a locally modified copy is refused with a named
reason.

---

### W7 — The per-turn injection path, and a supersede verb ✅ W7a DONE (commit d34a0a5), W7b decided (commit eb95a0a) (added after the 2026-11-18 review)

Rests on **D-ASK**, verified at `36bb947`, and on the review's finding 4.

**What is actually true at HEAD, so this item is not built on a stale premise.**
`src/hooks.rs:660` `recall_at` returns `Option<Vec<ScoredHit>>`; `ScoredHit`
(`:496-507`) carries `content`, `created_at` **and** `score: Option<f64>`, and
`the_score_should_survive_the_client_boundary` (`:1497`) pins that the score
survives the wire. So the *evidence-gathering* half of D-ASK has landed. What
has **not**: `recall_section` (`:680-706`) reads content and date and **never
reads `score`** — its only consumer is a `tracing::debug!` whose own comment says
"Nothing branches on this and nothing is gated by it". **Do not re-file the
"score is discarded" bug; it is fixed.** The open defect is that the scale is
available and unused.

**Two sub-items, deliberately separate because they fail differently.**

**W7a — a supersede/retract verb on the MCP surface.** ✅ DONE (commit d34a0a5) Today `memory_retain`
can only append, so an agent that notices an outdated memory mid-turn has no way
to correct it; it can only add a competing row. `delete_memory` exists but is
HTTP-only, so an MCP client cannot reach it. Add one verb — `memory_supersede`
or `memory_retract` — that marks a row superseded (W1's `superseded_by`) rather
than deleting it, so the audit trail survives. This is the **5th tool**, and the
justification is the one the audit already accepted for W4: it is a verb no
existing tool expresses, not a second way to spell `retain`.

**W7b — decide, do not guess, whether `recall_section` should branch on the
score.** The honest options are (a) leave it ungated and say so, or (b) gate it
on something the dev set supports. A raw score cut is **forbidden**: per-query
min-max normalisation forces the top hit of any irrelevant set to 1.0 by
construction (`src/recall.rs:319-337`), so a cut on that field is close to
meaningless — the same reasoning that already killed the threshold idea in
`CLOSED.md`. If W7b is attempted at all, it is a pre-registered hypothesis on
the frozen relevance battery, never a constant picked off an observation.

**Gate:** a supersede verb round-trips (supersede → default recall excludes it
→ `include_history` returns it with its lineage); the 4-tool count becomes 5
with a documented reason; and if W7b is attempted, its gate is the frozen
battery, not an anecdote.

> **Why this item exists at all, said plainly:** it is the one place where the
> review found the plan optimising a subsystem that is already fine while a
> diagnosed per-turn defect sat outside it. The defect is real, it is on the
> path every agent turn takes, and W0–W6 as first written did not touch it.

---

## 3. Ordering and what each item costs

| item | depends on | risk | rough size | what it buys |
|---|---|---|---|---|
| **W0** | — | low | hours | a gate that means something |
| **W1** | W0 | **medium** — schema migration | days | the substrate for W2/W4/W5; queryable scopes |
| **W2** | W1 | low | days | prompt-injection boundary (the real security win) |
| **W3** | W0 | low | days | retroactive privacy |
| **W4** | W1 | **medium** — new module, new verb | days | "Dreaming" without an LLM |
| **W5** | W1 | medium — serving path | days | bounded grounding at ~500 tokens |
| **W6** | — | low | hours | skills actually reach the hosts |
| **W7** | W1 | **medium** — a new verb, and a policy decision | days | the per-turn path; an agent can correct a stale memory |

**Do not parallelize W1 and W4/W5.** W1's column shape is what W4's detectors
and W5's filter read; changing it after W4 lands is a second migration.

---

## 4. Gates / success criteria (fixed before the run)

> **What this plan cannot move, stated plainly (added after the 2026-11-18
> external review).** Every gate below is deliberately conservative: no ranking
> weight changes, unfiltered recall stays byte-identical, the flag-off paths stay
> byte-identical. The consequence is that **no LongMemEval-S metric and no LoCoMo
> metric can move because of W1–W5.** A per-question fresh index has no W5 capsule
> rows; W1 adds no unfiltered effect; W4 never runs during a benchmark recall; W2
> is a write guard. The metrics W1/W4 can move are **PrecisionMemBench** (if the
> adapter populates and queries the new filters) and the **frozen relevance
> battery**. W2 moves no metric in the record at all. All four can move footprint
> and latency **negatively**, which Gate 8 bounds.
>
> That is the correct trade — but it must be said out loud, or the plan reads as
> promising a retrieval gain it structurally cannot deliver.

1. **W0:** `PATH=/usr/bin:$PATH cargo test --locked` green on a host with no
   agent harnesses; clippy `-D warnings --all-targets --all-features` clean.
2. **W1:** migration from a pre-column DB reads back every row; recall with no
   filters is byte-identical to pre-W1; a `(scope, aspect, lifecycle)`
   non-uniformity assertion passes on the fixture corpus.
3. **W2:** a `read_only` bank refuses writes on all three surfaces; an
   `untrusted_external` write cannot land as `scope=policy`.
4. **W3:** a redacted secret is absent from both `memories` and
   `memory_versions`; the audit skeleton survives; a second redact is a no-op.
5. **W4:** the scan is deterministic across runs; it names planted
   duplicates/contradictions/relative-dates/oversized rows and nothing else;
   the ranking path is untouched (recall output identical).
6. **W5:** capsule ≤40 lines and ≤2048 bytes at the defaults; the capsule +
   recall sum ≤ `DEFAULT_RECALL_BUDGET`; a glob-scoped row appears only with a
   matching `active_paths`; the flag-off path is byte-identical.
7. **W6:** `--with-skills` is idempotent and refuses a modified copy.
8. **Cross-cutting, every item:** binary ≤ 9 MiB (from 8.8), idle RSS ≤ 12 MiB,
   `DT_NEEDED` = 3, `cargo doc --no-deps --all-features` 0 warnings.
   **Any item that moves these is redesigned, not merged.**
9. **No ranking change is merged without a dev-set measurement** (LoCoMo or
   the frozen battery). This plan ships **zero** ranking changes; if a later
   item needs one, it is a new pre-registered hypothesis with its own gate.

---

## 5. Cost

No new dependency, no new shared library, no LLM, no background process. The
default build stays a single static binary. The largest single item is the W1
migration; the riskiest is W4 (a new module with a new public verb, which
`AGENTS.md` §5 requires be documented with `///` on every public item).

**What this plan deliberately does not do** (and why):

- **Change any ranking weight.** `CLOSED.md` forbids re-proposing closed
  levers without new evidence, and the audit (F2) shows the four "inert"
  mechanisms were measured and failed.
- **Add a tier prior to the RRF score.** Algebraically wrong (F4); if a
  structural prior is ever wanted it is a bounded rank voter, its own
  hypothesis, its own gate.
- **Promise a `single-session-preference` R@1 lift.** Structurally impossible
  on that instrument (F3); the plan says so rather than shipping a metric it
  cannot move.
- **Touch LongMemEval to select anything.** `AGENTS.md` §6; the next
  consultation would be #6 and is release acceptance only.

---

## 6. Resume here

**All items complete.** W0–W7 shipped across 10 commits:

- W0 (a45ea07) — hermetic golden_install_shapes + redactor identifier tests
- W1 (e10ba0b) — dimensional schema: scope/aspect/lifecycle/trust/parent_id/superseded_by
- W2 (8eaeac3) — mount profiles: readOnly + maxTrust on BankConfig
- W3 (052f3a6) — retroactive redaction with version audit trail
- W4 (ad8472a) — deterministic hygiene scan module
- W5 (4540b19) — startup grounding capsule module
- W7a (d34a0a5) — supersede verb on the MCP surface (5th tool)
- W6 (4793326) — connect --with-skills + embedded SKILL.md
- W7b (eb95a0a) — decision doc: no raw score gate on recall_section

`docs/handoff/STATE.md` is updated when a unit of work lands, per
`AGENTS.md` §13.

### 6.1 What the 2026-11-18 external review changed (round 1)

A context-pack review round was run against this plan — Hydrogen at `xhigh`,
75 kB pack, 18,985-char reply. Pack, prompt, reply and the verification pass are
in **`research/audits/context-pack-2026-11-18-round1/`**. Four adoptions:

1. **W7 added** — the per-turn injection path and a supersede verb. The reviewer
   found the plan improving a subsystem that is already fine while a diagnosed
   per-turn defect sat outside it. That was correct and is the round's main win.
2. **Gate 4 now states what the plan cannot move** (§4 preamble). True by
   construction; the plan had never said it.
3. **D-ASK's `hooks.rs` snippet corrected in place** — the score is no longer
   discarded, and the doc now says so rather than inviting the next reader to
   re-file a fixed bug.
4. **Process fix:** pack the artefact under review **in full**. The round's pack
   omitted this plan's W0–W6 body to fit a byte budget, and the reviewer's gaps
   hardened into confident claims about code it could not see. That is my error,
   recorded in the verification pass §5.

Two reviewer claims were **refuted** and recorded as such: that `hooks.rs`
discards the score (it carries `ScoredHit.score`, pinned at `src/hooks.rs:1497`),
and that the provenance label is missing (it is present at `:682`).