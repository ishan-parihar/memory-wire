// memory-wire native memory plugin for opencode.
// MEMORY_WIRE_OPENCODE_PLUGIN_VERSION 3
//
// Installed by `memory-wire connect opencode`, as
// ~/.config/opencode/plugins/memory-wire.ts -- opencode loads flat *.ts files
// out of that directory with no config entry. The mechanism was probed live
// against opencode 1.18.34 (2026-10-07): the flat file loaded, and a string
// pushed onto the system array inside `experimental.chat.system.transform`
// reached the model verbatim.
//
// Installed by memory-wire, managed by memory-wire, do not edit. `connect`
// overwrites this file and `connect --uninstall` deletes it.
//
// What it adds on top of the MCP entry `connect` already writes: automatic
// recall injection, and turn-end retention. An MCP tool exists only when the
// model decides to call it, and on a continuation prompt it does not -- a
// forked-session test showed the model reasoning "ignore memory-wire recall...
// we should continue actual work" and making zero recall calls across the
// turn. `experimental.chat.system.transform` is the one moment opencode lets a
// plugin put text in front of the model unasked.
//
// The properties are the ones the omp/pi extension
// (plugin/integrations/extension/memory-wire.ts) carries, applied to a
// different hook surface:
//
//   Never break the agent. Every request is bounded by an AbortController, and
//   every failure -- refused, unreachable, timed out, malformed -- becomes a
//   no-op. A memory backend that stalls a turn or puts an error in front of
//   the model is worse than one that has nothing to say.
//
//   Never inject nothing. An empty recall, a down server and a recall of
//   whitespace push nothing onto the system array.
//
//   Zero dependencies, no type syntax, runtime guards on every payload field.
//   The file is imported straight out of a home directory with no node_modules
//   beside it, the opencode payloads are known only by version, and a field
//   the harness stops sending must cost a no-op rather than a crash.

/** Upper bound on entries taken from one recall. The REST recall has no
 *  `limit` field, so the cap is enforced here where it is actually in
 *  effect. */
var RECALL_ENTRIES = 5;

/** Token budget for the recall, in the server's own `budget` field. A recall
 *  that omits it falls back to the bank's `recallMaxTokens` and then to
 *  DEFAULT_RECALL_BUDGET (8_000) -- enough to return a whole long session, not
 *  what a system-prompt block should be. */
var RECALL_BUDGET = 1200;

/** Wall-clock bounds. Recall rides every generation request, so it gets the
 *  shorter of the two. */
var RECALL_TIMEOUT_MS = 1500;
var RETAIN_TIMEOUT_MS = 2000;

/** Hard ceiling on what this plugin may add to a system prompt, so a large
 *  bank cannot grow the context without bound. */
var INJECT_CAP_CHARS = 2000;

/** Per-entry ceiling inside that block. */
var ENTRY_CAP_CHARS = 400;

/** Per-part ceiling on what is retained. Thirty KB would be a transcript, not
 *  a memory; but 800 chars kept only the closing summary of a working turn and
 *  lost the working state -- paths, measurements, decisions -- that made the
 *  answer worth keeping. */
var RETAIN_PART_CAP_CHARS = 4000;

/** Prompts at most this long cannot rank anything on their own: "Continue"
 *  matches every memory equally and the top of that tie is noise. */
var SHORT_PROMPT_CHARS = 160;

/** How much of the previous turn's answer a short prompt may borrow for its
 *  recall query. */
var QUERY_CONTEXT_CHARS = 600;

/** An answer shorter than this is an "OK", not a turn worth remembering. */
var MIN_ANSWER_CHARS = 32;

/** Tags whose rows are never auto-injected: imported raw transcripts and bare
 *  session markers are corpus, not context for a turn in flight
 *  (docs/RETRIEVAL_EFFICACY_AUDIT.md GAP-3/7). An explicit memory_recall still
 *  sees them — exclusion is for injection only. The field is optional on the
 *  server too, so a plugin newer than its server degrades to today's behavior
 *  rather than erroring. */
var EXCLUDE_TAGS = ["transcript", "marker"];

/** Server base URL. Loopback and unauthenticated, which is why this is a
 *  default the environment can move rather than something this file
 *  validates. */
function endpoint() {
  return (process.env.MEMORY_WIRE_URL || "http://127.0.0.1:8888").replace(/\/+$/, "");
}

