# Retrieval efficacy plan — closing the audit gaps

Written 2026-10-08 against `docs/RETRIEVAL_EFFICACY_AUDIT.md` (the evidence
base; every GAP-N below cites it). Status: **executed 2026-10-08, shipped as
v0.7.0** — Wave 0 through Wave 3 in full, Wave 4's dev set seeded and the
retain probe passed; the operant-side items in §7 remain the only open work.
The audit's §4 case table is this plan's v0 baseline — every wave re-runs it and
records the delta in `docs/CONSISTENCY.md`.

Execution notes, recorded where the plan said less than the doing taught:

- Wave 1 grew a second envelope: OMP wraps prompts in `<system-notice>` as
  Claude-family hosts wrap them in `<system-reminder>`. All four surfaces
  strip both spellings.
- Wave 2's hooks composition reads the last assistant turn from the
  payload's `transcript_path` (the file the host already names) rather than
  any transcript format guess; an unreadable transcript is the raw prompt.
- Wave 3's exclusion is applied post-pool, one `ids_tagged` set lookup per
  recall, rather than in SQL — the include filter must stay inside the
  candidate window, but exclusion *wants* rows gone from both streams, and
  a new `Store` method with a default kept every other backend untouched.
- GAP-8 is closed by characterization (see the addendum in
  `docs/INTEGRATION_AUDIT.md`): producer unnamed, effect neutralized because
  it execs the installed binary.
- GAP-4's decision held: coding harnesses read `omp`, the hermes family reads
  `memory-wire`, no memories moved, and the junk pairs stopped growing.

## 0. Doctrine constraints — non-negotiable, inherited

- **Mechanisms only.** Composition, caps, labels, tags, retention discipline,
  config. No score thresholds anywhere — `OPEN_HOOK_RECALL_RELEVANCE.md`'s
  conclusion stands until a labelled dev set exists (Wave 4 builds it).
- **Never-fail preserved** on every touched path: a stripped transcript, a
  failed tag filter, a config read miss must all collapse to today's
  behavior, never to an error in front of a model.
- **Frozen HTTP contract**: the only wire change is one additive optional
  field (`exclude_tags`), absent-by-default so every existing client sees
  byte-identical responses.
- **Zero new dependencies**; the extension and opencode plugin stay
  zero-dependency, no-type-syntax, `node --check`-clean.
- **LongMemEval untouched.** The Wave 4 dev set is a new labelled set over
  the real banks, its own directory, never the test partition.
- **No memory moves without an explicit step.** Retagging is additive and
  reversible; the one-time migrations list their exact row sets before
  touching anything.

## 1. Wave 0 — operations, no code (all reversible, do first)

| Step | Action | Effect |
|---|---|---|
| GAP-5 | `PUT /banks/{omp,memory-wire}/config` `{"recallMaxTokens":1200}` (local serve + VPS serve) | MCP callers omitting `budget` drop from ~8 KB to ≤~4.8 KB per call. Route already exists (`src/api.rs:827`); zero code. |
| GAP-8 | `strace -f -e trace=execve -o /tmp/exec.log` around one `opencode run`, grep `memory-wire` in the log | Attributes the unaccounted `hook stop` producer (46 markers). Adopt it into `connect`'s manifest, or remove it. Before this lands: no `connect --uninstall`, no bank cleanup that assumes `connect` knows every writer. |
| GAP-4 (decision) | Namespace map goes in writing: coding harnesses = `omp` (already uniform), hermes family = `memory-wire` (VPS 1041 live rows stay; local 943 v1 pairs become a read-only archive once GAP-6 stops new junk). No automatic migration; `BANK_IDENTITY.md` reasoning applies. | One bank identity per agent family, stated, not implied. |

GAP-4's operant half is **out of this repo** — see §7.

## 2. Wave 1 — harness-noise stripping (GAP-2), every capture surface

**Change.** Strip `<system-reminder>…</system-reminder>` envelopes from the
captured prompt at both uses: before composing the recall query and before
retaining the turn. Four files, one mechanism each (the surfaces share no
code by constraint, only design):

- `plugin/integrations/extension/memory-wire.ts` — in `before_agent_start`
  (query composition) and `agent_end` (retain capture).
- `plugin/integrations/opencode-plugin/memory-wire.ts` — same two seams
  (`chat.message` capture, `system.transform` query).
