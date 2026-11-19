# Outline plan — what is left, and what each blocker needs from you

Written 2026-11-19, after the four batches of `REMAINING_WORK_PLAN.md` closed.
Supersedes that file's §4/§5 as the live list; `TODO.md` rows 10–13, 31 stay the
implementation register.

**Two sections, deliberately.** §1 is what can proceed without you. §2 is the
part that cannot — each blocker stated as a decision with its cost, its
reversible-or-not property, and my recommendation. §3 is what the last day's
verification turned up that nobody had recorded.

---

## 1. Unblocked: what I would do next without asking

None of these needs a decision. All are inside this repo and inside the law
(AGENTS §6, §8).

### 1.1 PersonaMem v2 — score the run that already finished

**This is not a "run the benchmark" item. The run completed on 2026-10-17 and the
number has never been computed.** Verified just now:

| | |
|---|---|
| run | `eval/omnimevaleval/memory-wire-mw_pmv2_dated/`, 2026-10-17 11:52 IST, `TOPK=20` |
| completeness | 4,954 answers, 4,954 judged; search 4,989 success / 11 empty / 0 failed |
| answers | carry `is_correct` — **no judge re-run is needed** |
| score | **37.8% overall (1,872 / 4,954)** |

By category: `ask_to_forget` 25.1% (n 1,039), `sensitive_info` 30.2%, `neutral_preferences`
42.1%, `anti_stereotypical_pref` 37.6%, `therapy_background` 43.1%,
`health_and_medical_conditions` 46.2%, `stereotypical_pref` 48.2%.

Why it matters: DECISIONS §6 names this exact run as the preference-memory corpus
"directly informs answerer-side work", and `TODO.md` row 18 says "PersonaMem v2
remains named-not-scheduled". It is neither run-pending nor scheduled — it is
**run and unread**, which is the same shape as the four stale register rows.

**One complication, and it needs a ruling from you rather than a decision.**
The directory is `.gitignore`d, deliberately: the run wrote memory rows verbatim
and one held a pasted bot token. So the raw outputs cannot be committed as-is,
and AGENTS §14 says live-bank output is not committed. Options: (a) record the
aggregate only, with the command and the artifact path named as uncommitted, (b)
score it and commit the score without the raw rows, (c) leave it entirely.

### 1.2 Two more corrections the last pass exposed

- **`../_audit/` and `../_audit_scratch/` are both gone.** The pinned OmniMemEval
  clone (commit `a5c8b4b`) no longer exists anywhere on this box. H10/H12's
  artifacts are committed and reproducible in principle, but **re-running either
  would mean re-cloning and re-pinning**. Anyone planning a second HaluMem run
  (DECISIONS §2) is planning a re-clone, not a re-run.
- **`RND_PLAN.md` §6.0 still describes W1–W3 as "upcoming"** and its header says
  "the upcoming cycle, outlined" — all three are done (`eval/BEAM.md`, §47; the
  battery; H12 §46). Same stale-register shape.

### 1.3 The README rubric failure is 14 months old and structural

Not caused by this session and not fixable in a batch about plugin doors:
`docs-check` reports `length_in_band` + `visual_proof`. README is 1,091 lines
against a `generic`-archetype band of **100–170**. It was already failing at
v0.10.0 (1,077 lines), v0.9.0 (1,060) and v0.8.0 (995) — the same two issues.
Fixing it means either cutting ~900 lines or declaring the archetype honestly
(`curated-list` fits at 400–2,700). Flagging it because a gate that has cried
wolf for four releases is a gate nobody reads.

---

## 2. Blocked: the decisions that are yours

Each row: what it costs, what it buys, whether it is reversible, and what I
recommend. **Nothing below moves without you.**

### 2.1 Door parity (TODO row 31) — a design ruling, not a bug

**The actual asymmetry today**, verified against the route table and tool list:

| capability | HTTP | MCP | CLI | hook |
|---|---|---|---|---|
| redact | no route | no tool | yes | no |
| hygiene | `GET /banks/:id/hygiene` | no | no | no |
| supersede | no route | `memory_supersede` | no | no |
| config get | yes | `memory_bank_config_get` | yes | yes (probe) |

**Why it is not simply "add the missing doors."** `memory_redact` on MCP is
contested on its merits, not on effort: redaction is a *custody* operation over a
store, and exposing it to any agent that can reach the MCP port means any agent
that can reach the port can rewrite history. Today the same limitation applies to
HTTP `DELETE /banks/:id/memories/:mid`, which already exists — so this is a
consistency question, not a new capability. The honest options:

