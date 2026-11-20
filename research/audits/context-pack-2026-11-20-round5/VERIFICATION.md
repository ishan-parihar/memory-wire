# Verification pass — round 5 (2026-11-20)

Mandatory pre-adoption pass. A reviewer's finding is a **hypothesis about the
artefacts**, never a verdict. Each row is verified against the tree at `b8ef4c4`
by reading the cited code or recomputing the arithmetic, not by re-reading the
review.

Verdicts are kept strictly separate: **their finding, verified** / **their
finding, refuted** / **my finding, prompted by theirs** / **unknown**.

## 0. Does the reply audit my pack?

Yes, and unusually precisely. It cites `# SECTION` and `# FILE:` anchors that
exist in `PACK-AS-SENT.md`, names 22,209 chars of its own reasoning, and quotes
the subject's own artefacts. It also **audits the pack's arithmetic** rather than
paraphrasing it — which is what the prompt asked for and what makes the round
worth its tokens.

Its headline is **adopted with a correction**: reading (B), that verification is
the bottleneck, is right, and I had framed the choice as (A) vs (B) when the
pack's own evidence favours (B). Its most valuable output is not the ranking but
the specific code findings underneath it, two of which are real defects I
introduced or missed.

---

## 1. Pack-arithmetic findings — all three verified, all mine

### 1a. "D6 says 7 modules and lists 8" — **verified, my defect**

`B-DELTA.md:129` reads `**7 modules, 6,718 lines…**`. The table above it lists
eight rows (connect, connect_plugin, connect_codex, connect_ext,
connect_opencode, connect_claude, connect_copilot, connect_common) and the
arithmetic is over all eight:

```
code=3229 test=3489 total=6718  (8 modules)
```

**Verified.** "7" is a slip; the sums are right. Fixed in `B-DELTA.md`.

### 1b. "D2 says six byte-identical helpers and tabulates five" — **verified, my defect**

`B-DELTA.md` prose: "six were byte-identical bodies". The table lists five:
`mcp_url`, `baked`, `carries_marker`, `is_ours`, `preserve_comment`. The sixth
is `version`, which I described in the prose but never gave a row — and which
`connect_common` **does not contain**, because its two copies differ by a comment
and I judged the comment-bearing copy not worth extracting.

**Verified, and the reviewer is right that this reads as an inconsistency.**
Fixed: the prose now says five extracted and names `version` as the sixth
byte-identical pair deliberately left in place.

### 1c. "D7's +5 KB delta is wrong; it is +24,808 B" — **verified, my defect, and it makes AGENTS §1 stale**

```
11,166,048 − 11,141,240 = 24,808 B = 24.2 KiB   (not +5 KB)
11,166,048 / 1,048,576  = 10.6488 MiB           (rounds to 10.65, not 10.64)
```

**Verified.** I wrote "+5 KB" from an uncomputed impression. The reviewer's
arithmetic is right, and the consequence is the part I would have missed:
`AGENTS.md` §1 states **10.63 MiB (11,141,240 B, measured 2026-11-19)**, which
is now stale at HEAD. §7 of AGENTS says the artifact wins over the prose, so the
prose is the defect.

**Adopted.** `AGENTS.md` §1 corrected to the measured figure.

---

## 2. The code findings — the valuable part

### 2a. `injectedQuery` caches an empty block across a failed recall — **verified, a real defect I introduced in B0**

`plugin/integrations/opencode-plugin/memory-wire.ts:391-394`:

```js
var block = Array.isArray(items) ? formatRecall(items) : "";
injectedQuery = query;
injectedBlock = block;
```

When `post(...)` fails, `items` is not an array, so `block` is `""` — and then
`injectedQuery = query` **still executes**. The next turn with the same query
hits `:365` `if (query === injectedQuery)`, pushes nothing, and returns. A
transient outage therefore freezes empty recall for that query until the user
sends a *different* one. The reset at `:420` (`lastPrompt = ""`) clears
`lastPrompt` but **not** `injectedQuery`.

**Verified by reading the code.** This is in the file I rewrote in B0
(`3376b5c`), which I verified for *load* and for *injection* but never for
failure. The reviewer's framing is exact: I tested the happy path and the
starvation fix, and missed the caching path.