/** Bank to read and write. `connect --bank` bakes the install-time bank into
 *  `DEFAULT_BANK`, so this plugin and the MCP entry the same install wrote
 *  cannot disagree on the namespace -- the split that had injected recall
 *  reading one bank while the model's tools read another. A deliberate
 *  `MEMORY_WIRE_BANK` still overrides. Not derived from the cwd: guessing one
 *  from a directory name moves memories between namespaces on the strength of
 *  a path. */
var DEFAULT_BANK = "memory-wire";
function bank() {
  return (process.env.MEMORY_WIRE_BANK || DEFAULT_BANK).trim();
}

/** One bounded POST to the memory-wire server. `null` for every failure, with
 *  no distinction between "the server said no" and "the server never
 *  answered".
 *  @returns {Promise<unknown>} */
async function post(path, body, timeoutMs) {
  var abort = new AbortController();
  var timer = setTimeout(function () {
    abort.abort();
  }, timeoutMs);
  try {
    var res = await fetch(endpoint() + path, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
      signal: abort.signal,
    });
    return res.ok ? await res.json() : null;
  } catch {
    return null;
  } finally {
    clearTimeout(timer);
  }
}

/** Collapse to one line and cap, so a multi-paragraph memory cannot take a
 *  whole system prompt. */
function clamp(text, max) {
  var flat = text.replace(/\s+/g, " ").trim();
  return flat.length <= max ? flat : flat.slice(0, max - 1).trimEnd() + "…";
}

/** A stable, non-cryptographic digest of the prompt: the retain key that makes
 *  a repeated ask replace its row instead of piling up twin questions. djb2
 *  over UTF-16 units, base-36; a collision costs one overwritten memory. */
function hash(text) {
  var h = 5381;
  for (var i = 0; i < text.length; i++) h = ((h << 5) + h + text.charCodeAt(i)) >>> 0;
  return h.toString(36);
}

/** Drop <system-notice>/<system-reminder> envelopes the harness wrapped
 *  around the prompt. The envelope is the host talking to the model, not the
 *  user's words: retained as prompt text those rows outrank real questions,
 *  and in a query they are lexical noise (docs/RETRIEVAL_EFFICACY_AUDIT.md
 *  GAP-2). A prompt that is only an envelope strips to empty, which reads as
 *  no prompt. */
function stripReminders(text) {
  return text.replace(/<system-(?:reminder|notice)>[\s\S]*?<\/system-(?:reminder|notice)>/g, "").trim();
}

/** The text of a part array: only text parts carry any. */
function textOf(parts) {
  if (!Array.isArray(parts)) return "";
  return parts
    .map(function (block) {
      return block && block.type === "text" && typeof block.text === "string" ? block.text : "";
    })
    .filter(Boolean)
    .join("\n")
    .trim();
}

/** The tier a mount that ingests web content stamps on what it stores. Rust's
 *  `Trust::UntrustedExternal` and hermes' `UNTRUSTED_TIER` are the same tier
 *  under two spellings, and the injection gate has to read all of them the same
 *  way: the weakest door decides what an untrusted mount's content can reach.
 *
 *  Withholding is on **evidence, never absence** -- a row with no `trust` field
 *  on the wire is kept, because a server old enough not to send one is not
 *  evidence of anything. */
var UNTRUSTED_TIER = "untrusted_external";

/** A row a mount may not read back as instructions. Absent means keep. */
function isUntrusted(item) {
  if (!item || typeof item !== "object") return false;
  return item.trust === UNTRUSTED_TIER;
}

/** The block appended to the system prompt, or `""` when there is nothing
 *  worth saying. Empty is the signal the caller uses to inject nothing at all.
 *
 *  `items` are the server's `{id, score, content}` entries, so each carries
 *  the fused RRF score the ranking used. It is *relative* -- the top hit of an
 *  irrelevant set scores well by construction, because BM25 is min-max
 *  normalised per query -- and nothing branches on it here, and nothing will: a
 *  score threshold on the injection path was measured and rejected twice
 *  (CONSISTENCY §32/§48) on determinism grounds. The reason is not a shortage
 *  of labelled data -- AGENTS §6 designates `eval/data/locomo/` as the dev set,
 *  ~1,530 queries. A cut would be fitted on one corpus and would move when the
 *  ranking moved. The label line exists instead: it says what these rows are,
 *  so an irrelevant set cannot read as a verified one. The top
 *  `RECALL_ENTRIES` are injected unconditionally either way; see
 *  docs/OPEN_HOOK_RECALL_RELEVANCE.md. */
