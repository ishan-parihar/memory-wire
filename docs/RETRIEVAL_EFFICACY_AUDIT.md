# Live-bank retrieval efficacy audit — all wired harnesses

Recorded 2026-10-08. An operational audit of context injection and retrieval
across every memory-wire surface wired on this machine and the racknerd VPS,
run **against the live banks as they are actually filling up** — not against
LongMemEval, which was not consulted for any decision here (§6). No ranking
parameter, weight, or threshold was changed by this audit; every number below
was read off the running system (`memory-wire 0.6.3`, serve PID 1067) or its
SQLite files, read-only.

Companion records: `docs/OPEN_HOOK_RECALL_RELEVANCE.md` (the standing
relevance-gating diagnosis this audit does not re-litigate),
`docs/INTEGRATION_AUDIT.md` (F1–F7), `docs/CONSISTENCY.md` §31 (pointer).

## 1. Method

1. Enumerate every wired surface from the installed files and live processes
   (`ps`, harness configs, extension/plugin markers), not from `connect`
   docs — several findings below are things the docs do not know exist.
2. Read the query-composition path of each surface from source.
3. Profile each bank's corpus (SQLite, read-only): row counts, producers,
   size distribution, junk ratio.
4. Run a recall battery through the same REST route every surface uses,
   with each surface's own query shape (raw prompt, composed prompt,
   model-composed) and budget.
5. Exercise the live paths end-to-end: the `hook` binary with real payloads,
   the MCP tool from a live session, a real `opencode run` retain→recall
   cycle, and two recalls on the VPS hermes bank over SSH.

## 2. Surface inventory

| # | Surface | Live? | Bank | Recall query | Budget | Injection caps |
|---|---|---|---|---|---|---|
| 1 | OMP extension v2 (`~/.omp/agent/extensions/memory-wire.ts`) | **yes — this session** | `omp` (baked) | prompt; ≤160 chars borrows `clamp(lastAnswer, 600)` | 1200 | 5 entries × 400c, 2000c block, provenance label |
| 2 | pi extension v2 (`~/.pi/agent/extensions/memory-wire/index.ts`) | wired, no live process | `omp` (baked) | same | 1200 | same |
| 3 | opencode plugin v1 (`~/.config/opencode/plugins/memory-wire.ts`) | **yes** | `omp` (baked) | same + per-query dedup cache | 1200 | same |
| 4 | MCP stdio (OMP `mcp.json`, `opencode.json`, `cursor`) — 8 live procs | **yes** | `omp` (argv) | model-composed | 2000 default (bank config is `{}`) | **none** (~4.8 KB measured per call) |
| 5 | claude-code hooks ×5 (`~/.claude/settings.json`) | wired, no live process | `omp` (explicit `--bank`) | raw prompt from stdin | 1200 | **8 entries, no per-entry cap, no label** |
| 6 | codex hooks ×5 (`~/.codex/hooks.json`) + MCP `http://…:8888/mcp/omp` | wired, no live process | `omp` (explicit) | same | 1200 | same |
| 7 | hermes python provider (`~/.hermes/plugins/memory-wire/`, `config.yaml memory.provider: memory-wire`) | wired locally; **live on VPS** | `memory-wire` | `_strip_skill_scaffolding(user_query)` | 1200 | **top-8 uncapped, no label** |
| 8 | operant embedded provider (`operant-core/src/memory_wire.rs`, `~/.operant/memory_wire.sqlite`) | wired into `run.rs`, **store has 0 rows** | `operant` (own db) | raw last user message | 2000 | 5 hits, `format_hits` **discards the score**, uncapped content, `<memory_context>` tags |
| 9 | Unattributed `hook stop` producer | **yes** (46 markers) | `omp` | n/a — writes only | — | — |

Surface 9: something on this box executes `memory-wire hook stop --bank omp`
when opencode-format sessions end — 46 `session ses_… ended` markers since
2026-09-30 (~4–12/day), including one for this audit's own test session
15 ms after its plugin retain. Not the omp binary (0 `memory-wire` strings),
not oh-my-openagent, not AFT, not hermes scripts/agent-hooks, not operant-core
(no spawn of the hook CLI). An integration `connect` does not know about is a
governance blind spot: `connect --uninstall` would not remove it, and `doctor`'s
bank-sweep can misattribute its writes.

The dispatcher "Memory context" blocks injected into OMP seat sessions are
**not memory-wire**: they are composed by the operant gateway from
`~/.hermes/dispatch/registry*` and `~/.operant/*.db`. Two independent memory
systems ride every seat session; only one of them is memory-wire.