**Adopted.** Real defect, real line.

### 2b. Rust and hermes print "…and N more" after an early `left`-break — **verified**

`src/hooks.rs:779-781` and `hermes/__init__.py:573-574` both do:

```rust
if hits.len() > MAX_LINES {
    out.push_str(&format!("- …and {} more\n", hits.len() - MAX_LINES));
}
```

`hits.len()` is the **input** count. The loop can exit early at `:772`
(`if entry.chars().count() > left { break; }`), in which case the number omitted
is `hits.len() − (number actually emitted)`, which is **larger** than
`hits.len() − MAX_LINES`. The footer therefore **understates** what was dropped
whenever the character cap, not the line cap, stopped the loop.

**Verified by reading the code.** Both languages share the bug because I ported
the Rust shape to hermes. **Adopted.**

### 2c. TS skips blank content; Rust and hermes count it against the cap — **verified**

- `extension/memory-wire.ts:241` `if (typeof content !== "string" || !content.trim()) continue;`
  and `opencode-plugin` the same at `:213` — blank rows are dropped **before**
  the take.
- `src/hooks.rs:759-771` and `hermes/__init__.py:556-571` iterate
  `hits.iter().take(MAX_LINES)` / `hits[:MAX_LINES]` with no blank check, so a
  blank row consumes a slot and renders as `- \n`.

**Verified as a difference in the code, then narrowed by one more check the
reviewer could not make from the pack.** Same policy, three implementations, two
behaviours — but a blank row **cannot be written through this build's API**:

- `src/api.rs:749` `if content.trim().is_empty() { return Err(ApiError::InvalidContent) }`
  rejects blank content on every retain path;
- the schema is `content TEXT NOT NULL` with **no `CHECK`**, so a blank row is
  *schema-legal* — an older build, a raw `sqlite3` insert, or a future writer
  could produce one.

So the verdict is **verified as a latent divergence, not a live defect**: the
blank-check asymmetry only bites a row this build cannot create. It is a real
inconsistency in a policy that is supposed to be identical across four doors, and
the right fix is the cheap one — make the Rust/Python paths skip blank content
too, matching the TS doors — but it is **not** the same class as 2a/2b and I
have not treated it as urgent.

**Filed, not fixed in this commit.** The reason is the one AGENTS §8 teaches: a
change made because a review mentioned it, without a case that reproduces, is a
change made on assertion. A `CHECK` constraint or a test that inserts a blank row
through SQL would make it reproducible, and then it is a one-line fix.

### 2d. `injectionFormat` bank config is read by Rust and ignored by the TS plugins — **verified**

`src/hooks.rs` reads the bank config key; `extension/memory-wire.ts:290-303` and
`opencode-plugin:375-388` read only `MEMORY_WIRE_FORMAT`, with a comment stating
this is deliberate (Batch 4, `1f93bf0`) because the env costs no request and the
config would add one per turn.

**Verified, and it is a *deliberate* divergence with a recorded reason.** The
reviewer's finding is correct as a fact and the trade-off is documented. I record
it as **verified-but-decided**, not as a defect to fix: the alternative costs one
HTTP round-trip per turn on the injection path. Filed as a known asymmetry, not
work.

### 2e. `if (stored !== null)` (opencode) vs `if (stored)` (extension) — **verified**

`:443` and `:351` respectively. `if (stored)` is falsy for `0`, `""`, and
`false`; the retain path returns an id string or null, so the practical difference
is nil today — but the two forms are not the same predicate and one is stricter.

**Verified as a difference; judged cosmetic today.** Recorded, not fixed.

---

## 3. The reading (A) vs (B) — **their finding, adopted as (B), with a correction to my framing**

The reviewer decides **(B): verification is the bottleneck**, and the causal
chain it assembles is the strongest thing in the reply:

- `trust_gate_test.sh` existed but ran nowhere until `131cffb`;
- it then missed the post-slice starvation bug (`D0`) and the export-shape bug
  (`4646e7a` added the check reactively);