function formatRecall(items) {
  var lines = [];
  for (var i = 0; i < items.length; i++) {
    var item = items[i];
    // Filtered **before** the take, and the take is `lines.length` below —
    // not `i`. Looping on `i < RECALL_ENTRIES` would bound the *input* scan,
    // so five untrusted rows ranked above one trusted row would consume the
    // whole budget and the trusted row would never be reached. That is a
    // starvation bug, not a cap: `src/hooks.rs` and the hermes plugin both
    // filter first and take second, and this file has to agree with them.
    if (isUntrusted(item)) continue;
    // The default recall response is a bare array of content strings; a
    // `format: "full"` server answers with `{id, score, content}` instead.
    // Both are accepted so this file does not depend on which one it is
    // talking to.
    var content = typeof item === "string" ? item : item && item.content;
    if (typeof content !== "string" || !content.trim()) continue;
    if (lines.length >= RECALL_ENTRIES) break;
    // The occurrence time, when the wire carried one. `src/hooks.rs` prefixes
    // it for the same reason H12 measured: a reader resolving "what did we
    // decide last week" is blind without it. It renders only when the field is
    // there, so a server older than it degrades to the exact pre-2026-11-19
    // rendering, byte for byte. Day precision -- that is what an occurrence
    // time is for.
    var stamp = (item && item.created_at) || "";
    var datePrefix = stamp.length >= 10 ? "[" + stamp.slice(0, 10) + "] " : "";
    lines.push(lines.length + 1 + ". " + datePrefix + clamp(content, ENTRY_CAP_CHARS));
  }
  if (!lines.length) return "";
  var block =
    "## memory-wire long-term memory\n" +
    "(recalled by relevance from bank `" + bank() + "`, unverified)\n" +
    lines.join("\n");
  return block.length <= INJECT_CAP_CHARS ? block : block.slice(0, INJECT_CAP_CHARS - 1) + "…";
}

/** The newest assistant text of the session, via `ctx.session.context`.
 *
 *  In v2 the context *is* the client (`docs/build/plugins/server`), so there
 *  is no separate `client.session.messages` to reach for -- the v1 code called
 *  that and it no longer exists. `context()` takes `{ sessionID }` and returns
 *  the session's messages, verified live against 2.3.2 (it schema-validates
 *  the id before doing any work).
 *
 *  `""` whenever any of it is missing, refused, or slow: a retain that cannot
 *  read the answer is skipped, and the turn is simply not remembered. Nothing
 *  here may throw into the agent.
 *
 *  @param {{ context?: Function }} session the v2 session domain */
async function lastAssistantTextFor(session, sessionID) {
  if (!session || typeof session.context !== "function" || !sessionID) {
    return "";
  }
  var list;
  try {
    var bounded = Promise.withResolvers();
    var timer = setTimeout(bounded.resolve, RETAIN_TIMEOUT_MS);
    try {
      var res = await Promise.race([session.context({ sessionID: sessionID }), bounded.promise]);
      // The v2 client answers `{ data }`; a bare array is accepted too, so a
      // shape change costs nothing.
      list = Array.isArray(res) ? res : res && typeof res === "object" ? res.data : undefined;
    } finally {
      clearTimeout(timer);
    }
  } catch {
    return "";
  }
  if (!Array.isArray(list)) return "";
  for (var i = list.length - 1; i >= 0; i--) {
    var entry = list[i];
    var info = entry && typeof entry === "object" ? entry.info : undefined;
    if (!info || info.role !== "assistant") continue;
    var text = textOf(entry.parts);
    if (text) return text;
  }
  return "";
}

