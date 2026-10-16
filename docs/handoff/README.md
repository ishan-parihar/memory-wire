# Handoff — continuing the memory-performance R&D

Written 2026-10-16 for a fresh agent with no session context. Everything here is
checked against the working tree and committed artifacts on that date; where a
claim could not be checked it says so. **Artifacts win over prose** (AGENTS §7):
if a number here disagrees with an artifact, the artifact is right and this file
is stale.

## Where the project stands

`v0.8.2` is **released** (2026-10-16; the day also cut v0.8.1 — `docs/CONSISTENCY.md`
§45 records both, including an `update`-command bug found on the first real hop and
fixed in v0.8.2). The tree is clean and pushed. **The pre-registered R&D queue is
empty**: every H-item to date is terminal — measured and shipped (H0, H8), measured and
rejected (H1a–d, H2, H3), falsified (H3), flat (H4), unmeasurable-no-build (H11), or
already-shipped (V4) — see `RND_PLAN.md` §4's results log, one row per verdict.
What remains is either **blocked** (H9 BEAM labelling; Track 2 agent-budget) or
**decision-gated on the user** (capsule adoption by the surfaces, a second-system
HaluMem head-to-head, answerer-side product work, the next release).

## What the job is

Raise memory-wire's *memory performance* — retrieval ordering/coverage and, through
it, the judged answer accuracy of a reader — **without** (a) breaking the
single-small-binary position (AGENTS §1), or (b) overfitting the evaluation set
(AGENTS §6). The second is the rule most easily broken by accident and the one this
whole R&D process exists to enforce.

## Read in this order (≈25 min)

| # | file | why |
|---|---|---|
| 1 | `AGENTS.md` §1, §2, §6, §7, §8, §11, §12, §13 | the rules. §6 (three-set discipline) is non-negotiable |
| 2 | `docs/handoff/STATE.md` | the current state: releases, every headline number and where it lives |
| 3 | `docs/handoff/RND_PLAN.md` | the register: §4 results log (every verdict to date), §5 the survey-sourced cycle (all terminal), each H-item with mechanism, test, gate |
| 4 | `docs/handoff/RUNBOOK.md` | exact commands, environment traps, how to run the baseline and the LLM instrument |
| 5 | `docs/handoff/CLOSED.md` | levers already measured and rejected — **the complete list**; do not re-propose without new evidence |
| 6 | `TODO.md` | the implementation register (open rows are numbered there; treat it as authoritative, not this file) |
| 7 | `docs/COMPETITOR_GAPS_PLAN.md` §6 | what the six-repo field audit (agentmemory, Hindsight, MemOS, Honcho, OpenViking, OmniMemEval) found and how each candidate was vetoed — read before proposing anything a competitor has |

Deeper references, only when a hypothesis points there: `docs/EVALUATION_HYGIENE.md`,
`docs/BENCHMARK_LANDSCAPE.md` (the battery, roles, comparability rules; §4.6 has
OmniMemEval), `docs/RERANKING_PLAN.md` §3 and `docs/PERFORMANCE_PLAN.md` (the
*historical* rejected-lever records; CLOSED.md is the complete current list),
`docs/EXCEED_PLAN.md`, `docs/CONSISTENCY.md` (append-only; **§36–§45 are this
cycle's**, §34–§35 the ones before).

## The five rules that bite

1. **LoCoMo and the relevance battery choose. LongMemEval-S only measures.** A change
   picked by looking at a LongMemEval number is *fitted*, not earned (AGENTS §6). Any
   harness that runs LongMemEval (OAMB, MemBase, memorybench, AMB `longmemeval`,
   `answer_quality`, OmniMemEval's LME arm) *is* a test consultation: count it in
   `docs/CONSISTENCY.md`. Five consultations exist to date, all measurement-only.
2. **Decide by mechanism, then measure once.** Write the mechanism argument *before*
   the number. Never iterate until a metric improves. A null result is a result;
   record it. The 2026-10-14/15 cycle is the worked example: ten levers, ten terminal
   verdicts, zero iterations.
3. **Releasing is the user's call** (AGENTS §11). Working trees get committed and
   pushed as ordinary work; tags, releases and CI-disabling happen only when the user
   ratifies a release. The release chain itself is protocolized — §45's two-commit,
   gates-green, CI-cycled, REST-upload shape is the template.
4. **Do not edit the aroute repo** (`agentic-utility/proxy/artificial-route`) or restart
   its service; do not look at its credentials. Its bugs are handed off in
   `docs/AROUTE_BUGS.md`. Never send real API keys to harnesses: `env -u GOOGLE_API_KEY`.
5. **Delegate only with the `space-bunny` model** (AGENTS §11), and never cite a
   subagent's summary as a measurement — run the gate yourself (AGENTS §12).

## Decisions that belong to the user, not to you

Stop and ask (in your final report, not mid-run) before any of these:

- turning the dense-vector arm on by default (costs binary size and a model download;
  measured 2026-10-15: +1.8pp `recall_all@10` best case at −9.9pp R@1 — §41);
- a paid/frontier reader or judge for any LLM run (the instrument is free-tier
  `small-stack` through `eval/llm_shim.py`);
- committing to a release, tagging, pushing a tag, publishing;
- changing a shipped default in `FusionWeights::SHIPPED` (even with dev-set evidence:
  report it, mark it `provisional:` if it was ever LongMemEval-informed, and let the
  user decide);
- adopting capsule injection on any surface by default (H8 is shipped opt-in; the
  adoption call cites `eval/relevance/BATTERY-H8-2026-10-15.txt` and is the user's).

## Definition of done for a hypothesis

A hypothesis is closed when (1) its mechanism argument is written, (2) the dev-set
measurement exists as a committed artifact under `eval/`, (3) `docs/CONSISTENCY.md`
has an appended section with the numbers, the n, the loadavg, and the
test-consultation count, (4) the gate in `AGENTS.md` §2 passes, and (5) `TODO.md` and
`docs/handoff/RND_PLAN.md` status lines are updated. "It looked better" is not done.
