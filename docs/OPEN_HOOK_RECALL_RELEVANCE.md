# Open: hook recall injects the best of an irrelevant set

Status: **OPEN — diagnosed, deliberately not fixed.** Recorded 2026-09-30 so it
can be picked up after the operant agentic-loop work.

## Symptom

On 2026-09-30, twice, the `UserPromptSubmit` hook injected roughly 10,000
characters of quantitative-trading cell-architecture notes into a conversation
about Telegram agentic-loop bugs. The injected rows were well-formed, correctly
attributed, and completely irrelevant:

> `# Cell-Architecture Audit Report — pre-deployment revision (2026-09-22)`
> … 263 audited cell-venues … Kelly allocation … RG-071 …

The second instance was a fragment of an unrelated agent session log:

> `Full battery, five separate stages:` … `All five stages green independently`

The cost is not only the wasted context. It is that the injection is
indistinguishable, at the point of receipt, from a genuinely relevant recall — so
it reads as *my memory system is telling me something true* rather than *my memory
system has no notion of relevant*.

## Mechanism

> **CORRECTION 2026-11-18 — the code block below is STALE. Do not file a bug
> against it.** It was written 2026-09-30. The score is **no longer discarded**:
> at `36bb947` (v0.10.0) `src/hooks.rs:660` `recall_at` returns
> `Option<Vec<ScoredHit>>`, `ScoredHit` (`:496-507`) carries `content`,
> `created_at` **and** `score: Option<f64>`, `recall_at_fmt` (`:583`) asks for
> `format` explicitly so the score is on the wire, and
> `the_score_should_survive_the_client_boundary` (`:1497`) pins it.
>
> **The open defect is narrower and still real:** `recall_section` (`:680-706`)
> reads `content` and `created_at` and **never reads `score`**. The only consumer
> of `score` is a `tracing::debug!` (`:640`) whose own comment says "Nothing
> branches on this and nothing is gated by it". So the evidence-gathering half
> of this document has **landed**; the policy half has not.
>
> Tracked as **W7** in
> `docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md`.

`src/hooks.rs:333-362`, the live path **as it was when this was written**.

```rust
fn recall_at(endpoint: &str, bank: &str, query: &str) -> Option<Vec<String>> {
    if query.trim().is_empty() { return None; }
    let body = json!({ "query": query, "budget": BUDGET }).to_string();
    …
    serde_json::from_str::<Vec<String>>(&resp.body).ok()   // ← score discarded
}

fn recall_section(bank: &str, hits: &[String]) -> String {
    let mut out = format!("\n### memory-wire recall — `{bank}`\n\n");
    for hit in hits.iter().take(MAX_LINES) {              // ← top-N, unconditionally
        out.push_str("- ");
        out.push_str(hit);
```

Three facts, each verified in the tree:

1. **The ranking is relative by design, and deliberately so.**
   `src/recall.rs:330-334` states it outright: the min-max normalised BM25 term
   means *"better BM25 than its competitors for this query"*, explicitly **not**
   *"confidently relevant"*, and it records that a hard confidence floor is
   forbidden by `AGENTS.md` §1 without a dev set. That reasoning is sound. Given
   a query with no good match, the best-of-a-bad-set is still returned — that is
   what a relative ranker does, and it is not a defect in the ranker.

2. **The score is fetched, carried, and then not used.** *(Repaired 2026-11-21.
   This item said the score was "thrown away one line later" and that `recall_at`
   deserialises to `Vec<String>`. Both were true on 2026-09-30 and neither is true
   now — the CORRECTION block above already said so, and this paragraph went on
   contradicting it eight lines below. A correction block that sits above text it
   corrects is not a correction; it is a second wrong thing.)*
   The recall response is a scored structure and the score is on the wire:
   `recall_at` (`src/hooks.rs:727`) returns `Option<Vec<ScoredHit>>`, and
   `ScoredHit` (`:516`) carries `content`, `created_at` and
   `score: Option<f64>`. `recall_section` (`:680-706`) reads `content` and
   `created_at` and **never reads `score`**; the only consumer of `score` is a
   `tracing::debug!` at `:702` whose own comment says "Nothing branches on this
   and nothing is gated by it". So the evidence-gathering half of this document
   has landed and the policy half has not.

   **On the citations, which is the other half of this repair.** This item cited
   `src/api.rs:1903` for the test `recall_should_score_with_the_fused_rrf_value`.
   That test is at **`src/api.rs:3132`**; line 1903 is a bare `}`. The citation
   was not stale — it was never right: at `36bb947`, the commit this document
   pins itself to, the same test was at **2168**. The `src/hooks.rs:660` /
   `:583` / `:1497` citations in the correction block are correct at their stated
   commit and have drifted since, which is the ordinary kind and is left alone
   because the block names its commit.

3. **The only gate is non-emptiness.** `recall_section` takes the first
   `MAX_LINES` hits unconditionally. Nothing between the server's ranking and the
   injected text asks whether the ranking was any good.

So the defect is not in the ranker and not in the caller. It is the seam inside
memory-wire: a **relative** ranking is consumed as though it were an **absolute**
one, and the scale that would distinguish the two is discarded before the
consumption.