/** memory-wire's memory plugin: recall injected before each turn, the turn
 *  retained after it.
 *
 *  **OpenCode v2, not v1.** Until 2026-11-20 this file exported the v1 shape --
 *  `export default async (input) => ({...hooks})` -- and opencode 2.x refuses it
 *  at load with `PluginModule.LoadError: Plugin must export a default
 *  definition with an id and an effect or setup function`. The door was
 *  therefore *silently dead* on every 2.x install: the file is copied to disk,
 *  `connect` reports `wired`, and nothing runs it. The plugin is not listed by
 *  `opencode plugin list`, and its memory injection never happens.
 *
 *  The v2 contract (`opencode.ai/docs/build/plugins/migrate-v1`): a default
 *  object with `id` and `setup(ctx)`, hooks registered through `ctx` rather
 *  than returned as a string map. Local plugins must live in a **directory**,
 *  so the install path is `plugins/memory-wire/index.ts` -- a bare file now
 *  only loads if configured, and `connect` writes no such entry (see
 *  `src/connect_opencode.rs`).
 *
 *  The three hook ports that matter:
 *    `chat.message`                                    -> `session.hook("prompt")`
 *    `experimental.chat.system.transform`              -> `session.hook("context")`
 *    `event` + `session.idle`                          -> `ctx.event.subscribe()`
 *  In v2 the callbacks take **one mutable event**, not `(input, output)`, and
 *  `ctx.session` replaces the old `client.session`. Everything below the host
 *  boundary -- the caps, `post`, `formatRecall`, the trust gate -- is the same
 *  code as the extension door, unchanged.
 *
 *  The `id` is not cosmetic: plugin storage is scoped by it and diagnostics
 *  name it, so it has to stay stable across versions or a user's stored
 *  plugin state stops being reached.
 *  @param {{
 *    session: { hook: Function, messages: Function },
 *    event: { subscribe: Function },
 *    location: object,
 *    options: object,
 *    storage: object
 *  }} ctx the v2 plugin context */
export default { id: "memory-wire", setup: setup };

