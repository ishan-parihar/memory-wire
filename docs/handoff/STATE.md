# State at handoff — 2026-10-16

> **STALE HEADER, added 2026-11-22. Read this before the date in the title.**
>
> This file is titled 2026-10-16 and its body describes **v0.8.2**, released that
> day. The tree is now **v0.11.0 plus post-release commits**, which is **past**
> `c919f13`, the commit this file names as HEAD. The body was not wrong when it
> was written; it has simply not been rewritten since, and a reader who takes the
> title at face value will believe the project is 98 commits behind where it is.
>
> The body is kept as the dated record (AGENTS §7). For where the project stands
> now, the live list is
> [`docs/plans/2026-11-22-remaining-gaps-plan-v6.md`](../../plans/2026-11-22-remaining-gaps-plan-v6.md)
> (**executed 2026-11-22**, with its record in §9) and, for the implementation
> register, **`TODO.md`** -- which `docs/handoff/README.md` already says, and this
> file now says too, because a fresh agent should not have to open two files to
> find out which one is authoritative.
>
> **The current state, in one place** (2026-11-22, after plan v7 and `v0.11.2`):
> `v0.11.2` is tagged (`e7011c8`) and published with three Linux assets, and it
> is **installed and serving here** (pid 560976, health `ok`, bank `omp` 4250
> memories). Plan v7's five doc items are done (`5c48e96`), its omp entry was
> **kept** after verification (`3f99063` corrected the premise), and two defects
> were found by *running* the release: `update` needing `--clobber` on its
> scratch dir, and `build-release.sh` conflating "cross loader absent" with
> "does not run" (`d06c413`). 682 tests, clippy 0, doc 0, four doc gates green
> (the fourth is `scripts/tool_table_check.sh`). `CONSISTENCY.md` §81-§83
> record the round. **The honest limit:** the live `update` path has never been
> run to completion against a real upgrade here, because every attempt so far
> was either a version behind or pointed at the data dir by mistake.
>
> The status table at the foot of this file is **generated** (`genome emit`), not
> hand-written. It read "open 4 / done 24" until 2026-11-22, then "open 6 /
> done 31"; it now reads "open 5 / done 32" after TODO #31 closed, and matches
> `TODO.md`. Editing that table by hand would be overwritten on the next
> `genome emit` and would hide the drift.

Repo `agentic-utility/memory/memory-wire`, branch `main`, HEAD `c919f13` — **v0.8.2,
released** (the tag names `95e54c1`; `3ac422c` and `c919f13` are the records and
navigation passes on top). The day cut two patches: **v0.8.1** (`fbe0a91`+`52e27a7`: the H8 capsule
mode, the `MEMORY_WIRE_FORMAT` hook knob, the OmniMemEval HaluMem battery entry,
acceptance #5 byte-identical) and **v0.8.2** (`62fb072`+`95e54c1`: the `update`
`parse_tag` fix found dead on the first real hop — §45 tells it plainly, including the
correction of the v0.8.0-era "live-verified update" claim). racknerd sits on 0.8.2
(installer hop; wiring preserved; `update` now reads 12 known releases). The H9 BEAM
shape-check landed its verdict: not mechanical, labelling stays the unblock.

> **This page's header is stale and says so.** HEAD is `36bb947` (**v0.10.0**), not
> `c919f13`/v0.8.2. §1 below carries the 2026-11-18 correction, the upgrade plan it
> produced, and the external review round run against it. Read §1 first.

## Contents