## 3. Bank corpus profiles (SQLite, read-only)

| Bank / store | Rows | Bytes | Avg len | Composition |
|---|---|---|---|---|
| `omp` (serve `memory.db`) | 798 | 6.44 MB | 8071c (max 34185c) | ~700 one-time Hindsight transcript imports (raw `[role: …]` session dumps, 10–34 KB each, multi-project); 33 `turn-*` v2 pairs; 46 `stop` markers; ~41 manual/contextless |
| `memory-wire` (hermes local) | 1217 | ~0.8 MB | 682c | **943 v1-era `asked:` pairs** (the pre-v2 retention that stored every turn); 265 other; 9 markers |
| `default` (serve) | 0 | — | — | dead bank |
| `operant` (`~/.operant/memory_wire.sqlite`) | 0 | — | — | schema present, never written — provider inert |
| VPS `memory-wire` (`hermes.db` on racknerd) | 1041 | — | — | 13 tags, actively written daily; contains raw tool-output flushes and real ops decisions |

Two structural facts follow. First, **88% of bank `omp`'s bytes are imported
raw transcripts from other projects**, and they are long: any query that does
not strongly match a v2 turn row competes against 34 KB documents. Second,
the hermes agent family reads the *junk-legacy* bank: locally 77% of its
namespace is v1 question-pairs, and the VPS bank mixes genuine decisions
with raw `bash`-output dumps that recall then serves as "memory".

## 4. Recall battery — bank `omp`

Each case is the exact query text a real surface would send, run through
`POST /banks/omp/recall` with `budget: 1200, format: "full"` (the extension's
own call). "inj" = characters the extension would inject (top-5 × 400c cap).
Relevance is judged against the query's evident intent; scores are the fused
RRF value (relative by design — see `OPEN_HOOK_RECALL_RELEVANCE.md`).