- **(a) Do nothing; document the asymmetry as deliberate.** Cost: one doc pass.
  Buys: nothing changes, and a reader stops assuming parity. Reversible.
- **(b) Add `memory_redact` to MCP.** Cost: a tool plus tests plus a doc entry.
  Buys: an agent that retained a secret through MCP can retract it through MCP.
  Reversible. **This is the one I'd pick** — the capability gap is real and the
  risk profile already exists on HTTP.
- **(c) Add hygiene to CLI/MCP, supersede to HTTP.** Cost: two more surfaces.
  Buys: completeness that nothing has asked for. Reversible. Lower value than (b).

**Recommendation: (a) + (b).** Document the ruling, close the widest gap.

### 2.2 aroute (TODO row 13) — not my repo, and the fix is yours

Four defects, all in `agentic-utility/proxy/artificial-route`, all handed off in
`docs/AROUTE_BUGS.md` (202 lines, with a verification recipe at the end):

| | defect | effect on us |
|---|---|---|
| B1 | upstream failure → HTTP 200 with `{"error"}` body, logged `outcome=ok`, no failover | every OpenAI client misreads a failure as an answer |
| B2 | cline replies forwarded inside `{"success","data"}` envelope | clients must unwrap; the shim does |
| B3 | cline token/cost headers read 0 | spend limits silently miss cline |
| B4 | permanent 403 gets `cooldown_ms=0` and is retried every request | wasted calls on a dead provider |

**What it costs to leave it:** `eval/llm_shim.py` exists solely to paper over
B1/B2. Every LLM-judged run routes through it. Fixing aroute deletes the shim and
speeds up all future judged runs.

**What it costs to fix:** their repo, their service, their call — AGENTS is
explicit that memory-wire agents do not edit aroute or restart its service.

**What you'd need to do:** hand `docs/AROUTE_BUGS.md` to whoever owns aroute, then
re-run one short shim job; the doc names the exact log fields that prove each fix
(`attempts > 1` = B1, `enveloped: true` = B2, zero `tokens_in` = B3).

**Recommendation: treat as a separate piece of work, not part of this plan.**
Blocking on it costs nothing today; it only gates *speed* of future runs.

### 2.3 agent-memory-bench (TODO row 10) — needs a model budget, and it is the highest-value one

**What it is:** GiulioDER's agent-memory-bench, the only external benchmark that
grades what this product actually does — hooks and MCP wired end-to-end, task
success, plus a harm suite (superseded / contradictory / adjacent).

**Why it is different from everything else in the battery:** every other number
we publish measures retrieval on a corpus we injected. This one measures whether
an agent using memory-wire through its real interface gets the task right. It is
the answer to "does any of this help a user".

**What it costs:** an adapter (`adapters/memory_wire/` in their repo, wiring
`memory-wire connect claude-code`), plus **agent-model tokens** — a real spend, on
your account, against a frontier model. No free tier for the agent arm.

**What it buys:** the product question, and a harm-suite read that no other
instrument provides.

**Reversible?** The measurement is repeatable; the spend is not recoverable, but
it is bounded by the number of tasks you choose to run.

**Recommendation: fund it, but only after §2.1(b).** DECISIONS §4 already says
"schedule after recommendation 1 lands so it measures the improved product, not
the old one" — and recommendation 1 (dates in the surfaces, capsules) shipped in
`0b518de` and `1f93bf0`. So the precondition is now met: the product it would
measure is the improved one.

**What I need from you:** the budget ceiling, and which agent model. Without a
number I cannot even scope the adapter.

### 2.4 LLM-judged runs (TODO row 11) — needs budget and touches the test set

Four corpora: AMB `rag` on BEAM/PersonaMem/LifeBench, OAMB LME-60,
memorybench ConvoMem, MemBase RealMem/MobileMem.

**The split that matters, and it is a law question rather than a preference:**

| run | role | consequence |
|---|---|---|
| AMB `rag` on BEAM / PersonaMem / LifeBench | measure-only | may report, never select |
| **OAMB LME-60** | **test consultation** | burns a LongMemEval consultation |
| MemBase / memorybench arms that embed LME | test consultation | same |

**The consultation counter is at 5.** AGENTS §6 makes the 6th release-acceptance
only, and DECISIONS §5 says release only when a ratified decision ships. So an
OAMB run now would consume the release-acceptance slot before there is a release
to accept. That is the real cost, and it is invisible in the row text.