- `golden_install_shapes` (TODO row 4) verified **self-authored bytes** and
  therefore could not see that opencode 2.x never loads a flat file (`D1`);
- `memory_redact`'s first test used a string its own redactor eats (`D3`);
- the result: every recent high-severity defect came from a check that ran
  nowhere or verified the wrong thing.

**Adopted.** And the correction it forces on me: I framed C-ASK as (A) vs (B)
when the pack's evidence already favoured (B), and reading (A)'s own instruments
(H4 at 92.0/94.0/94.0; the 21-case battery at gold 16/17, junk 0/9) show the
context-drowning failure does not reproduce. My framing was the weaker one.

### 3a. Its falsifier for (B) — **adopted as the test to beat**

> "If all 4 auto-injection doors and all declared host layouts passed automated
> runtime hook+retain smoke tests with zero silent-dead-door failures across a
> release cycle, and running agent-memory-bench or an expanded multi-turn battery
> showed measurable task-success drops (>5%) caused by unconditional recall
> injection, then (A) would become #1."

That is a real falsifier with a number in it. Recorded.

---

## 4. The `connect_*` question — **their finding, verified, and it answers round 4's open question**

It argues the **enumerated parity check** beats a shared abstraction, on the
measurement I supplied: 51.9% of those lines are inline tests that "validate exact
host config bytes and cannot be deduplicated without obscuring golden
expectations", leaving 1,793 production lines across six adapters — and a Rust
trait "cannot see host-side load failures like D1".

**Verified as reasoning from the pack's numbers**, and it converges with round
4's independent conclusion ("an enumerated parity check, not a shared
abstraction"). Two rounds reaching the same instrument from different evidence is
the strongest signal in this whole record.

Its refinement is the part I did not have: the check should be **sourced from
`Host::ALL`** in `src/connect.rs` rather than a literal list — which is precisely
the defect I fixed in my own `measure_surface.sh` this session after round 4
flagged it. **Adopted.**

---

## 5. The one thing it asked for that the pack lacks — **verified as a real gap**

> "The agent-memory-bench Claude Code evaluation results (TODO.md row 10, status:
> open) — live task-success and harm-suite measurements with hooks + MCP actually
> wired together… zero live end-to-end measurements of whether unconditional
> per-turn hook injection plus the 6-tool MCP surface helps or harms real
> multi-turn agent tasks."

**Verified against TODO row 10: open**, and the reviewer is right that this is
the load-bearing absence. Every ranking above it is inference from static
retrieval benchmarks and unit checks. **Adopted as the honest limit of this
round.**

---

## 6. Verdict summary

| # | finding | verdict |
|---|---|---|
| 1a | D6 "7 modules" over 8 listed | **verified — my defect**, fixed |
| 1b | D2 "six helpers" over five tabulated | **verified — my defect**, fixed |
| 1c | "+5 KB" is +24,808 B; AGENTS §1 stale | **verified — my defect**, fixed |
| 2a | `injectedQuery` caches empty block on failed recall | **verified — real defect**, adopt |
| 2b | "…and N more" understates after early break (Rust + hermes) | **verified**, adopt |
| 2c | TS skips blank rows; Rust/hermes count them | **verified as latent, narrowed**: `api.rs:749` rejects blank content, no `CHECK` in schema. Filed, not fixed |
| 2d | `injectionFormat` read by Rust, not by TS | **verified but deliberate**, record only |
| 2e | `if (stored !== null)` vs `if (stored)` | **verified, cosmetic**, record only |
| 3 | reading (B) over (A) | **adopted**, with my framing corrected |
| 3a | falsifier for (B) | **adopted** |
| 4 | enumerated parity check > shared trait | **verified**, converges with round 4 |
| 5 | agent-memory-bench missing | **verified**, adopted as the limit |

**Net effect:** three documentation/arithmetic defects of mine to fix, **two real
code defects to fix** (2a, 2b), one latent divergence filed with the reason it is
not urgent (2c), one reading adopted that corrects my own framing, and one honest
statement of what this round cannot settle.

**Nothing in this reply was refuted.** That is unusual across five rounds and is
itself evidence that the pack was specific enough to be checkable.