| # | Query (shape) | Top hits (score / len / what) | Verdict |
|---|---|---|---|
| A | verbatim README turn, 167c (raw prompt — the query this session's docs turn actually sent) | 0.0205 own `asked/answered` pair (gold) · 0.0179 technical-authority row (irr) · 0.0014 "Design Aesthetics homepage" (irr) · 0.0010 "The quick brown fox…" (test row) | 1/5 gold, inj ~1334c |
| B | verbatim audit turn, 936c (**the current user request**) | **0.0204 cortex "The System, As It Actually Is… 20 pools" transcript, 4800c — the pollution live in this very session, reproduced** | 0/1 gold, inj 400c |
| C | `"Continue"` raw (fresh-process v2, and every hook surface) | 0.0187 a 4800c Hindsight transcript "We need to proceed with the next phase…" | noise, inj 400c |
| D | `clamp(lastAnswer,600)+"\nContinue"` (v2 composition) | **0.0205 the actual work being continued (gold)** · 0.0176 memory-wire gap-investigation row (gold) · aroute (irr) · operant assignment (irr) | **2/4 gold — the mechanism works** |
| E | "release process build-release.sh musl glibc floor installer" | 0.0205 v0.6.3 release turn (gold, 2699c) · aroute release (semi) · **`<system-reminder>` turn row (junk)** · operant discipline (cross-project) | gold at #1; 2 junk in top-4, inj ~1500c |
| F | "opencode plugin recall injection system.transform" | 0.0205 gap-closure row (gold) · 0.0201 gap-investigation row (gold) · law-of-one fault-injection (cross) · quick-brown-fox (test) | **2/4 gold** |
| G | "dispatcher cron registrations routing telegram" | 0.0203 **`<system-reminder>` row, 3897c (junk)** · generic tooling discipline (cross) | 0 gold — dispatcher knowledge is not in this bank |
| H | "quantitative trading cell architecture kelly" | **0.0204 the Cell-Architecture Audit Report — the exact polluter from `OPEN_HOOK_RECALL_RELEVANCE.md`, still rank #1** | pollution persists, inj 400c |
| I | `"session ended"` | five 44c stop markers | mild marker surfacing |
| J | `"<system-reminder> background bash"` | 0.0191 unrelated Hindsight transcript | noise, inj 400c |

Same server, same query E through the **MCP tool** from this live session:
identical top-4, no caps, ~4.8 KB of tool result; bank config is `{}`, so a
caller that omits `budget` gets the 2000-token default ≈ 8 KB.

## 5. Live specimens

1. **This session is polluted, and the polluter is identified.** The
   `## memory-wire long-term memory` block at the top of the audit's own
   context carries the cortex 20-pools memory. Case B reproduces it exactly:
   a long, specific, on-topic prompt about auditing memory-wire injection
   ranks an unrelated project's architecture transcript first, because v2's
   answer-borrowing only arms for prompts ≤160 chars — long prompts are
   raw-prompt queries, and raw lexical match over a corpus where 88% of the
   bytes are other projects' transcripts loses. The provenance label is the
   only thing that marks it as unverified.
2. **Pollution with observed model damage.** The first `opencode run` test
   (model `space-bunny-free`) answered a retention-test prompt with unrelated
   Quickshell exit-code lore — content that exists in bank `omp` via `/tmp`
   session rows. The model latched onto injected recall instead of the prompt.
   The re-run with `nvidia/z-ai/glm-5.3` answered correctly, retained
   (`turn-bwoa4c`, 12:48:19.978Z), and recalled on probe. The flaky-model run
   left **no retain row** — one missed headless retain in two attempts,
   undiagnosed (GAP-10).
3. **The hook surface injects ~4.8 KB for "Continue".** Live binary runs:
   project query → 4885 bytes (top hit genuinely relevant, but a
   `<system-reminder>` row at rank 3 and a cross-project lesson at rank 4,
   no entry cap); `"Continue"` → 4848 bytes of whatever the tie picked;
   `session-start` → 6074 bytes of preamble+recall at every session open;
   blank prompt → **0 bytes** (the never-fail contract holds, verified live).
4. **VPS hermes (bank `memory-wire`, 1041 rows).** Ops query ("memory-wire
   serve systemd unit restart") → genuinely relevant cutover notes. Telegram
   decision query → raw `git push` tool output and roster dumps served as
   "memory". The bank's flushes retain tool output verbatim, and recall serves
   it back as best-of-set.
5. **Hermes local bank (the 943-junk-pair namespace).** A memory-wire project
   query returns useful rows (the `435f1f31…` findings, the original
   integration request); a telegram query returns TTL/outreach question-pairs
   — irrelevant. The namespace is mostly stale question texts, but its
   on-topic rows still rank.

## 6. Gap register

Ordered by per-turn impact on the agentic loop. Every fix direction is a
mechanism (composition, caps, labels, namespaces, retention discipline) —
none is a score threshold, per `OPEN_HOOK_RECALL_RELEVANCE.md`'s standing
conclusion that a cut on the min-max-normalised score is a fitted constant.

- **GAP-1 — Surface parity drift (highest impact).** The v2 mitigations —
  short-prompt composition, 400c/2000c caps, 5-entry limit, provenance label —
  exist only in the OMP/pi extension and opencode plugin. The claude-code and
  codex hooks (raw query, top-8, uncapped, no label) and the hermes python
  provider (top-8, uncapped, no label) predate them. Measured: 4848 bytes
  injected for `"Continue"`; a 6 KB preamble at every session start; the
  hermes plugin injects up to 8 full entries. Fix: port the extension's
  composition and caps into `hooks.rs::recall_section` (and the preamble's
  curl-examples block deserves a size audit of its own) and into the hermes
  plugin's `prefetch`.
- **GAP-2 — Harness noise retained as memory.** The extension captures the
  prompt verbatim, including `<system-reminder>` envelopes injected by the
  harness: 2 of the 8 newest `omp` rows are reminder text, and case G's
  top hit for a real question is a 3897c reminder row. These rows now
  compete in every ranking. Fix: strip harness-reminder envelopes in
  `before_agent_start` / `chat.message` capture, the same place the prompt
  is read.
- **GAP-3 — The imported-transcript corpus dominates ranking.** ~700
  Hindsight rows are 88% of bank `omp`'s bytes: multi-project raw transcripts
  10–34 KB. They win generic ties (B, C, J), the trading polluter still ranks
  #1 (H), and the test row "The quick brown fox…" surfaces at near-zero
  scores. This is a data-governance problem, not a ranker problem: the
  ranker is doing exactly what its corpus tells it. Fix options, in the
  boring order: retag the import with an `imported`/`transcript` tag and
  have auto-injection surfaces recall with a tag filter (the `tags` parameter
  already exists on the route); or move the import to its own archive bank
  (explicit, reversible, no ranker change).
- **GAP-4 — Store and namespace fragmentation.** Four banks across three
  stores locally; the hermes agent family reads the v1-junk bank `memory-wire`
  while every connect-managed harness reads `omp`; operant's embedded provider
  is wired into `run.rs` with its own sqlite and **zero rows ever written**
  — it prefetches an empty bank every turn and has no retain path, so the
  dispatcher's knowledge (case G) is invisible to every memory-wire surface.
  Fix: pick one canonical bank per agent family, and either give operant a
  retain path or unwire its provider.
- **GAP-5 — MCP is unbounded by default.** Bank config `{}` → omitted budget
  = 2000 tokens ≈ 8 KB per `memory_recall` call, 4× the extension's budget,
  with no entry caps. Fix: set `recallMaxTokens` in the bank config (one
  `PUT /banks/omp/config`), no code change.
- **GAP-6 — Hermes `sync_turn` retention discipline.** It tail-cuts at 2000
  chars (`body[-FLUSH_CHARS:]`), which drops the *question* — the retrieval
  key — whenever the answer is long; it retains every turn including bare
  acknowledgements; and it has no `document_id` dedupe (the local bank's 943
  pairs are the result). Fix: head-anchor the pair, port the extension's
  `MIN_ANSWER_CHARS` and `turn-<hash>` document_id.
- **GAP-7 — `stop` markers rank.** 46 `session ses_… ended` rows surface for
  session-ish queries (case I). Small (44c each) but pure noise; retagging
  them (they already carry `context: hook:stop`) and excluding them from
  auto-injection would be the same tag mechanism as GAP-3.
- **GAP-8 — Unattributed integration writes to the bank.** Surface 9 above.
  Find the producer before any `connect --uninstall` or doctor-driven cleanup.
- **GAP-9 — The relevance-gating question is unchanged and stays OPEN.**
  The score now survives the wire in hooks (0.6.3) and the hermes plugin, but
  `operant-core`'s `format_hits` still discards it, and no surface branches on
  it. Any gating work must start by labelling a dev set over the real bank
  (§6's three-set discipline) — this audit's case table is a 10-row start of
  exactly such a set.
- **GAP-10 — Headless retain fragility.** One missed retain in two headless
  `opencode run` attempts (flaky model). Undiagnosed single instance; needs a
  repeatable probe before headless retention is trusted in automation.

## 7. What works (measured, not assumed)

- **The v2 composition mechanism works**: case D turns `"Continue"` from a
  random transcript (C) into the actual work being continued, top-2 gold.
- **On-topic queries find gold at rank 1–2** (E, F, A) — the ranker is not
  the problem; the corpus is (GAP-3).
- **End-to-end retain→recall is live and correct** in the opencode plugin:
  real run, `turn-bwoa4c` retained at `session.idle`, recalled on probe.
- **Never-fail holds under test**: blank query → 0 bytes; every failure mode
  in the extension/plugin/hooks collapses to silence by design.
- **Bank agreement holds** on every connect-managed surface (all read `omp`);
  the v2 caps and provenance label are in effect where they are installed.

## 8. Priorities

1. GAP-1 parity (biggest per-turn pollution and context cost).
2. GAP-2 reminder stripping (small diff, immediate ranking hygiene).
3. GAP-3 corpus decision (biggest ranking win available; a data decision).
4. GAP-4 + GAP-6 (namespace consolidation, hermes retention discipline).
5. GAP-5 (one config write), GAP-7 (same tag mechanism), GAP-8 (attribution).
6. GAP-9 only after a labelled dev set exists — this audit's graded battery
   is its first 10 rows.

## 9. Reproduction

```bash
# battery (same route every surface uses)
curl -s -X POST localhost:8888/banks/omp/recall \
  -H 'content-type: application/json' \
  -d '{"query":"<verbatim prompt>","budget":1200,"format":"full"}' | jq '.[0:5]'

# hook surface, live binary, real payload shapes
echo '{"prompt":"Continue"}' | memory-wire hook prompt --bank omp | wc -c   # 4848
echo '{"prompt":"   "}'    | memory-wire hook prompt --bank omp | wc -c   # 0

# corpus profile
sqlite3 -readonly ~/.local/share/memory-wire/memory.db \
  "SELECT COUNT(*), SUM(LENGTH(content)) FROM memories WHERE bank_id='omp';"

# a real retain→recall cycle
opencode run -m ar/nvidia/z-ai/glm-5.3 "<distinctive prompt>"   # then probe the bank
```

The graded case table in §4 is committed with this document; it doubles as
the seed of the labelled dev set GAP-9 requires.