**What it buys:** OAMB's answer-visible-token axis is where a budget-trimmed
recall should show up — the one axis none of our instruments measure. The others
are new corpora (MemBase, memorybench) with no other evidence.

**Recommendation: fund AMB `rag` + MemBase/memorybench (measure-only, no
consultation); hold OAMB until a release is actually being cut.** That keeps the
consultation budget intact for its stated purpose.

### 2.5 TODO row 12 — turn-granularity writes: measured, and I recommend closing it

The row is still `open`, but the decision it asks for has already been made by
data:

| arm | R@1 | R@5 | `recall_all@10` | n |
|---|---|---|---|---|
| whole sessions, `overlap 0.25` (shipped) | **60.4%** | 86.0% | 94.2% | 1,531 |
| turns pooled (`locomo --turns`) | 54.6% | 83.0% | 92.2% | 1,531 |
| windows W=2 | 59.3% | — | 82.6% | — |

Turn-level loses on every aggregate; windows lose on `recall_all@10` badly enough
to breach the pool ceiling. The row's own text says it "still buys measurability
of the long corpora at a ranking cost, and is not a ranking lever."

**Recommendation: strike it as rejected, keeping the row as the record.** That is
AGENTS §8's "rejected work stays rejected" applied to an unstruck row. No new
decision needed from you unless you want it kept open for the long-corpus
measurability reason.

### 2.6 Release (DECISIONS §5) — the one with a deadline shape

**42 commits since `v0.10.0`**, including four product-visible changes:
the trust gate reaching two doors that lacked it (`98b1a36`), recall failures
becoming tellable apart (`81a2212`), dates on all four doors (`0b518de`), the
capsule knob on the plugin surfaces (`1f93bf0`).

DECISIONS §5 said: "a release becomes worth cutting when you ratify recommendation
1 (dates in surfaces / capsule flag): that is the first product-visible change of
the next cycle, and it deserves its own version." Recommendation 1 has now
shipped twice over.

**Cost of a release:** AGENTS §11 — version bump first, one unlocked `cargo
check` to refresh `Cargo.lock`, then the `--locked` gates; two commits; a tag; the
three-Linux asset set via `scripts/build-release.sh`, which refuses a dirty tree
and bills no Actions minutes. CI must be disabled around the tag push
(`ci.yml` **and** `release.yml` both trigger on `v*`).

**Cost of not releasing:** the improved product is not what anyone installs. The
dates and capsules only exist in a tree.

**Recommendation: cut one release after §2.1(b), before §2.3.** It is the
cheapest way to make the last two batches real for users, and the aroute work
does not gate it.

---

## 3. Order, once you have ruled

```
now, unblocked
  1.1  score PersonaMem v2 (needs your ruling on the gitignored raw rows)
  1.2  re-clone note + RND_PLAN §6 header correction
  1.3  README archetype: cut to band, or declare curated-list

needs you
  2.1  door parity: (a) document + (b) redact on MCP        <- my pick
  2.5  strike TODO 12 as rejected                            <- cheap, do it
  2.6  one release (bump, tag, 3-Linux assets)              <- after 2.1
  2.4  AMB rag + MemBase (measure-only); hold OAMB
  2.3  agent-memory-bench (needs the budget number)
  2.2  aroute handoff (their repo)
```

**What I am not proposing, and why:** any new ranking lever (CLOSED.md is
complete; the six-repo audit found no incoming mechanism), any LongMemEval
consultation beyond release acceptance, any threshold (CONSISTENCY §32/§48,
closed twice), any change to the shipped `overlap 0.25` (two dev corpora in
tension, DECISIONS §3 — no action is the correct action).

---

## 4. What I could not determine

Stated rather than papered over, per AGENTS §12:

- **Whether the PersonaMem v2 number is comparable to anything.** H10 established
  that no other system has run this harness with this answerer, so 37.8% is a
  standalone reading. A second system running the same harness is the only thing
  that makes it comparable — DECISIONS §2's head-to-head, on the dated adapter.
- **Whether the gitignored pmv2 raw rows contain anything else sensitive.** I
  checked the run's status files and the one known pasted-token case in the
  `.gitignore` comment; I did not scan all 4,954 rows, and I am not going to
  without being asked.
- **What the README looked like when the archetype was chosen.** The rubric has
  failed since at least v0.8.0, but I could not find the commit that made the
  README grow past the band, so I cannot say whether the archetype is wrong or
  the document outgrew it.