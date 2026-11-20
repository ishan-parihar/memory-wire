# Reviewer prompt — round 5

You are reviewing a Rust agent-memory service called `memory-wire`. You have no
prior context on it. Everything you know is in the pack.

**Treat the pack's claims as CLAIMS TO AUDIT, not as ground truth.** Where you
cannot find support in the pack, say **not in evidence** rather than accepting
the claim or inventing support. Where two numbers in the pack disagree, name
which one is wrong.

## What the system is

A single-binary memory service: a bank-isolated SQLite store with FTS5,
exposed over HTTP and MCP, plus hooks that capture conversation prose into it.
Four surfaces reach one core service: Rust hooks, an MCP server, the HTTP API,
and plugin adapters for agent hosts (two TypeScript, one Python). Its stated
competitive position is **the cheap option**: one static binary, ~10.64 MiB, 3
shared libraries, 17 dependencies, no daemon required.

## The question

**Where is this architecture actually bottlenecked, redundant, fragmented, or
gapped on the path to an optimal agentic experience — and which one should be
fixed next?**

Not whether it is well built; four prior rounds established that. I want the
**ranked** answer, earned from the pack.

## What the prior rounds established, so you do not re-derive it

1. **Round 1** asked whether an upgrade plan was the right shape. Yes on
   substance, no on process (the pack omitted the plan body).
2. **Round 2** asked where the shipped architecture was. It adopted one thing:
   the per-turn hook path injects recall with no relevance gate and no trust
   gate. It refuted two claims — that the TypeScript plugins and the Rust hook
   had diverged on the short-prompt rule (implemented in all three), and that
   empty hook output violates an output-conventions rule (it is the never-fail
   contract, which requires silence).
3. **Round 3** asked whether the next cycle should keep optimising retrieval
   ordering. No; it named the asymmetry in how policies are enforced across
   doors.
4. **Round 4** asked whether the `connect_*` per-host family is the right shape
   or the next site of that asymmetry. Its conclusion: the asymmetry is about
   cross-surface capabilities, and the smallest fix is an *enumerated parity
   check*, not a shared abstraction. **Round 4 was never verified** — it was
   sent and no verification pass was written. Treat its specifics as
   unverified; its two concrete instances were both closed the next morning
   (`B-DELTA` D0).

## What changed since round 4, and why it matters

`B-DELTA` is the whole of it. The three items that should move your ranking:

- **A plugin door had never loaded on its host.** The opencode plugin exported
  the wrong shape for opencode 2.x and was installed in a layout 2.x skips with
  no error and no log line. The round-4 review had examined that door and
  called it healthy. This is the asymmetry one layer out: the check covered
  what reaches a model and could not see that *nothing* did.
- **Round 4's stated falsifier is now measurable.** It could not tell whether
  `connect_*` is mostly inline test fixtures or duplicated production wiring.
  `measure_surface.sh` now reports 52% test / 48% production. That is the
  evidence it asked for; decide what it implies.
- **Three gates now exist that previously ran nowhere.** One of them caught
  real drift on its first outing.

## Two readings I cannot choose between

- **(A)** The real bottleneck is the **per-turn injection path**. It has not
  been re-measured since round 2, the finding that a plugin door can be
  silently dead on its host is new, and the relevance question is
  deliberately unfixed.
- **(B)** The real bottleneck is **verification, not code**. Most recent defects
  were found by an external reviewer rather than by the system's own checks.

Decide between them on the pack's evidence. Say what would change your mind.

## What I need back, in this shape

A ranked list. For **each** item:

1. which axis it is — **bottleneck**, **redundancy**, **fragmentation**, or
   **integration gap**;
2. the **artefact and the line** that evidences it. Quote the file. A finding
   I cannot open is a finding I cannot act on.
3. its **cost against the "cheap option" position**, in the unit that position
   is measured in (binary bytes, dependencies, shared libraries, or a latency
   figure — say which);
4. **what would have to be true for the opposite choice to be right.** A
   falsifier, not a caveat.

Then answer these specifically, because they are where I am most likely to be
wrong:

- **The `connect_*` question.** 52% of those lines are inline tests. Given
  that, is a shared per-host abstraction worth its cost, or is the enumerated
  parity check the better instrument? Cite the measurement, not the intuition.
- **The four auto-injection doors** (`src/hooks.rs`, `plugin/integrations/hermes/memory-wire/__init__.py`,
  `extension/memory-wire.ts`, `opencode-plugin/memory-wire.ts`). They implement
  the same policy in three languages. Name the *specific* place they still
  disagree, if one exists — `B-DELTA` D0 closed the one I knew about, and I
  would rather learn of another from you than from a user.
- **What an agent hits first.** Given 13 CLI subcommands, 10 HTTP routes,
  6 MCP tools, 1 prompt, 8 agent hosts and 4 plugin doors, which of those does
  an agent reach for and not find?

Finally: **one thing the pack does not contain** that you would want before
committing to your ranking. Name it. Do not invent it.

## Discipline

- Do not re-open a lever listed in `F-CLOSED` (`docs/handoff/CLOSED.md`); each
  was closed on a measurement, and `D-LAW` says rejected work stays rejected.
- Do not propose a score threshold on the injection path. `D-LAW` prohibits
  it, and `G-RELEVANCE` gives the reason: min-max normalisation makes the top
  hit of an irrelevant set score 1.0.
- The pack is large. If you run low on room, answer the ranked list first and
  say what you did not reach — do not truncate silently.
- Prefer naming a real defect at a real line over a generality argued from the
  pack's prose. I will verify every line you cite.