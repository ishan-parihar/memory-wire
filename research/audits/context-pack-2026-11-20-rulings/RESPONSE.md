# Reviewer reply — rulings round, 2026-11-20

Received as a chat message (not via `RESPONSE.md`); transcribed verbatim.

Model `gemini-4-hydrogen`, default effort, per the operator's ruling.

---

I'll start by reading the uploaded pack, since only its path is in my context.

Status: I've now read the whole pack and am running the arithmetic checks, covering the module-table sum, the TODO counts and the line anchors. After that I'll write up the rulings.

I read the whole pack: 1,971 lines, nine sections. No outside sources were used. Every claim below is pack-only, and where the pack is silent I say so.

## Rulings

I use RULINGS.md's letters. The plan and your prompt letter R1 differently: the plan's "(c) fence now" is RULINGS' (b).

**R1 — recommend (b), fence it, built as a behavioural check. Defer (a).**

- **Reasoning.**
  - The cited cost of R1 doesn't belong to R1. The round-5 footer bug lived in Rust and hermes (`# FILE: src/hooks.rs`, hermes `prefetch` comments: "same defect… fixed the same day"). The TS doors render no footer at all.
  - The one TS-vs-other defect (the trust gate missing in both TS files, `# SECTION I-GAPS`) was TS against Rust/hermes, not TS against TS.
  - The only TS-vs-TS divergences in the pack sit in host glue, outside the 8 shared functions. The extension has `if (stored) lastRetained = content`, while opencode has `if (stored !== null) …`. The cache-freeze bug exists only in opencode.
  - So (a) removes duplication where no defect has been observed. It also leaves four copies of the policy (Rust, Python, two TS) as three.
  - "include_str! concatenation… spliced by the trust gate" names two different mechanisms. One is a Rust compile-time macro and the other is a shell test script.
  - `connect` edits the installed TS files. TODO 1 says it reads the pin from the installed file's `DEFAULT_BANK`, and TODO 35 says the version marker moved 2→3. A splice has to keep those anchors intact.
  - Opencode's install is already a directory (`plugins/memory-wire/index.ts`, TODO 35). The objection to (d), "rewrites live installs", therefore holds only for omp/pi.
- **The fence must compare behaviour, not text.** The two `formatRecall` doc comments already differ: the extension omits the "label line exists instead" sentences, and the line wraps differ. A byte or golden diff would be red on day one. `trust_gate_test.sh` already executes each file's real `formatRecall` in node (`# SECTION I-GAPS`). Extending it to assert equal output on shared fixtures costs about half a day.
- **What would make (a) safe.**
  - The generated file is committed, with a regenerate-and-diff check.
  - The pin round-trip and marker tests run over the generated file.
  - Both hosts really load it. Plan §7 admits no live 2.x load has been checked since the port.
- **Gap and the one measurement.** Count how often history touched a shared function in one TS file but not the other, using `git log -L` on the 8 functions in both files. If it is zero, (a) buys nothing observed.
- **Defect class.** (b) reduces silent drift and adds no seam. (a) reduces duplication but adds a generated-file seam and a coupling to `connect`'s anchors.
- **Confidence.** Medium. The pack carries only 154 of 356 and 306 of 479 TS lines. My recommendation also matches the agent's own plan preference (§2.1), so you would be ratifying a preference the agent already holds.

**R2 — recommend (b), widened. Don't build (a) as specified.**

- **"CI-only" defeats the purpose.** `ci.yml` fires only on PR, dispatch or tag, and Actions quota is "currently exhausted" (`# FILE: AGENTS.md (lines 357,396)`). A CI-only suite runs nowhere until a tag push, which is the "check that ran nowhere" failure class. It must live in the local gate or `cargo test`.
- **"One fixture set" doesn't mean one expected output.**
  - Rust and hermes render `###` headers, `- ` bullets, entry-dropping on a character cap, and a "…and N more" footer.
  - The TS doors render `##` headers and numbered lines, slice the whole block at `INJECT_CAP_CHARS-1`, and have no footer.
  - Rust counts chars, Python counts code points, and TS counts UTF-16 units. Section-cap overflow will therefore diverge by design, and someone has to rule which semantics are intended. That ruling is unpriced.