- `src/hooks.rs` — at prompt extraction (`field` call sites feeding
  `prompt_at`/`session_start_at`).
- `plugin/integrations/hermes/memory-wire/__init__.py` — inside
  `_strip_skill_scaffolding`, which is already the strip seam.

**Tests.** `hooks.rs` canned-server tests (reminder-wrapped prompt → recall
body carries no reminder text; retain body carries none); embedded==disk
doc-parity tests carry the extension/plugin change automatically; hermes
offline checks gain one case.

**One-time prune.** List, then delete, the existing reminder-keyed rows in
bank `omp` (the audit found 2 in the newest 8; full sweep by content match).
Deletion is by id after a printed list — the user sees exactly what goes.

**Verify live.** A synthetic reminder-wrapped prompt through a probe session;
assert the retained row is clean. Re-run audit cases E/G: their top sets no
longer contain reminder rows.

## 3. Wave 2 — injection parity (GAP-1), retention discipline (GAP-6), marker tags (GAP-7)

**GAP-1a — `src/hooks.rs`.** Bring the hook surface to the extension's shape,
constants and all:

- `MAX_LINES` 8 → 5; per-entry clamp 400c; block cap 2000c; the provenance
  label line (`recalled by relevance from bank \`X\`, unverified`).
- **Short-prompt composition**: when the prompt is ≤160c and the payload
  carries `transcript_path`, borrow the last assistant text from the
  transcript tail (reuse `transcript_tail`, clamp 600c) into the recall
  query — the same mechanism as extension v2, fed from the file the payload
  already names. An unreadable transcript falls back to the raw prompt;
  never-fail extends to this path.
- **Preamble diet**: `session-start`'s preamble measured 6074c. Keep the
  write-path examples (they are claude-code's only write surface — it has no
  MCP entry), capped to the two essential lines; target ≤1500c total for
  preamble + recall on a session's first turn.

**GAP-1b — hermes `prefetch`.** `MAX_LINES` 8 → 5, per-entry clamp 400c,
label line. (The plugin already carries the score; it stays carried.)

**GAP-6 — hermes `sync_turn`.** Head-anchor the pair instead of tail-cutting
the whole body: user clamp 600c **always present**, answer tail to 1400c;
skip answers <32c (`MIN_ANSWER_CHARS`, ported); `document_id =
"turn-" + djb2(user)` so a repeated ask replaces its row. This stops the
943-pair failure mode at the only place still producing it.

**GAP-7.** `stop` markers retain with `tags:["marker"]` (hooks.rs and the
hermes session-end retain). Wave 3's `exclude_tags` consumes the tag.

**Tests.** Canned-server tests for: composition from transcript tail, both
caps, label, unreadable-transcript fallback, preamble size ceiling, marker
tag on the wire. Hermes offline checks for each GAP-6 rule.

**Verify live.** `echo '{"prompt":"Continue"}' | memory-wire hook prompt
--bank omp | wc -c` → ≤ ~2100 (was 4848). Project query → same shape with
the label present. Session-start → ≤1500c. Audit case C: no 34 KB
transcript at rank 1 through the hook path.

## 4. Wave 3 — corpus hygiene (GAP-3): `exclude_tags` + the retag

**Server.** Recall gains optional `exclude_tags: string[]` — candidates
carrying any listed tag are removed after the candidate pool is drawn and
before fusion, so ranking math is untouched. Absent = today's behavior,
byte-identical (pinned by a contract test). Wrong type → `422`, unknown
enum semantics unchanged. Additive field, frozen-contract-safe.

**One-time migration** (script `scripts/retag_imports.py`, listed before
run, backup via the existing store backup first):

- every `context LIKE 'imported from hindsight bank doc%'` row gains tags
  `transcript`, `imported` (~700 rows, 88% of bank `omp`'s bytes);
- the `The quick brown fox…` test rows are deleted after listing.

**Surfaces adopt.** The four auto-injection surfaces (extension, opencode
plugin, hooks, hermes prefetch) pass `exclude_tags:["transcript","marker"]`.
MCP and `reflect` stay unfiltered — an explicit search must see everything.

**Verify.** Re-run the audit battery: case B (the audit's own pollution
specimen) no longer auto-injects the cortex transcript; C's noise is gone;
H's trading report still retrievable through MCP; D unchanged (gold rows
are `turn-*`, not transcripts).

