# DECISIONS — recommendations for the §6.4 gates (2026-10-17)

Advice to the user, one recommendation per open decision, ranked by what the
cycle's measurements say actually buys memory performance. Every number cited
lives in a committed artifact (§44–§48) or the pre-registrations in
`RND_PLAN.md` §6. The ranking principle: **serving shape beat every ranking
lever we measured** (H12's date prefix moved HaluMem +13.0pp overall; the
best ranking delta this cycle was §47's +3.5pp, and the overlap weight split
across corpora), and the answerer holds the error mass (on LME, 73.0% of
served-and-gold contexts are answered wrong by the small reader). Retrieval
ordering is near its ceiling on our instruments; the remaining performance
lives in what the reader receives and who the reader is.

---

## Ranked recommendations

### 1. Adopt the serving-shape wins — dates in the surfaces, capsules per-bank
*(the only measured double-digit levers left; product work, free)*

- **Serve occurrence dates in the surfaces' injection path** (auto-injection
  and `memory_recall` for agents), the way `answer_quality` already does and
  the way H12's adapter did. H12's whole +13.0pp came from this one shape
  change; a surface-level equivalent is a small patch. The corpus's temporal
  questions are unanswerable-by-construction without it — and real agent work
  ("what did I decide last week?") is temporal in exactly the same way.
- **Adopt capsule injection behind a per-bank flag** (`format: "summary"`),
  opt-in, then enable on banks showing injection noise. Frozen-battery
  evidence: 41–43% less served text, fidelity held, ranking byte-identical
  (`BATTERY-2026-10-16-frozen.txt`). This is a cost win (same ranking,
  less context) — it makes memory cheaper at equal quality, which for a
  context-limited reader is itself a quality win.
- *Why first:* both are measured, both are cheap, neither touches ranking
  (nothing can regress), and together they attack the two things the reader
  actually experiences: what shape the memories arrive in, and how much of
  the window they eat.

### 2. Run the second-system HaluMem head-to-head on the dated harness
*(turns 0.7433 into a claim; ~7 h free-tier + their setup)*

Until a second system runs this exact harness, 0.7433 is a number about us,
not a position in the field. Run MemOS or Hindsight's memory on the same
corpus, same answerer, same judge, dates served. If we lead, the headline is
earned; if we trail on a type (Dynamic Update especially), that's the next
real work item — this is the cheapest instrument that can *redirect* the
whole effort. Worth the wall clock; free in dollars.

### 3. Keep `overlap 0.25` — and close the weight question
*(no action is the correct action)*

LoCoMo says +1.4pp for 0.25; BEAM says +3.5pp for 1.00. Two dev corpora,
opposite signs, neither dominant. Changing the weight in either direction is
a new dev-set selection with no evidence net gain — and the shipped value's
`provisional:` marker can now honestly read "split across corpora, retained
by default." The stream itself is settled (§47: not deleteable — it carries
pool coverage at scale). Re-open condition: a third dev corpus at pool scale
(PersonaMem v2 could serve, once its gold resolves).

### 4. Fund Track 2: agent-memory-bench
*(the only external benchmark grading the product's real job)*

Hooks + MCP wired end-to-end, task success, a harm suite — everything else we
run grades retrieval on injected corpora. This is the only measurement of
the path agents actually use. Worth the agent-model budget; schedule after
recommendation 1 lands so it measures the improved product, not the old one.

### 5. Commit the cycle now; release only when a ratified decision ships
*(the tree is one coherent unit; nothing in it changes the product)*

W1/W2/H12 produced records, harnesses, and tooling — no shipped default
changed, no product surface moved. Commit as one unit. A release becomes
worth cutting when you ratify recommendation 1 (dates in surfaces / capsule
flag): that is the first product-visible change of the next cycle, and it
deserves its own version.

### 6. Measurement triangulation — run when the tier is idle
*(free, fills the instrument shelf)*

- **PersonaMem v2 first run**: the preference-memory corpus — a different
  error mass than anything we measure; directly informs answerer-side work.
- **OmniMemEval LoCoMo judged cross-check**: a second harness on the same
  84.1% AMB number; cheap triangulation.
- Both measure-only, both counted per §6 discipline if they touch LME arms.

### 7. Explicitly declined, on evidence

| declined | why |
|---|---|
| Dense-arm promotion | +1.8pp best case at −9.9pp R@1 (§41), 3× artifact + model download — measured twice, still no |
| Changing `overlap` now | recommendation 3 — no dominant direction |
| A standing paid reader/judge | the free tier answered every question this cycle; pay once, later, for a single **confirmation run** of the post-recommendation-1 numbers (establishes whether the residual error is reader-limited or memory-limited — high information value, but only after the free wins land) |
| aroute work in-session | their repo, their service; the recipe is handed off in `docs/AROUTE_BUGS.md` — fixing it deletes the shim and speeds every future judged run, so it's high leverage for you, not for me to touch |

## What this adds up to

If you ratify 1–5, the next cycle is: **ship the serving shape** (dates +
capsules, one release), **prove the position** (head-to-head + Track 2),
**keep the ranking core frozen** (it survived every audit and every re-test),
and measure the result on the triangulation instruments. That is the
evidence-backed path to the best judged-accuracy this architecture can reach;
the ranking core is not the bottleneck, and we now have the numbers that say
so.
