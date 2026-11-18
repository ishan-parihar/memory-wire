# Reviewer prompt — memory-wire, the shipped architecture, round 2

You are reviewing a small Rust agent-memory service that has just finished a
multi-quarter upgrade. You have no session history and no repo. The pack is all
you know. **Treat the pack's claims as CLAIMS TO AUDIT, not as ground truth.**
Where the pack does not settle a question, say **"not in evidence"** and continue
under a stated assumption — do not fill the gap with plausible generalities.

## What changed since the last round

Round 1 reviewed a *plan*. That plan has since been **implemented and shipped**:
schema columns, mount profiles, retroactive redaction, a deterministic hygiene
scan, a startup grounding capsule, an MCP supersede verb, a `connect --with-skills`
installer — then five batches fixing audit findings in those changes. The tree is
`555d1aa`. **This round reviews the thing, not the plan.**

## The decision this round exists to test

The project's stated goal is to be **the cheap option**: a single static binary
(10,937,128 B per `G-MEASURE`), 3 shared libraries, no daemon required, retrieval
that beats the one competitor we can compare against. The upgrade added a *lot* to
that binary.

The question is not "is each feature correct" — the repo says each is tested. It
is:

> **Where is the architecture now — as a system an agent actually lives inside —
> and what should the next iteration be?**

Name four things, and for each say whether it is a **bottleneck**, a
**redundancy**, a **fragmentation**, or an **integration gap**, and whether it is
worth fixing at all. A "none here, and here is why" is a valid answer.

## Five questions

1. **Bottlenecks.** Where does the system force work onto a path that cannot
   avoid it? Think about: the write path (redaction, hashing, dedup, version
   rows), the recall path (FTS5, fusion, budget trim), the hook path (five
   events, each spawning a process), the startup path (grounding capsule).
   Name the *measured* cost where the pack gives one; name it as unmeasured
   where it does not.

2. **Redundancies.** The surface now carries HTTP routes, 5 MCP tools, a CLI,
   a daemon, an installer across six hosts, a plugin, a skill file, a hygiene
   module, a grounding module, a sweep module. Which of these *duplicate* a
   capability another already has? Which are dead weight a user never reaches?
   Name the duplication, not a style preference.

3. **Fragmentations.** The architecture doc says "one service, three doors" and
   "there is no second implementation of recall". Test that claim. Is
   `recall` really reached identically from HTTP, MCP, the CLI and the hook?
   Does `hygiene` have a surface? Does `grounding`? Does `redact`? Where does
   a capability exist in one door and not another?

4. **Integration gaps toward an optimal agentic experience.** If you were the
   agent whose context this fills, what is still wrong after everything
   shipped? Be concrete: what arrives in your context, at what moment, from
   which door, and what is wrong with it. The pack carries
   `docs/OPEN_HOOK_RECALL_RELEVANCE.md` — a defect diagnosed and deliberately
   unfixed — and it carries `TODO.md`, which lists what the project thinks is
   left. Judge whether those lists are **complete**: name what is on neither.

5. **What the next iteration should be.** Given 1–4, what is the single
   highest-value next move — one item, not a roadmap — and what is the
   *cheapest* thing that would be a mistake to do instead? Defend the choice
   with a named anchor, and say what would have to be true for the opposite
   choice to be right.

## Constraints on your answer

- **Every finding names an anchor**: `# SECTION <NAME>` or `# FILE: <path>`.
  A finding with no anchor is an assumption — label it one.
- **Every figure keeps its three attributes**: population, basis, selection
  constraint. A number without them is not usable. `G-MEASURE` carries the
  surface count with its exclusions named; if you cite a count, say which
  population it is over.
- **Questions 1–4 are pack-only.** The pack is the only admissible evidence.
  If it does not settle one, say `not in evidence` — do not answer it from the
  web.
- **Question 5 is open to outside sources.** If the wider agentic-memory field
  has a mechanism this system should know about, cite it inline with its URL
  and the date you read it, and mark it as web-sourced. A web-sourced finding
  without a URL is an assumption.
- **A rejection is as useful as a finding.** If you conclude the architecture
  is correctly scoped and the lists are complete, say so and say why. Do not
  manufacture work to be helpful.

Reply in markdown, ≤ 2,500 words, numbered findings matching the questions above.
For each finding, say which it is: **from the pack** (name the `# SECTION` or
`# FILE:` anchor), **from a cited source** (give the URL), **my own inference**,
or **not in evidence**.