- **Most fixtures don't discriminate.**
  - Failed-then-recovered `/recall` matters only for opencode, the only door with an `injectedQuery` cache. Rust is one-shot, and hermes and the extension are stateless in the carried code. `cache_source_check.mjs` and `cache_rule_test.mjs` already cover it.
  - Blank content has already converged in the pack's code (see audit item 6).
  - Repeated prompts diverge in retained shape. Hermes writes `turn <id>\nuser:…\nassistant:…` with no `lastRetained` check and `_djb36(user)`. The TS doors write `asked:/answered:` with `hash(prompt)`. Neither hash body is carried.
  - `format=summary` is env-only in TS and env-plus-bank-config in Rust. Whether hermes reads it at all isn't visible, because `_recall` isn't carried.
- **The biggest evidenced failure class is out of reach.** TODO 35's dead opencode door passed a stub-ctx drive and failed only on real opencode 2.3.2. A driver-with-stubs suite would have passed it.
- **The estimate.** 2–3 days is credible for the mechanics only: a loopback stub, four thin drivers, and the existing opencode stub-ctx. The spec decisions above are unbounded. The host count is also unsettled (see audit item 9).
- **Cheapest version that still catches the class.**
  - A cross-door "included row ids" invariant for three cases: trust-before-take starvation, blank skipped before take, and shown + omitted = candidates (Rust and hermes only).
  - A `Host::ALL` unit test in `cargo test` that fails when a host has no door-table entry.
  - A real-host load check that prints SKIPPED loudly when the host isn't installed.
- **Gap and the one measurement.** Write down, per fixture, the expected row-id list for each door. If any cell can't be filled without a design ruling, that cell is the true cost.
- **Defect class.** It reduces N-1-of-N divergences in trust, blank and footer handling. It adds a fifth place the policy is encoded (the oracle) and a false-assurance risk: TODO 38's falsifier says a green suite "closes the verification bottleneck".
- **Confidence.** Medium.

**R3 — recommend (a), measure now. Keep the cut as a separate, later word.**

- **Reasoning.**
  - The only figures in the pack are stale by its own admission. 10.65 MiB was measured on a binary built at `6a2a125` that predates every round-5 fix (`# SECTION A-STATE`, plan §1.3), and the installed binary is 0.9.0.
  - Measuring is read-only. Cutting is not: `build-release.sh` refuses to clobber a release that has assets, and a tag moves only by history rewrite with the owner's go-ahead (§11). The cut is the one non-undoable act across the three rulings.
  - R1(a) and R2 reach users only through a release. TODO 35 says the opencode door "shipped" dead on 2.x. If that shipped in a published release, deferral leaves a broken released artefact. Which release it was is not in evidence.
- **Gap and the measurement.**
  - The commit counts are contradictory (audit item 5). Run `git tag -l 'v0.*'; git rev-list --count v0.9.0..HEAD; git rev-list --count v0.10.0..HEAD`.
  - A fresh `cargo build --release --locked`, then `stat -c%s` and `readelf -d | grep -c NEEDED`.
  - Not in evidence: whether `build-release.sh` has a no-publish mode. The "three-Linux asset set" can't be built safely until that is checked.
  - Not in evidence: what "grow materially" means. HEAD is +0.40 MiB (+3.9%) over v0.7.0 (10.6487 vs 10.25, my arithmetic), and no pass/fail line is defined.
  - Whether 60 commits is one coherent release is, as the sheet says, a judgement and not in the pack.
- **Defect class.** It reduces stale-figure and installed-vs-tree drift, which is the repo's most repeated defect. The cut adds irreversibility.
- **Confidence.** High on measure-now. Low on any cut until the tag question is answered.

## Ranking

1. **R3.** It has the only irreversible leg, and it gates how anything else reaches users. The measurement it asks for is also what R1 and R2 are missing.
2. **R1.** If (a) were chosen and shipped, a generated file lands on installs under `connect`'s anchors, and undo needs another release. (b) is almost free to undo.
3. **R2.** The worst case is wasted days plus false assurance, and deleting `scripts/` undoes it.

The two that matter most are R3 and the (a)-vs-(b) fork inside R1. Settle the tag and footprint facts first, because they decide how costly an R1(a) mistake would be.

## Baseline audit