- [1. What is in the working tree](#1-what-is-in-the-working-tree-post-release-residue)
- [2. Current numbers](#2-current-numbers-every-one-traces-to-a-committed-artifact)
- [3. Instrument caveats](#3-instrument-caveats-a-fresh-agent-will-otherwise-rediscover-the-hard-way)
- [4. Environment](#4-environment)
- [5. What the LoCoMo judged run says](#5-what-the-locomo-judged-run-says-about-where-the-headroom-is)

## 1. What is in the working tree (post-release residue)

  **2026-11-21 -- v3 execution: three commits, two defects found by reading, one by
  testing.** `docs/plans/2026-11-21-remaining-gaps-plan-v3.md`, at the user's
  request.
  - `1fcde7d` `src/mcp_http.rs` said "four tools" in three places and served
    six; `INTEGRATION_GAPS.md`'s table said 4 and "all 4"; both same-day plans
    now carry supersession markers.
  - `c425f1e` the count gate now scans `src/` -- the place the first defect hid
    -- checking `N tools` and `N hosts` against `Server::tools()` and
    `Host::ALL`. Mutation-checked both directions.
  - `ed96bbd` `POST /banks/:id/memories/:mid/redact`, the last door redact was
    missing. **Testing it found a frozen-contract gap**: `StoreError::BadInput`
    (a regex typo) fell through to an opaque 500 and now maps to 400 `invalid
    content`.
  - **The mcp_http defect survived review because its test asserted against
    `Server::tools()` rather than a literal.** The name and the docs were wrong,
    the assertion was right, so nothing failed. Recorded because the distinction
    between a documentation defect and a coverage defect is the whole lesson.
  - Measured free: LoCoMo R@10 94.2%, NDCG@10 73.6%, MRR 71.8%, R@pool 100%
    (load 23.71); relevance battery H8 gate held, 42% less auto-injected. Both
    identical to the prior batch, as expected for doors-and-docs work.
  - Footprint: 10.73 MiB (was 10.65), `DT_NEEDED` still 3.
  - Hygiene gets no MCP tool -- a deliberate deferral recorded in TODO 31, not an
    oversight. Revisit only if a caller needs it.

  **2026-11-21 — full-scale execution of the remaining-gaps plan, four commits.**
  `docs/plans/2026-11-21-remaining-gaps-plan-v2.md`, executed at the user's
  request. All gated, all committed:
  - `d86d200` the plan. `6df26dd` doctor names the *input* behind an empty
    bank (`paths::BankSource` + `bank_source_in`), with the fix line differing
    correctly between an env-sourced and a derived bank. `34237bf` supersede
    got its HTTP route and hygiene its CLI -- two of TODO 31's three claims
    turned out to be already false, and the row now says so. `59a2919` closes
    TODO 36 as *deliberately not extracted* (extension 356 lines exporting a
    function, opencode 484 exporting a v2 object; 0 vs 12 event hooks) and
    records TODO 38's real-drive block with its evidence. `b7bee48` is
    CONSISTENCY §74.
  - **Measured free after the changes:** LoCoMo dev R@10 94.2%, NDCG@10 73.6%,
    MRR 71.8%, R@pool 100% (load 32.37 on 24 cores, so ranking only);
    relevance battery H8 gate held, ranking identical, 41% fewer auto-injected
    characters.
  - **Deliberately not run:** LongMemEval-S. The dataset was present and the
    command would have worked, but nothing in this batch touches ranking, so
    spending one of the five counted consultations would have bought a green
    number that says nothing. That reasoning is in §74, not just the omission.
  - **Blocked:** agent-memory-bench (not cloned; needs an answering model and
    Docker -- the user's budget). AMB/PrecisionMemBench (`../_audit/amb` is
    gone). TODO 38's real-host drive (needs a HOME override the guard refuses,
    or a container).

  **2026-11-21 -- the plan review came back and I acted on its three strongest
  findings.** `research/audits/context-pack-2026-11-21-rulings/` (the reply
  arrived pasted by the operator; the tool never collected it --
  `notes/RESPONSE-NOT-COLLECTED.md` records why, and that the page was
  recycled by a concurrent session). It scored the plan 4/10 on evidence
  density and falsified six of nine claims. Verified before acting, in three
  commits:
  - **My round-trip test had the same blind spot I was accusing TODO 35 of
    having.** It seeded the marker through the API and only tested the read,
    so mutating the opencode capture subscription to `false` left it green.
    **Fixed** (`b25a56b`): the door now retains the pair itself through the
    real `ctx.event.subscribe()` iterator, and the mutation fails by name.
  - **AGENTS.md's own counts were stale in three places** -- five tests files
    instead of six (`installer.rs` missing), v0.8.2 instead of 0.10.0, H5
    "will be the next" when it landed 2026-10-14. **Fixed** (`fca968c`), which
    also removed the hand-maintained section-number cross-reference after it
    was found stale for the third time (§36, then §50, actual §72).
  - **The plan said "stub server" when the built test uses a real one.** The
    reviewer read my word, not my code, because the pack did not carry the
    test. Corrected in the plan; the lesson recorded there is that a plan
    describing a check must carry the check.

  **2026-11-20 — execution plan closed and round 5 of the context-pack review
  landed.** Two batches, both committed and both gated.

  - **Tiers A/B/C of `docs/plans/2026-11-19-execution-plan.md`**
    (`131cffb`..`b8ef4c4`, then `5285ea3`). A: the plugin-door tests wired to
    a gate (they had run nowhere), the `genome` lint E003 cleared, and a
    `#[cfg(test)]` test -- `the_embedded_source_is_the_one_on_disk`, in
    `src/skills.rs` -- asserting the `include_str!`-embedded skill equals
    `plugin/skills/memory-wire/SKILL.md` on disk. **Corrected 2026-11-20**: this
    entry first said "a nightly check that `.skills_to_install/` still matches
    `src/skills.rs`". No such directory exists anywhere in the repo or its
    history and nothing references one; the test named here is what actually
    gates it. The test is stronger than a nightly check would be, because a
    change to `SKILL.md` cannot compile unless the file matches. A doctor block
    that names *which bank* holds the memories when the resolved bank is empty
    (it printed `bank: <resolved>` next to `memories: 0` and left the reader to
    join them); a `doctor banks` row per bank. B: `connect_common.rs` (six
    byte-identical helpers, net -138 lines);
    `scripts/module_table_check.sh` wired into the pre-commit hook, with the
    §3 table regenerated 18 -> 33 rows; a narrow per-door redact fix. C:
    `memory_redact` as the 6th MCP tool; the banks list.
  - **Round 5 of the fresh-context review** (`research/audits/context-pack-2026-11-20-round5/`,
    Hydrogen at `xhigh`). Two real defects in code from the previous batch:
    **opencode `injectedQuery` cached an empty block across a failed recall**
    (a transient outage froze recall until the user retyped), and the
    **"…and N more" footer understated the dropped count** in Rust and hermes
    when the character cap broke the loop before the line cap. Both fixed, both
    mutation-checked, and two new checks added to the trust gate (5 -> 7).
    The reviewer's headline is adopted: **verification is the bottleneck** — four
    of the five recent high-severity defects came from a check that ran nowhere
    or verified the wrong thing.

  **2026-11-19 — round-2 follow-ups landed (uncommitted at the time of
  writing; this entry names the batch).** Two things, one of which was a
  real defect rather than a follow-up:

  - **The `trust` column had no bottom rung.** W2's schema table and
    `memory.rs` both list `untrusted_external` as a tier a memory may hold, but
    `Trust::tier` accepted three tiers and mapped everything else — including
    `untrusted_external` — to `agent_inferred`. The tier the auto-injection
    gate refuses therefore could not be *written*, so a gate built on it would
    have been dead code with every test green. Fixed: `UntrustedExternal` added
    below `AgentInferred` (the ceiling's `downgrade` now bottoms out there),
    `trust` carried on the `full`/`summary` recall wire, and
    `hooks::recall_section` drops that tier **before** the `MAX_LINES` take. An
    explicit `memory_recall` still returns those rows — a reader who asked
    consented. The same hole is closed in the hermes plugin's `prefetch`
    (`UNTRUSTED_TIER`). **Not a relevance threshold:** a deterministic
    allow/deny on a stored tier, no fitted constant, so CONSISTENCY §32/§48 does
    not bite. Record: `CONSISTENCY.md` §51; TODO row 30.
  - **Doc truth from the round-2 pack.** `memory.rs` now documents the four
    tiers that exist and says the plan's `manual_policy`/`human_approved`/
    `agent_verified` set was never built; `mcp.rs` says in place that
    `HISTORIAN` is a **prompt**, not a sixth tool; `mcp --help` no longer omits
    `memory_supersede`; and the doors' asymmetry is recorded with a verified
    table in `docs/INTEGRATION_GAPS.md` (addendum) + TODO row 31.

  Gate at this batch: **658 passed** (272 lib + 363 bin + 23 it), clippy
  `-D warnings` clean, `cargo doc` 0 warnings. The gate's load-bearing property
  was checked, not assumed: removing the filter from `recall_section` fails
  four tests, including a full-chain test from a real router through
  `hooks::prompt_at`.

  **2026-11-19, same day — recall failures are now tellable apart (CONSISTENCY
  §52, TODO row 32).** `recall_at_fmt` returned `None` for three different
  reasons (connection refused, non-2xx, 200-with-an-unparseable-body) and all
  three produced byte-identical silence. Each names itself on stderr now. Still
  exit 0, still empty stdout, and `RUST_LOG=off` is total silence, so §9 is
  intact; the empty-query path stays silent because it is not a failure.
  Verified live against three real servers.

  **2026-11-19, third round — the trust gate was missing from two of the four
  doors (CONSISTENCY §53).** A context-pack review found that `1f5ee17`'s trust
  gate shipped in `src/hooks.rs` and the hermes plugin but in **neither
  TypeScript plugin**: both `formatRecall` implementations injected every row a
  recall returned, so an untrusted mount's web content reached a model's system
  prompt as rules. The weakest door decided the policy, and it was the two files
  with no test suite. Fixed in both; proof is
  `plugin/integrations/trust_gate_test.sh`, which runs each file's real
  `formatRecall` in `node` and fails with `untrusted_external reached the block`
  when either gate is removed. Same review corrected four false claims in the
  docs (ARCHITECTURE's "redaction happens on write", AGENTS' 10.25 MiB binary
  size, C-ASK's "every number is `[LIVE]`", `measure_surface.sh` stopping at
  `src/` and never counting the 1,622 lines under `plugin/` and `install/`) and
  **refuted one finding of my own** — the "500/500 includes 30 gold-less
  abstention questions" objection; all 30 carry `answer_session_ids` and
  `eval/RESULTS.md` already reports the 470 split. Verdicts in
  `research/audits/context-pack-2026-11-19-round3/VERIFICATION.md`.

  **2026-11-19, the remaining-work plan — all four batches done.** The plan is
  `docs/handoff/REMAINING_WORK_PLAN.md`; the four commits are `e0f1bb4`
  (stale register rows), `db57850` (hermes trust-gate proof), `0b518de` (dates on
  the three doors that dropped them), `1f93bf0` (the capsule knob). Record:
  `CONSISTENCY.md` §54–§57, TODO row 33.

  The shape behind all four: a policy enforced in four places with a check in
  three, and a register that stopped being maintained. Batch 1 corrected four
  open rows whose work had already landed (P2.1, P4.1, P3.1, G6.8 — each closed
  by a file or a command). Batch 2 gave the trust gate's fourth door a hermetic
  test. Batch 3 served `created_at` in hermes and both TypeScript plugins, which
  requested `format: "full"` and dropped the field. Batch 4 decided the capsule
  knob by cost: the plugin surfaces read `MEMORY_WIRE_FORMAT`, not the bank
  config key, because the env is free and the config would add a request per
  turn — documented at each site rather than left as a silent asymmetry.

  **2026-11-19, round 4 — a cap that counted input, not output (CONSISTENCY
  §58).** Round 4's context-pack review found a defect in the batch-3 code:
  both TypeScript plugins bounded the recall loop with
  `i < items.length && i < RECALL_ENTRIES`. That reads as a cap and is not one —
  it bounds the *input scan*, so five untrusted rows at ranks 0-4 consumed the
  whole budget and a trusted row at rank 5 was never reached. `src/hooks.rs` and
  the hermes plugin filter first and take second; the two TS plugins took first
  and filtered second. Fixed in both (`ed8eea4`), with the cap restated as
  `lines.length >= RECALL_ENTRIES` on the emitted count.

  The generalisable part: the three doors *looked* identical, and the trust gate
  could not have caught it because it tested that untrusted rows are withheld,
  never what happens when they sit **above** trusted ones. A guard for one
  property does not check its ordering counterpart. The new test asserts both
  directions and fails on each door when the loop bound is restored.

  Same commit corrected four plugin comments claiming a score threshold had "no
  dev set to choose one" — false, AGENTS §6 designates `eval/data/locomo/`, and
  the real reason thresholds stay closed is determinism (§32/§48) — plus the
  four-vs-five MCP tool count in AGENTS.md and `src/mcp.rs`.

  **2026-11-19, the execution plan — Tier A landed (`131cffb`, CONSISTENCY
  §61).** The plan is `docs/plans/2026-11-19-execution-plan.md`; it covers
  implementation gaps, redundancy and operational upgrades, and it is ordered by
  what is verifiable rather than by what a register says.

  Its first finding is the one worth carrying forward: **`plugin/integrations/trust_gate_test.sh`
  was wired to nothing.** It is the cited proof for the trust gate in four
  CONSISTENCY sections, and it appeared in no hook, workflow or script.
  `.githooks/pre-commit` ran the secret gates; `ci.yml` ran cargo test and
  clippy. So the only executable check on the two doors with no compiler was a
  file a human had to remember to run — which is exactly how both the round-3
  gap and the round-4 starvation bug reached a reader. It now runs in
  pre-commit behind a `command -v node` guard that says so instead of skipping
  silently, and as a CI step. Mutation-checked: restoring the bad loop bound
  makes the hook exit 1.

  Also in that commit: the genome lint was red (`E003` on TODO row 4, whose
  evidence cell still read as an instruction to a future agent) and is clean
  again; and `RND_PLAN.md` §6 stopped calling itself "the upcoming cycle" for a
  cycle that closed on 2026-10-17.

  **Still open from the same review:** AGENTS.md §3's module table lists 18 of
  32 modules and claims "22 modules / 24.8k lines" against a measured 35,298;
  `connect_*` is 6,609 lines (18.7% of `src/`) with no audit of its code-vs-test
  split; and the custody asymmetry (§2.1) — MCP has `memory_supersede` but no
  redact, HTTP has redact but no supersede — remains open. All three are recorded
  in `research/audits/context-pack-2026-11-19-round4/`, verified but not yet
  actioned.

  Two claims were corrected while verifying rather than after: `README.md` said
  every auto-injected *and MCP-recalled* memory carries its date "in-band"
  (MCP recall serves `created_at` as a field, which is right for a structured
  caller), and the capsule knob was described as if it worked everywhere.

  **A trap worth carrying forward: a hooks test that spawns the binary is only
  as current as the last explicit `cargo build --bin memory-wire`.** `hooks`
  tests resolve the binary via `main::bin()` → `target/debug/memory-wire`, and
  `cargo test --bin memory-wire` does **not** rebuild that file. My first
  negative check *passed with the notes deleted*, because the test ran a stale
  binary. Run `cargo build --locked --bin memory-wire` before trusting a hooks
  test that spawns the process.

  **2026-11-18 — read this first if you are picking the upgrade up.** HEAD is
  `36bb947`, **v0.10.0**, not the `c919f13`/v0.8.2 this page was written at;
  the tree has moved a long way since. Three external upgrade proposals arrived
  (Claude hierarchical memory + Dreaming, a red-team of it, and an `operant` ×
  `memory-wire` blueprint) and **all three describe a tree that does not exist**:
  they claim 54 MCP tools where there are **4**, they ask for `WAL` /
  `busy_timeout` / loopback bind / `spawn_blocking` which **shipped in v0.7.0**,
  and they promise a `single-session-preference` R@1 lift from a hierarchy that
  **structurally cannot produce one** (LongMemEval-S is per-question fresh index,
  session-as-document — there is no HotIndex and no global tier to score).

  The grounded replacement is **`docs/audit/2026-11-18-surface-audit.md`**
  (findings F1–F10, every one marked verified or asserted) and
  **`docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md`** (W0–W7).
  TODO rows 21–29 were the register. **All W0–W7 items shipped** (2026-11-18,
  commits `a45ea07` → `b4b4698`): rows 21–29 are closed with their closing
  commits, and the W7b recall-gating question is a recorded decision
  (`docs/DECISIONS/2026-11-18-w7b-recall-gating.md`) rather than open work.

  **A context-pack review round has been run against that plan** (Hydrogen at
  `xhigh`, 2026-11-18): pack, prompt, reply and the mandatory verification pass
  are in **`research/audits/context-pack-2026-11-18-round1/`**. It changed the
  plan in four ways, and one of its findings was a defect in my own pack:

  - **W7 added** — the per-turn injection path, and a supersede verb. The review
    was right that W0–W6 optimised a subsystem that is already fine while a
    diagnosed per-turn defect sat outside it.
  - **Gate 4 now says plainly that W1–W5 cannot move any LongMemEval-S or LoCoMo
    metric.** True by construction (every gate is byte-identical), and the plan
    had never said so.
  - **Two of the review's claims are refuted** and recorded as such: `hooks.rs`
    does **not** discard the score (it has carried `ScoredHit.score` since before
    v0.10.0, pinned at `src/hooks.rs:1497`), and the provenance label **is**
    present. D-ASK's own doc was stale and is corrected in place.
  - **The pack omitted the plan's W0–W6 body** (lines 82–331) to fit a byte
    budget — my error, and the reason the reviewer fell back on D-ASK's stale
    description. Any future round packs the plan whole.

  Three facts from that audit outrank everything else on this page:

  - **The four "built but inert" ranking mechanisms are not inert-pending, they
    are measured and closed** (`eval/LOCOMO_LEVERS.md`, `CLOSED.md`): H1a
    `keyword_scope` R@1 +3.0pp but gate failed; H1b `overlap_scope` multi-hop
    R@1 **−11.2pp**; H1c `bm25_magnitude` a wash; H3 recency temporal R@1
    **−33.4pp**. Do not re-propose them (AGENTS §8).
  - **`cargo test --locked` is green again**: W0.1 (`a45ea07`) made
    `golden_install_shapes` hermetic — it drives `apply(..., no_path, ...)`, so
    detection is a function of the fixture, not of what is installed on the
    host. The pre-`a45ea07` failure (no `copilot` on PATH) is fixed and gone.
  - **No item in the new plan changes `FusionWeights::SHIPPED`**, and no item is
    selected on LongMemEval. The plan ships zero ranking changes.

  The §45 records pass and the navigation pass are **committed** (`3ac422c`,
  `c919f13`); the tree was clean at `c919f13`. It now holds the **§6 cycle
  pass, uncommitted by instruction**: the outline (`RND_PLAN.md` §6, H12
  pre-registered) plus its execution — W1 (BEAM pair-unit labels + harness:
  `eval/beam_label.py`, `eval/beam_labels_100k.json`, `examples/beam.rs`,
  artifacts `eval/BEAM.md` + `eval/results_beam.json`), W2 (frozen battery:
  `eval/relevance/freeze_battery.py`, grown `cases.jsonl`, artifact
  `BATTERY-2026-10-16-frozen.txt`), `docs/CONSISTENCY.md` §47–§48,
  `docs/handoff/README.md`, `docs/handoff/STATE.md`, `TODO.md` (rows 6, 9, 20).
  Untracked data under `eval/data/` is gitignored by design. W3's adapter fix
  lives in the pinned clone, not here.

  Cycle state (§6, **closed 2026-10-17 — closing record `docs/handoff/CLOSING-2026-10-17.md`**): **W1 done** — the BEAM pool-scale
  overlap re-test answered (`eval/BEAM.md`, §47): the stream is **not
  deleteable** (0.25 > 0.00 on every ordering metric; 0.00 additionally loses
  R@pool 100→95.5% — the overlap stream carries coverage below BM25's 50-row
  cut at 94–196-doc banks), **but the unfitted 1.00 leads 0.25 on the same
  corpus** (R@5 67.1 vs 63.6, opposite sign to LoCoMo's +1.4pp) — the weight
  stays user-gated. **W2 done** — the battery is frozen (sha256-pinned
  snapshot → deterministic 1,413-row bank) and grown 10→21 (gold 16/17,
  junk 0/9, D-case passes, H8 re-confirmed); precision levers registrable
  from here — none registered. **W3 done — H12** (`eval/omnimevaleval/
  memory-wire-mw_h12_dated_medium/`, §46): the dated instrument reads
  Dynamic Update 0.2056→**0.4056** (+20.0pp), overall 0.6129→**0.7433**,
  Memory Boundary control holding at −2.5pp and context only +12% — the
  H10 headline measured a reader blind to time; both numbers stand as two
  instrument versions. **The §6 cycle is complete**: every workstream
  terminal, records current, 0 test consultations added all cycle. Blocked:
  Track 2/agent-memory-bench (agent-model budget). No ranking lever is open;
  the fitted `overlap 0.25` is now *replicated on LoCoMo, inverted on BEAM* —
  an honest split, the user's call.

That release batch (feature `04123f7`, bump `a272196`) carried everything the old
list below named — the full R&D record set, the product code, and every new
harness and artifact — into the v0.8.0 tag. The pre-release residue list is
retired here rather than kept current:

  Modified (all now **in `v0.8.0`**): `AGENTS.md INSTALL_FOR_AGENTS.md README.md TODO.md
  docs/CONSISTENCY.md docs/INTEGRATION_GAPS.md docs/INTEGRATION_PLAN.md eval/LOCOMO.md
  eval/README.md eval/RESULTS.md eval/download.sh eval/locomo_dev.rs eval/results_locomo.json
  examples/answer_quality.rs examples/bench_footprint.rs examples/locomo.rs
  examples/longmemeval.rs plugin/skills/memory-wire/SKILL.md src/api.rs src/connect.rs
  src/connect_ext.rs src/connect_opencode.rs src/doctor.rs src/hooks.rs src/main.rs
  src/mcp.rs src/paths.rs src/store.rs src/sweep.rs`.
 Untracked (all now **in `v0.8.0`**): `docs/AROUTE_BUGS.md docs/BENCHMARK_LANDSCAPE.md
  docs/handoff/ eval/amb/ eval/baseline/ eval/llm_shim.py eval/LOCOMO_TURNS.md
  eval/results_locomo_turns.json examples/locomo_ranks.rs eval/LOCOMO_RANKS.md
  eval/results_locomo_ranks.json examples/locomo_levers.rs eval/LOCOMO_LEVERS.md
  eval/results_locomo_levers.json eval/LOCOMO_WINDOWS.md eval/results_locomo_windows.json
  eval/h4_subset.json eval/amb/h4_build_subset.py src/connect_claude.rs src/connect_copilot.rs
  eval/SELECTION_VECTOR_AXIS_ALL10.md eval/results_selection_vector_all10.json src/update.rs`.

`eval/amb/` now also carries the run artifacts `PRECISIONMEMBENCH-2026-10-14-h6-filters.json.gz`
and `h4_locomo_rag_topk{3,5,10}.json.gz`; `eval/baseline/` carries
`2026-10-14-v0.7.0-dated-answer-quality/` (H5) and `2026-10-14-bench-concurrency/` (H7).

Check with `git status --short`; `git diff --stat` shows size. The gate passed on this tree
on **2026-10-15, after the H1d measurement** (release build + all examples, **589** tests
across 9 suites, clippy `-D warnings --all-features` clean — the embed code path included,
0 doc warnings). The embed build ran in an isolated `CARGO_TARGET_DIR`; the default
binary is untouched (`DT_NEEDED` 3). **Re-run the whole gate (AGENTS §2) before
reporting anything as verified** — the tree keeps moving.

### Product changes in this cycle (all tested, none tuned)

| change | where | test |
|---|---|---|
| `retain` accepts `created_at` (RFC 3339 or `YYYY-MM-DD`, stored UTC fixed-width; else frozen `400 invalid content`, nothing written) over HTTP and MCP | `src/api.rs` `parse_created_at`, `retain_doc_at`; `src/main.rs`; `src/mcp.rs`; const `store::CREATED_AT_FORMAT` | `retain_created_at_should_round_trip_through_full_recall` in `src/main.rs` and `src/mcp.rs` |
| `format: "full"` recall returns `{id, score, created_at, content}` (one shared `api::full_hit_json`) | `src/api.rs` | same tests; default (bare-string) recall unchanged |
| `sweep.rs` uses the shared stamp constant | `src/sweep.rs` | existing sweep tests |

TODO row 15 is closed by this. It is a *capability*, not a ranking change: the recency
weight ships at 0.0, so retrieval order is unchanged.

### Harness changes in this cycle

| change | where | note |
|---|---|---|
| `recall_all@10` (every gold session in the top 10) added to the LoCoMo dev scorer and its artifact | `eval/locomo_dev.rs` (`recall_all`, `Row.all10`, `Agg.all10()`), `examples/locomo.rs` | session arm is metric-identical to the pre-change run |
| `--turns` arm: one memory per dialogue turn, session ranked at its best turn | `eval/locomo_dev.rs` (`Corpus::load_at`, `session_of`), `examples/locomo.rs` | artifact `eval/LOCOMO_TURNS.md` — a *rejected* experiment, see `CLOSED.md` |
| corpus occurrence times: `Document.timestamp` ingested via `api::parse_created_at` (H0) | `eval/locomo_dev.rs` (`Document::created_at`, `Corpus::load_at`); note + regeneration in `eval/LOCOMO.md` | makes LoCoMo date-aware for H3; control bit-identical at shipped `recency 0.0` — `locomo`, `locomo --turns`, `locomo_ranks` re-run under dated ingestion, every diff line provenance-only |
| H1.0 rank distribution of missed gold sessions | `examples/locomo_ranks.rs`, artifact `eval/LOCOMO_RANKS.md`, `CONSISTENCY.md` §36 | descriptive, selects nothing; gate fires for reordering levers (H1a–c) |
| H1a/H1b/H1c/H3 lever A/B (one pre-registered value each, shipped control in-run) | `examples/locomo_levers.rs`, artifact `eval/LOCOMO_LEVERS.md`, `CONSISTENCY.md` §37 | **all four gates failed** (H1a near-miss: R@1 +3.0pp, all10 +0.5pp < +1pp gate, single-hop R@5 −0.4pp); CLOSED.md rows added; 0 test consultations |
| H2 windows arm: `--windows W`, `Granularity` enum in `eval/locomo_dev.rs` | `examples/locomo.rs --windows 2`, artifact `eval/LOCOMO_WINDOWS.md` | W=2 pre-registered from the production turn-pair; gate failed (R@1 −1.1pp, all10 −0.4pp, R@pool →98.7%); granularity closed |
| H4 fixed 200-q reader-drowning subset | `examples/locomo_h4_subset.rs` → `eval/h4_subset.json`; `eval/amb/h4_build_subset.py` materialises the AMB input | eligibility all-gold-in-top-3 (1,035 eligible), proportional category split, seeded draw 42 |
| AMB adapter: `MEMORY_WIRE_TOPK` cap (H4) + scope-tag filters (H6) | `eval/amb/memory_wire.py` (copied into `../_audit/amb`) | PMB: 14→19/77, precision 0.24 at recall 0.95; `narrow_any` unhonoured, named |
| `answer_quality` stores sessions at `haystack_dates`, prefixes each served memory `[<date>]`, puts `question_date` in the prompt | `examples/answer_quality.rs` | **changes the instrument**: runs before 2026-10-14 are undated and not comparable |
| AMB provider sends `Document.timestamp` as `created_at`, reads it back from `full` recall | `eval/amb/memory_wire.py` (copied into `../_audit/amb/src/memory_bench/memory/`) | the committed baseline run predates this; see §3 |
| `eval/llm_shim.py`: unwrap/retry/total-deadline proxy between harnesses and aroute | `eval/llm_shim.py` | exists only to paper over `docs/AROUTE_BUGS.md` B1–B2; delete when aroute is fixed |
| H7 quiet-box `bench_concurrency` | `eval/baseline/2026-10-14-bench-concurrency/BENCH_CONCURRENCY.md` | **PASS**: 1.85× at 64 clients vs 1.50 floor, load 0.98→4.33, RSS flat 12.9 MB; the 2026-10-13 failure was load noise (`CONSISTENCY.md` §40) |
| `eval/baseline/run_baseline.sh`: whole battery, resumable `amb_rag` | `eval/baseline/` | see `RUNBOOK.md` |

## 2. Current numbers (every one traces to a committed artifact)

Baseline directory: **`eval/baseline/2026-10-13-v0.7.0/`** (see its `README.md` and
`PROVENANCE.md`). Load was 11–28 on 24 cores throughout: retrieval metrics are
load-independent, **every latency/throughput/RSS figure is range-quality.**

### Retrieval (free, deterministic)

| | R@1 | R@5 | R@10 | NDCG@10 | n | artifact |
|---|---|---|---|---|---|---|
| LongMemEval-S, all (**test**) | 83.8% | 97.2% | 98.6% | 88.2% | 500 | `LONGMEMEVAL.md` |
| LongMemEval-S, 470 (abstention excluded) | 84.0% | 97.2% | 98.5% | 88.5% | 470 | same |
| LoCoMo (**dev**), `overlap 0.25` shipped | 60.4% | 86.0% | 94.2% | 73.6% | 1,531 | `LOCOMO.md`, repo `eval/LOCOMO.md` |
| LoCoMo `recall_all@10` (new), shipped | — | — | **83.0%** | — | 1,531 | `eval/LOCOMO.md` |
| LoCoMo `recall_any@10` for contrast | | | 94.2% | | 1,531 | same |
| BEAM-100k (**dev**, pool-scale re-test §47): 1.00 / 0.25 / 0.00 | 42.5 / 39.9 / 36.4 | 67.1 / 63.6 / 62.3 | 77.6 / 76.4 / 74.8 | 42.3 / 41.2 / 39.3 | 313 | `eval/BEAM.md` |
| Frozen relevance battery (§48): auto gold 16/17, junk-injected 0/9, search-reachable 17/17; H8 gate green (43% less text) | | | | | 21 | `eval/relevance/BATTERY-2026-10-16-frozen.txt` |
| PrecisionMemBench (measure-only) | total 14/77, active 3/43, precision 0.17, recall 0.95 | | | | 77 | `precisionmembench.json.gz` |

Honesty note: `overlap 0.25` was chosen on LongMemEval (46 configurations) and is
**fitted** (`docs/EVALUATION_HYGIENE.md` §2.1). The honest unfitted LongMemEval R@5 is
93.0% (equal weight); 97.2% is the fitted number. Do not quote 97.2% as evidence that
anything is "better".

New 2026-10-14 (**H1.0**, dev set, no LLM — `eval/LOCOMO_RANKS.md`, `CONSISTENCY.md` §36):
of the 261 `recall_all@10` misses, the worst-ranked gold session sits at 11–15 / 16–20 /
21+ / absent = 89 / 73 / 99 / **0**. Every miss is an ordering position inside a complete
list (fused list = whole bank on 261/261); 62.1% within 11–20 ⇒ reordering levers (H1a–c)
are worth building, 99 misses at 21+ bound what they can recover.

### LLM-judged (reader and judge both aroute `small-stack`, temperature 0)

| | accuracy | n | artifact |
|---|---|---|---|
| `answer_quality`, LongMemEval-S (test), conditioned | 17.0% (85) | 500 | `ANSWER_QUALITY_LME.md`, `answer_quality_lme.json` |
| same, closed-book control | 6.0% (30) | 500 | same |
| AMB LoCoMo `rag` (measure-only) | **84.1%** (1,295) | 1,540 | `amb_locomo_rag.json.gz` |
| — open-domain / temporal / multi-hop / single-hop | 92.7% / 81.9% / 68.8% / 66.0% | 841 / 321 / 96 / 282 | same |
| — by gold coverage in the served top 10: all / some / none | 91.2% / 53.0% / 36.5% | 1,289 / 166 / 85 | derived from the same file |

How to read them (the full reasoning is `docs/CONSISTENCY.md` §34):

- The LongMemEval 17.0% is a **small-reader floor**, not memory quality: a gold session
  was served in 454/500 and ranked first in 419 of those, yet 368 of the 374 wrong
  answers were `I don't know`. Gold alone was answered 6/10, gold + 2 other sessions
  1/10 (10 items, diagnostic).
- On LoCoMo the reader does *not* refuse (12 of 245 wrong answers). Accuracy tracks
  whether **all** gold sessions were served — this is why `recall_all@10` exists.
- **Never read 84.1% against 17.0%.** Different corpus and bank size (LoCoMo banks hold
  19–32 sessions). Never place a retrieval number beside a judged one, or beside a
  number from a different reader/judge (`docs/BENCHMARK_LANDSCAPE.md` §6).

### Footprint and speed (range-quality — load 11–28)

Binary 10,745,080 B (10.25 MiB), `DT_NEEDED` 3; idle RSS 9.88 MB; cold start 13–16 ms;
retain 2,018 ops/s; recall p50 7.2 ms at 10k memories. **`bench_concurrency` failed its
2-client gate (1.30× vs 1.50×) under load but absolute throughput rose** — unreadable;
re-measure on a quiet box (`TODO.md` #14).

## 3. Instrument caveats a fresh agent will otherwise rediscover the hard way

- **The reader is a fallback chain.** aroute routes only combo names; `small-stack` is
  served by free-tier models, in practice `poolside/laguna-s-2.1:free` (3,542 of 3,572
  AMB-era calls). The served model cannot be pinned; `llm-calls.jsonl` records it per
  call. Two baselines compare cleanly only where the served mix matches. Free tier is
  slow at load (median 12 s/call on ~15k-token prompts).
- **The committed AMB LoCoMo row used the *old* adapter** (dates held client-side in a
  header prefix). The current adapter round-trips `created_at` through the product. The
  retrieval order is identical, so the expectation is a similar score, but a re-run is a
  new instrument version — put it in a new `TAG`, never over the old directory.
- **AMB `rag` trap:** the dataset prompt builders replace the formatted documents with
  `json.dumps(raw_response)` whenever a provider returns one. The adapter must return
  `None` there (it does; the first run returned metadata and scored 0/29).
- **AMB does not retry client timeouts** (SDK default 600 s). The shim therefore caps a
  call at 480 s and returns a 502 (which AMB retries). Do not remove `--deadline`.
- **The host reset three times** during the baseline (network outage 21:00–22:10, then
  hard resets 02:09 and 03:34 on 2026-10-14; no OOM/panic record precedes either reset).
  `amb_rag` resumes from AMB's per-conversation saves in `target/amb-out/$TAG`
  (gitignored). Relaunch the same command with the same `TAG`; it skips finished units.
- **aroute defects** (HTTP-200-with-error-body, cline `{"success","data"}` envelope,
  cline token headers 0, 403 without cooldown): `docs/AROUTE_BUGS.md`. When aroute is
  fixed the verification recipe is in that file; then delete `eval/llm_shim.py`.

## 4. Environment

| thing | value |
|---|---|
| build env | `export PATH=/usr/bin:$PATH` (a different `ar` in `~/.local/bin` breaks `libsqlite3-sys`) |
| live memory-wire serve | `127.0.0.1:8888`, bank `omp`, the user's **real** store — never write to it from a harness. Harnesses use scratch servers on 8897–8899 |
| aroute | systemd user unit `aroute.service`, `127.0.0.1:20128`, no client auth; do not restart/modify |
| AMB checkout | `../_audit/amb` @ `03c1d0f`, venv `../_audit/amb/.venv/bin/amb`; provider installed from `eval/amb/memory_wire.py` |
| datasets | `eval/data/` is gitignored; fetch with `./eval/download.sh` (LoCoMo, LongMemEval-S are already present on this machine) |
| disk | `/home` ~126 GB free at handoff |
| background jobs | the PMV2 triangulation chain is running (resume #2 from `--from-step 3`, `/var/tmp/pmv2_resume_chain.sh`); marker `PMV2 chain done` in `/var/tmp/pmv2/resume-launcher.log`; lands §49 + `eval/omnimevaleval/memory-wire-mw_pmv2_dated/`; on a daemon-restart wake, FIRST check :8899 health + the responses checkpoint, then relaunch (idempotent) |

## 5. What the LoCoMo judged run says about where the headroom is

From `amb_locomo_rag.json.gz` joined with `eval/data/locomo/queries.json`
(`gold_ids`) — derive again with the snippet in `RUNBOOK.md` §7:

- 1,289 of 1,540 questions had every gold session in the reader's top 10; those were
  answered 91.2%. The remaining 251 were answered 47.4% (119/251). So most recoverable
  headroom is *coverage of multi-session questions*, not single-gold ordering.
- `recall_all@10` misses concentrate where gold is plural: single-hop with ≥4 gold
  sessions 43/49 miss, multi-hop with ≥4 gold 10/10 miss, single-hop with 2 gold
  75/163 miss (from `eval/results_locomo.json`, shipped config).
- **H1.0 (2026-10-14) settled where those missed sessions sit**: worst gold at 11–15 /
  16–20 / 21+ / absent = 89 / 73 / 99 / 0 of 261 misses; per missed session
  146 / 113 / 115 / 0 of 374 (`eval/LOCOMO_RANKS.md`). Zero absent and the fused list
  covers the whole bank on every miss, so **no miss is a matching failure** — the
  pre-registered gate fires for the reordering levers (H1a–c); the 99 queries at 21+
  are out of a reordering lever's reach and are the semantic residue (H1d, user's call).
- **H1d (dense arm) measured 2026-10-15 — gate FAILED** (`CONSISTENCY.md` §41,
  `eval/SELECTION_VECTOR_AXIS_ALL10.md`): `recall_all@10` +1.8pp best at vec=0.75,
  bought at −9.9pp R@1; deep-miss rescue **4/99** — the arm reorders the shallow end,
  it does not reach the semantic residue. No test consultation. The `--features embed`
  capability stays opt-in/off-by-default; the promotion decision is now informed and
  the evidence says no. **This closes every open H1 lever; the R&D queue is empty
  except condition-gated Tier-3 items.**
- **The reordering levers were then measured once each and all failed their gates**
  (`CONSISTENCY.md` §37, `eval/LOCOMO_LEVERS.md`): H1a `ExactAndPrefix` near-miss (R@1
  +3.0pp but all10 +0.5pp, single-hop R@5 −0.4pp), H1b `BestSentence` (all10 −0.3pp,
  multi-hop R@1 −11.3pp), H1c `bm25_magnitude` (all10 −0.2pp). H3 recency was
  falsified on the dated corpus (temporal R@1 −33.4pp, control exact), and H2 windows
   W=2 lost too (R@pool →98.7%). **The shipped configuration survived every challenger**
   — the 99 deep misses remain H1d's (user's call), and precision levers await H4's
   verdict (landed: AMB rag k=3/5/10 on `eval/h4_subset.json`; **flat: 92.0/94.0/94.0%**
   at median context 4.3k/7.2k/14.6k tokens, zero refusals, n=200 per k ⇒ the drowning
   failure does not reproduce on the dev set; TODO #17 closed "not reproduced", and no
   precision lever is built — `CONSISTENCY.md` §38). H5 dated `answer_quality` also landed:
   **18.6%** conditioned (93/499, 1 unscored), closed-book 6.0%, delta +12.6pp — beside the
   undated 17.0% as a different instrument, consultation #3 (`CONSISTENCY.md` §39,
   `eval/baseline/2026-10-14-v0.7.0-dated-answer-quality/`). The disagreement shape is
   unchanged: gold served 90.8%, 73.0% served-and-wrong — the answerer, not the ranking.
- AMB `rag` serves k = 10 of a 19–32-session bank. That cut is a *harness* parameter,
  not a memory-wire one. The adapter now carries `MEMORY_WIRE_TOPK` to move that cut
  for measurement; H4 used it to fix the served set at k = 3/5/10.

<!-- genome:zone status sha=3d6e7c329f1b -->
| status | items |
|---|---|
| open | 5 |
| done | 32 |

- `10` [open] agent-memory-bench (GiulioDER) Claude Code arm
- `11` [open] LLM-judged runs: AMB `rag` (BEAM/PersonaMem/LifeBench), OAMB LME-60, memorybench ConvoMem, MemBase R
- `12` [open] AMB provider writes at the product's granularity (turn pairs, as the hooks do), not 25k-token sessio
- `13` [open] aroute: 200-with-`{"error"}` logged `outcome=ok` with no failover (B1); cline replies forwarded insi
- `38` [open] ~~**the enumerated cross-door parity check both review rounds asked for**~~ **half done 2026-11-21**
<!-- genome:end status -->