**Stated honestly:** case D proved transcript rows are sometimes the right
answer. Exclusion applies to auto-injection only, and Wave 4 measures on
the dev set whether any transcript row ever belongs in an auto-injected
block; the answer can reopen this. The retag is reversible (drop the tag).

## 5. Wave 4 — the labelled dev set, then the gating decision (GAP-9, GAP-10)

- **Build `eval/relevance/`** — its own directory, never LongMemEval: ~100
  labelled cases harvested from real `turn-*` rows (query, gold ids,
  distractor ids, expected auto-inject set) plus the audit's 10 committed
  cases as the fixed head of the set. Generator script committed; labels
  reviewed by the user. Metric: auto-inject precision@5 plus
  junk-characters-per-turn (the audit's `inj-cost`), reported per wave.
- **Then, and only then, the gating question** gets a data-backed answer:
  always-inject vs tag-filter vs any candidate gate, decided on the dev
  set, measured once for the report, never iterated against it. An
  acceptable outcome is "no gate beats tag hygiene" — that closes GAP-9
  with evidence instead of a constant.
- **GAP-10**: a 10-run headless retain probe (both models) gates the
  release. If the miss reproduces, the suspect is `lastAssistantTextFor`'s
  2s race against the SDK client — raise the bound to `RETAIN_TIMEOUT_MS`
  and retry once at idle. If it does not reproduce in 10 runs, record it as
  unreproduced and move on.

## 6. Release train

One minor bump — surface behavior changes are user-visible — **0.7.0**,
cut once, after Wave 3 verifies (Wave 4 can land before or after; it adds
no runtime behavior). Procedure is `AGENTS.md` §11 verbatim: feature
commit(s), bump commit, tag names the bump, both `ci.yml` and `release.yml`
disabled around the tag push, `--only "linux-x86_64 linux-x86_64-musl
linux-aarch64"`. Per-wave gates, all four cargo commands with
`PATH=/usr/bin:$PATH`, `node --check` on both templates, doc-parity tests,
and the live probes listed in each wave. Docs ride the same PRs (§6):
README wiring/injection paragraphs (caps now uniform across surfaces),
`INTEGRATION_AUDIT.md` addendum, this file's status, `CONSISTENCY.md`
append per wave's battery delta.

## 7. Out of this repo (separate tickets, named so they are not lost)

- **operant** (`agentic-harness/operant`): its embedded provider
  (`memory_wire.rs`, bank `operant`, 0 rows) needs either a `sync_turn`
  retain path or to be pointed at the serve and unwired from its own
  sqlite; its `format_hits` still discards the score the rest of the
  system now carries. Not memory-wire repo work.
- **The dispatcher seat-memory system** (operant registry dbs, seat
  `MEMORY.md`) is a separate memory system; this plan deliberately does
  not touch it. GAP-8's attribution may end with a documented bridge.
- **VPS hermes flushes** retaining raw tool output — the VPS bank's
  retention seam (`on_session_end`/`on_pre_compress` prose capture) may
  want the same GAP-6 discipline; measure on the VPS after Wave 2 before
  deciding.

## 8. Sequencing at a glance

| Wave | Gaps | Repos touched | Runtime effect when done |
|---|---|---|---|
| 0 | 5, 8, 4-decision | none (ops) | MCP default ≤1200 tokens; producer attributed; namespaces stated |
| 1 | 2 | 4 capture files + tests | No reminder text in queries or bank rows again |
| 2 | 1, 6, 7 | hooks.rs, hermes plugin | Every auto-inject surface: ≤2000c, ≤5 entries, labeled, composed; junk retention stopped |
| 3 | 3 | api.rs + 4 surfaces + script | Auto-injection never emits imported transcripts or markers; explicit search still sees all |
| 4 | 9, 10 | eval/relevance/ + probes | Gating decided on data; headless retain proven ≥10/10 |

End state: every harness — OMP, pi, opencode, claude-code, codex, cursor,
hermes, MCP — injects the same bounded, labeled, cleanly-composed block;
auto-injection excludes corpus noise by tag rather than by score; retention
adds only question/answer pairs worth recalling; and the one question the
audit left open is closed with a labelled set instead of a guess.