function setup(ctx) {
  var session;
  var event;
  var subscribe;
  // v2 groups these on the context; v1 put them on a `client` argument. Guarded
  // rather than destructured, because a plugin that throws inside `setup`
  // fails to load rather than failing to recall -- the door would be dead
  // again, which is the bug this port exists to undo.
  session = ctx && typeof ctx === "object" ? ctx.session : undefined;
  event = ctx && typeof ctx === "object" ? ctx.event : undefined;
  subscribe = event && typeof event.subscribe === "function" ? event.subscribe : undefined;
  var lastPrompt = "";
  var lastAnswer = "";
  var lastRetained = "";
  // A multi-step turn calls the hook once per step; one recall per distinct
  // query keeps step N from paying (or duplicating) step 1's recall.
  var injectedQuery = "";
  var injectedBlock = "";

  if (session && typeof session.hook === "function") {
    // The user message text for the turn in flight. The hook hands over the
    // event; reading its parts is the whole capture -- no fetch needed for
    // what arrives in the event.
    session.hook("prompt", function (ev) {
      var parts =
        ev && typeof ev === "object" && Array.isArray(ev.parts) ? ev.parts : [];
      var text = stripReminders(textOf(parts));
      if (text) lastPrompt = text;
    });

    // The injection point. Pushing onto `ev.system` reaches the model as
    // system content.
    session.hook("context", async function (ev) {
      if (!(ev && typeof ev === "object" && Array.isArray(ev.system))) return;
      var prompt = lastPrompt;
      if (!prompt) return;

      // A prompt this short cannot rank anything on its own: "Continue"
      // matches every memory equally and the top of that tie is noise. The
      // query borrows the previous turn's answer, so what ranks is the work
      // being continued, with the prompt still steering inside it. A
      // mechanism, not a filter -- nothing is cut on a score
      // (docs/OPEN_HOOK_RECALL_RELEVANCE.md).
      var query = prompt;
      if (prompt.length <= SHORT_PROMPT_CHARS && lastAnswer) {
        query = clamp(lastAnswer, QUERY_CONTEXT_CHARS) + "\n" + prompt;
      }

      if (query === injectedQuery) {
        if (injectedBlock) ev.system.push(injectedBlock);
        return;
      }
      var items = await post(
        "/banks/" + encodeURIComponent(bank()) + "/recall",
        // `format: "full"` is what puts `score` on the wire; the default shape
        // is a bare array of content strings. `formatRecall` reads both, so
        // this stays compatible with a server that does not know the field.
        // `MEMORY_WIRE_FORMAT=summary` asks for H8 capsules instead of whole
        // memories. It comes from the environment rather than the bank's
        // `injectionFormat` config key because this file already reads
        // `MEMORY_WIRE_URL` and `MEMORY_WIRE_BANK` from there, so it costs no
        // extra request; reading the config key would add one per turn, which
        // `src/hooks.rs` documents as already costing one. The Rust hook reads
        // both and the bank wins there. Anything but "summary" reads as default.
        {
          query: query,
          budget: RECALL_BUDGET,
          format:
            (process.env.MEMORY_WIRE_FORMAT || "").trim().toLowerCase() === "summary"
              ? "summary"
              : "full",
          exclude_tags: EXCLUDE_TAGS,
        },
        RECALL_TIMEOUT_MS
      );
      var block = Array.isArray(items) ? formatRecall(items) : "";
      // Only cache a recall that SUCCEEDED. A failed `post` yields a
      // non-array, so `block` is "" -- and caching that under `query` meant
      // the next turn with the same prompt hit the `query === injectedQuery`
      // short-circuit and injected nothing, forever, until the user happened
      // to type something different. A transient outage froze recall for that
      // query. (Found by the round-5 review of this file, 2026-11-20, which
      // read the failure path this rewrite had never exercised: the rewrite
      // verified that the door LOADS and that injection works, not what
      // happens when the server is down.)
      if (Array.isArray(items)) {
        injectedQuery = query;
        injectedBlock = block;
      } else {
        // Drop the cache rather than keeping it. Clearing `injectedQuery` means
        // the next turn with the same prompt re-asks, so a recovered server is
        // picked up on the very next turn instead of after the user retypes.
        injectedQuery = "";
        injectedBlock = "";
      }
      if (block) ev.system.push(block);
    });
  }

  if (subscribe) {
    var controller = new AbortController();
    void (async () => {
      try {
        for await (var ev of subscribe({ signal: controller.signal })) {
          // Turn end. `session.idle` still exists in 2.3.2 and still carries
          // `{sessionID}`, but it does NOT fire under `opencode run` --
          // measured, over a real turn, 31 other event types delivered and
          // idle not among them. `session.execution.succeeded` does, and it
          // is the narrower claim anyway: the execution completed rather
          // than the UI going quiet. Verified live as
          // `{"type":"session.execution.succeeded", ...}` with the
          // `sessionID` on the event, not in `properties`.
          if (!(ev && typeof ev === "object" && ev.type === "session.execution.succeeded")) {
            continue;
          }
          var sessionID = typeof ev.sessionID === "string" ? ev.sessionID : "";
          var prompt = lastPrompt;
          // Cleared before the await: a turn that starts before this one
          // finishes must not have its prompt consumed by the earlier
          // retain.
          lastPrompt = "";
          var answer = await lastAssistantTextFor(session, sessionID);
          // The next turn's short-prompt query composition borrows this
          // answer.
          lastAnswer = answer;
          // An unanswered or bare-acknowledgement turn is a question with no
          // knowledge in it: retaining prompt-only rows is how the bank fills
          // with duplicate question texts that recall then injects as if they
          // were memory. The pair is the memory.
          if (!prompt || !answer || answer.length < MIN_ANSWER_CHARS) continue;

          var content = [
            "asked: " + clamp(prompt, RETAIN_PART_CAP_CHARS),
            "answered: " + clamp(answer, RETAIN_PART_CAP_CHARS),
          ].join("\n");
          // An auto-retry re-submits the same turn; the server dedupes
          // identical content anyway -- but not before paying for the write.
          if (!content || content === lastRetained) continue;

          // `document_id` keyed on the prompt makes a repeated ask replace
          // its row (the server's default update mode) rather than add a
          // twin.
          var stored = await post(
            "/banks/" + encodeURIComponent(bank()) + "/retain",
            { content: content, document_id: "turn-" + hash(prompt) },
            RETAIN_TIMEOUT_MS
          );
          if (stored !== null) lastRetained = content;
        }
      } catch (_e) {
        // The connection closing is the harness going away, not a memory
        // fault. Never break the agent: nothing here propagates.
      }
    })();
  }

  // v2 replaces the v1 `dispose()` hook with whatever `setup` returns. Only
  // what this plugin owns -- the event subscription -- needs undoing; the two
  // session hooks are scoped and unloaded with the plugin.
  return function cleanup() {
    if (controller) controller.abort();
  };
}
