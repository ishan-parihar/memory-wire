# Relevance dev set — labelled cases over the real banks

**Not LongMemEval.** This directory is the labelled development set the
retrieval-efficacy audit (`docs/RETRIEVAL_EFFICACY_AUDIT.md` GAP-9) requires
before any relevance-gating decision can be made honestly. LongMemEval-S
remains the test partition, consulted for release acceptance only
(`AGENTS.md` §6). Nothing in this directory may tune against LongMemEval,
and nothing measured here is a claim about it.

## What is in here

- `cases.jsonl` — the labelled cases. The seed 10 (A–J, hand-reviewed against
  the bank as of 2026-10-08): verbatim queries with `gold` (content prefixes
  that must rank) and `junk` (content prefixes that must not be
  auto-injected). Plus 11 harvested cases (K01–K11, 2026-10-16) taken from
  the frozen snapshot's turn rows by the harvest recipe below — the question
  is the query, the row is the gold — with the provenance in each case's
  `label` field (agent-authored, human review pending).
- `freeze_battery.py` — freezes the corpus the battery measures. `dump`
  snapshots the live bank read-only (sqlite `mode=ro`; the HTTP list route
  omits `tags`) into `../data/battery/omp-snapshot.jsonl` (gitignored: live
  session content, never committed; the sha256 in the battery artifact is the
  committed record). `seed` replays a snapshot through the product's own
  HTTP retain route into any scratch serve; byte-identical rows collapse by
  content-hash dedup, the write path's own semantics, so a pinned file always
  yields the same bank.
- `run_battery.py` — runs every case through the same REST route the
  surfaces use, in both shapes that matter:
  - `auto` — the auto-injection shape (`exclude_tags: ["transcript",
    "marker"]`, budget 1200): what a turn in flight would receive.
  - `search` — the explicit-search shape (no exclusions, budget 2000):
  what a model calling `memory_recall` sees.

  It reports per case: gold@5, junk@5, and the injected-character cost,
  plus totals, plus the H8 capsule gate (ranking identity, served load,
  fidelity).

## How to use it

Against the live bank (drifts — fine for a quick look, never for a gating
number):

```bash
python3 eval/relevance/run_battery.py --bank omp
```

The **frozen battery** — the reproducible instrument a gating decision cites
(`CONSISTENCY.md` §48 has its first committed run):

```bash
python3 eval/relevance/freeze_battery.py dump    # once per corpus pin
./target/release/memory-wire serve --db /var/tmp/battery.db --addr 127.0.0.1:8897 &
python3 eval/relevance/freeze_battery.py seed --endpoint http://127.0.0.1:8897
python3 eval/relevance/run_battery.py --bank omp --endpoint http://127.0.0.1:8897
```

Same snapshot file ⇒ same bank ⇒ same marks, on any machine, at any later
date. A gating comparison names the snapshot sha256 it used.

The battery re-runs after every retrieval-affecting change and the numbers
go in `docs/CONSISTENCY.md`. The 2026-10-08 baseline (pre-fix) is recorded
in the audit doc §4; the first post-fix run is recorded in
`docs/CONSISTENCY.md` §32.

**Artifacts are committed, not paraphrased.** Every run whose numbers are
quoted in prose ships its full output here as a `BATTERY-<date>[-<tag>].txt`
file *in the same commit* as the prose (AGENTS §7: the artifact wins). The
v0.6.3 baseline has no such file — its output was not saved at measurement
time, so §32's baseline column is prose-only by miss; the first committed
artifact is `BATTERY-2026-10-08-v0.7.0.txt`.

## The discipline

- A change is **decided** on this set, then measured once on LongMemEval
  for the report — never the other way round.
- Labels are ground truth by human review. A case whose label proves wrong
  gets its label fixed in the same commit that records the measurement.
- Growing the set: harvest new cases from real `turn-*` rows (the question
  is the query, the row itself is gold, unrelated turns are distractors).
  10 cases is a seed; ~100 is the size at which a gating decision becomes
  evidence rather than anecdote.
