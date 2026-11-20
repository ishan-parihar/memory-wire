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

> **Update 2026-11-20, after building part of it.** The `Host::ALL` half landed
> as two tests in `src/connect.rs`
> (`every_host_in_all_wires_in_a_pristine_home`,
> `every_host_in_all_unwires_what_it_wired`). It enumerates `ALL` rather than
> listing hosts, so a ninth host cannot silently escape the check — which is
> round 4's exact finding about a script that listed 6 of 8. The four-door
> fixture matrix (failed-then-recovered recall, cap overflow, `format=summary`)
> is still open. The 2–3 day estimate was for the whole thing; this half cost
> under an hour and the remaining half is the part that needs a runtime per
> host. A review round is reviewing this plan's premises; see
> `research/audits/context-pack-2026-11-20-rulings/`. The venue for that round
> is **`gemini-4-hydrogen` at default effort**, per the operator's ruling of
> 2026-11-20 -- not `xhigh`. |
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