1. **No script output is in the pack.** RULINGS says every figure is "produced by" `measure_rulings.sh`, but only the script is carried, with no output. The script also never tests identity: it prints name lists for a human to compare.
2. **The script's regexes bias the counts.** `^var [A-Z]…` misses `const` and `let`. `^(export )?function` skips `async function` and `export default function`. "9 functions, 8 identical" is therefore partly a regex artefact. Opencode's `lastAssistantTextFor` is an `async function`, so "doors differ only in `lastAssistantText` and `setup`" omits a second glue function. `post` is also uncounted and unverified, although opencode's own docstring lists it as shared code.
3. **The 13-constants claim can't be checked from the pack.** The visible excerpts name 12 distinct constants: `UNTRUSTED_TIER`, `RECALL_ENTRIES`, `ENTRY_CAP_CHARS`, `INJECT_CAP_CHARS`, `SHORT_PROMPT_CHARS`, `QUERY_CONTEXT_CHARS`, `RECALL_BUDGET`, `EXCLUDE_TAGS`, `RECALL_TIMEOUT_MS`, `RETAIN_TIMEOUT_MS`, `MIN_ANSWER_CHARS` and `RETAIN_PART_CAP_CHARS`. "Zero divergence" holds for the visible code only. Comments diverge.
4. **R1's cost evidence is misattributed** (see R1 reasoning).
5. **The commit and version figures conflict.**
   - The plan says 59 commits ahead of installed 0.9.0 (§1, §3.2) and 59 past `v0.10.0` (§3.1), while RULINGS says 60.
   - If `v0.10.0` is a tag later than 0.9.0, the two 59s can't both hold. The pack never shows the tag exists.
   - Plan §3.1 also labels 10.65 MiB as "v0.7.0's baseline". AGENTS puts v0.7.0 at 10.25 MiB, and 10.65 is HEAD at `6a2a125` (11,166,048 B = 10.6487 MiB, which checks out).
6. **The plan's Tier-1 premise is stale against the pack's own code.**
   - Plan §1.2 and TODO 37 say `hooks.rs` and hermes count blank rows. The carried `recall_section` and hermes `prefetch` already filter them, citing round 5.
   - Row 37's acceptance is struck through but its status still reads `open`, so the "8 open" figure includes a finished row. I count 7 genuinely open. I can only count the carried rows (18 done, 8 open), so the plan's "30 done" can't be checked.
   - The module table sums to 39,871 across 33 rows, matching AGENTS. The plan's 39,778 is 93 lines older.
   - Your prompt names HEAD `05672ec`, but the pack names only `84ba417` and `6a2a125`. I can't confirm the pack describes `05672ec`.
7. **A count was corrected in 9 places and missed a tenth.** Hermes `get_tool_schemas` says "The same four tools `memory-wire mcp` serves". MCP serves six. TODO 34's list of places corrected omits hermes, which lacks redact and supersede. The INTEGRATION_GAPS asymmetry table has no hermes column.
8. **The TODO 37 line citation is stale.** `opencode-plugin:213` is 215 in the carried excerpt, while the extension's `:241` checks out.
9. **The host count is inconsistent.** RULINGS and the plan say the old script listed "6 of 8" hosts, TODO 4 says "seven hosts", and R2 says "a 7th host". `Host::ALL` is never quoted.
10. **Evidence classes are unusable.** The reading contract asks that figures keep `[LIVE]`, `[CERT]` and similar tags. No figure carries one, and no row is tagged `[UNCONFIRMED]`. The only tags are the two `[agent-held]` items (R1(d) and R3's owner line).
11. **R2 is misclassified.** Plan §2.2 calls the parity suite "not blocked on you" and "Tier 1 by my own classification", yet RULINGS lists it as operator-owned. RULINGS says it excludes rows an agent can decide, so what you are really being asked for is a budget green light.

## What the plan does not see

- **It never asks whether a memory written in session 1 reaches session 2's prompt on a real host after `connect`.** TODO 35's dead door was found "live… while probing B2" by accident. Plan §3.4 concedes every ranking is "inference from static retrieval plus unit checks", then parks the one live-job benchmark (TODO 10) as external-blocked. A scripted per-host write-then-read smoke test would be cheaper than R2 and would target the worst failure class on the record.
- **It never asks why policy lives in the doors.** Every defect on record is the rendering policy (trust gate, blank rows, footer) re-implemented client-side in Rust, Python and TS.
  - The server already serves `format=summary` and an opt-in `toon`, so a server-rendered injection format is an untested alternative that would make the doors thin and parity nearly trivial.
  - It might conflict with the frozen contract and the hooks' query and cache logic, and the pack can't settle that. The plan never asks the question.
- **It never counts users.** The "other people's machines" cost behind rejecting (d) has no evidence in the pack beyond one machine and one VPS.
- **It reads "remaining gaps, redundancy, features/operational upgrades" as "remaining register rows".** It concludes almost nothing is left in cleanup and proposes no feature or operational work. The only recurring defect it names is the N-places-with-a-check-in-N-1 shape (TODO 33), and the plan itself repeats it in the 59/59/60 counts and the `.skills_to_install/` claim it corrects in §1.1.