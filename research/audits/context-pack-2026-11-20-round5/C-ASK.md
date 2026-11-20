# C-ASK — what this round exists to decide

## The question, stated plainly

**Where is memory-wire's architecture actually bottlenecked, redundant,
fragmented, or gapped on the path to an optimal agentic experience — and which
one of those is the one to fix next?**

Not "is it well built". Four rounds have established that it is. This round asks
for the *ranked* answer, with the ranking earned from the pack rather than
asserted.

## What the last four rounds established (do not re-derive)

| round | question | verdict, in one line |
|---|---|---|
| 1 (11-18) | is the W0–W6 plan the right shape? | yes on substance, no on process (the pack omitted the plan body — my defect) |
| 2 (11-18) | where is the shipped architecture? | per-turn hook injection is un-gated on relevance and trust → **adopted as the next move**; four bookkeeping defects; two refutations that must not be acted on |
| 3 (11-19) | should the next cycle keep optimising retrieval ordering? | no — it named the door-enforcement asymmetry instead |
| 4 (11-19) | is the `connect_*` family (i) the right shape or (ii) the N−1 risk? | (ii) holds for cross-surface capabilities; the smallest change is an *enumerated parity check*, not a shared abstraction. **Never verified** — the round was sent at 23:52 and no `VERIFICATION.md` was written |

Round 4's two *instances* were closed by D0. Its *conclusion* stands and is the
starting point here.

## The three things that changed the picture since round 4

1. **A door was dead and nobody knew** (D1). The opencode plugin did not load on
   opencode 2.x at all — wrong export shape, wrong install layout, and a flat
   file is skipped with no error. The round-4 review had called that door
   healthy. That is the N−1 shape one layer out: the check covered what reaches
   a model and could not see that *nothing* did.
2. **Round 4's falsifier is now measurable** (D6). It could not tell whether
   `connect_*` is mostly inline test fixtures or duplicated wiring. Answer:
   **52% test, 48% production** (3,489 / 3,229 of 6,718). That is evidence for
   one of its two readings, and this round should say which and why.
3. **Three gates now exist that previously ran nowhere** (D5). One of them
   (`module_table_check.sh`) caught real drift on its first outing.

## The tension to weigh

The stated competitive position is **the cheap option** (`AGENTS.md` §1): one
static binary, 10.64 MiB, 3 shared libraries, 17 dependencies, no daemon
required. Every structural change is paid against that.

Two readings of the same facts, and the pack deliberately does not pick one:

- **(A) The architecture's real bottleneck is the per-turn injection path.**
  Round 2 adopted this and it has not been re-measured since. The hook path
  injects up to the recall budget on every turn, and
  `docs/OPEN_HOOK_RECALL_RELEVANCE.md` is still deliberately unfixed (no score
  threshold, ever). The new D1 shows the injection doors can be *silently
  dead* — which is a worse failure than a noisy injection.
- **(B) The architecture's real bottleneck is now verification, not code.**
  Four of the last five defects were found by an external reviewer, not by the
  system's own checks (round 4's framing). Since then three gates were wired.
  Is that enough, or is the pattern that gates get written for what was just
  found and not for the next shape?

Both are consistent with the pack. **Decide, and say what would change your
mind.**

## What is NOT being asked

- Whether the recall pipeline should change. Ranking is frozen: every lever in
  `docs/handoff/CLOSED.md` was closed on measurement, and AGENTS §8 says
  rejected stays rejected.
- Whether to add a score threshold anywhere. Prohibited (§32/§48).
- Whether to cut a release. That is the operator's call (AGENTS §11) and is
  measured in CONSISTENCY §68.
- Whether `vector.rs`/`embed.rs` should be deleted. Closed by measurement
  (H1d) and they compile to zero bytes.

## The output I need

A ranked answer to the question, where each item carries:

1. **which of the four axes** it is (bottleneck / redundancy / fragmentation /
   integration gap),
2. **the artefact and the line** that evidences it — not a restatement of the
   pack's framing,
3. **its cost against the "cheap option" position** (§1), stated in the unit
   that position is measured in,
4. **what would have to be true for the opposite choice to be right** — the
   falsifier, not a caveat.

And then one thing the pack does not contain that you would want before
committing to the ranking. Name it; do not invent it.
