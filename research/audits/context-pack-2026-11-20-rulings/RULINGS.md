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