## The same defect exists independently, twice

`operant`'s in-process provider reproduces it exactly, in the same shape, in a
different codebase:

- `crates/operant-core/src/memory_wire.rs:166-180` — `format_hits` iterates
  `&[ScoredMemory]` and emits only `hit.memory.content`. The struct is *named*
  `ScoredMemory`; the score is one field access away.
- `crates/operant-core/src/agent/run.rs:1749-1769` — the injected block is gated
  on `!is_empty()` and nothing else.

That second one is **not** the path that produced the observed injections — its
bank is `DEFAULT_BANK` and it emits `[memory]\n- …`, which does not match the
observed `### memory-wire recall — \`omp\`` header. Recording it because the same
wrong assumption was written twice independently, and a fix that lands in one
place will otherwise leave the other.

## What is NOT wrong, and must not be "fixed"

- **Do not add a score threshold to `recall`.** `AGENTS.md` §1 forbids acquiring
  one without a dev set, and the reasoning at `recall.rs:330-334` is correct. A
  hard-coded cut would be a fitted constant with no evaluation behind it — the
  exact failure mode this project has already paid for once in the fusion weights.
- **Do not treat "it returned something" as a bug in FTS5 or the BM25 arm.** Both
  are measured to work; the corpus is simply not about the query.

## Where the fix belongs

The evidence-gathering half is unambiguous and cheap, and **has landed**
(2026-11-21; it was already true when this was re-read, and this section went on
recommending it):

- ~~Stop discarding the score in `recall_at`; deserialise the scored shape that
  `api.rs` already returns, and pass it through to `recall_section`.~~ **Done:**
  `recall_at` (`src/hooks.rs:727`) returns `Option<Vec<ScoredHit>>` and the score
  is on the wire. What remains is the *last* step — `recall_section` reads
  `content` and `created_at` and never reads `score`.
- Same change in operant's `format_hits`, which already receives `ScoredMemory`.
  **Not done, and not this repository's to do.**

The remaining step — reading `score` in `recall_section` — is the same kind of
change: it makes the decision *possible* without making it, and it is testable
without a dev set (assert the score survives the client boundary, which
`the_score_should_survive_the_client_boundary` already does for the half that
landed).

The policy half is **not** decidable without one, and should not be guessed:

- What score, below which, is "not worth injecting"? BM25 magnitude is
  per-query min-max normalised (`recall.rs:319-337`), so the top hit of a bad set
  scores `1.0` by construction. A cut on that field is close to meaningless
  **unless** the raw, un-normalised BM25 is also carried across the boundary.
- Whether the right cut is a score, a count, a budget, or a per-turn cost
  ceiling is a product decision with a token-cost trade-off.
- A dev set for this means labelled (query, relevant?) pairs over the real bank.
  Until one exists, the honest options are the current always-inject, or an
  explicit opt-out the user controls.

## Related, already fixed — the mirror image

0.5.1 fixed the *opposite* failure: a hook resolving to a bank with no memories
returned a bare `[]` and `doctor` reported a store nothing was serving. That was
**silent empty recall**. This is **silent irrelevant recall**. Both present to the
user as "the memory system did not help", and the 0.5.1 fix does nothing for this
one. Worth reading them together, because a system that has been hardened against
one half of a failure class should not be surprised by the other.

## Verification when fixed

- With a bank whose contents do not match the query at all, the hook emits
  **nothing** — not an empty section, not a best-of-bad-set.
- With a bank that does match, the previously-returned content is still returned.
- The score demonstrably survives `recall_at` — assert on the parsed value, not on
  the rendered string.
- Cost: report injected characters per turn before and after, on a real
  conversation, not a benchmark. The whole point is a user-visible behaviour.

## Status update (2026-10-08): mitigated by mechanism, not by threshold

Verification work on opencode/OMP ("Continue"-style prompts returned irrelevant
recall) shipped three changes. None adds a score cut — the conclusion reached
above (a threshold is a fitted constant) still stands.

- **Session-aware recall query** (extension v2 + opencode plugin): prompts
  under 160 chars borrow the previous turn's answer into the recall query, so
  what ranks is the work being continued instead of the literal word
  "Continue". Lives in `plugin/integrations/extension/memory-wire.ts`
  (`before_agent_start`) and `plugin/integrations/opencode-plugin/memory-wire.ts`
  (`experimental.chat.system.transform`). Mechanism, not a filter: nothing is
  dropped on a score.
- **Provenance label**: the injected block now opens with
  `(recalled by relevance from bank \`<id>\`, unverified)`, so a
  best-of-irrelevant set can no longer read as a verified one.
- **Retention discipline**: turns with no answer or a bare-acknowledgement
  answer are no longer retained, and `document_id = "turn-" + hash(prompt)`
  makes a repeated ask replace its row rather than pile up identical question
  texts. Verified live: bank `omp` gained `turn-*`-keyed rows carrying real
  answers, including an `asked: continue` row with substance.

**Residual, said out loud:** the borrowed-answer signal is in-process state; a
fresh harness process (headless `opencode run`, restarted TUI) recalls with the
prompt alone on its first turn. And the scoring gating question stays OPEN by
decision, not neglect.
