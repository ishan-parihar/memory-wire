// memory-wire native memory plugin for opencode.
// MEMORY_WIRE_OPENCODE_PLUGIN_VERSION 2
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

/** The block appended to the system prompt, or `""` when there is nothing
 *  worth saying. Empty is the signal the caller uses to inject nothing at all.
 *
 *  `items` are the server's `{id, score, content}` entries, so each carries
 *  the fused RRF score the ranking used. It is *relative* -- the top hit of an
 *  irrelevant set scores well by construction, because BM25 is min-max
 *  normalised per query -- and nothing branches on it here, because a cut on
 *  that number is a fitted constant and no dev set exists to choose one. The
 *  label line exists instead: it says what these rows are, so an irrelevant
 *  set cannot read as a verified one. The top `RECALL_ENTRIES` are injected
 *  unconditionally either way; see docs/OPEN_HOOK_RECALL_RELEVANCE.md. */
function formatRecall(items) {
  var lines = [];
  for (var i = 0; i < items.length && i < RECALL_ENTRIES; i++) {
    var item = items[i];
    // The default recall response is a bare array of content strings; a
    // `format: "full"` server answers with `{id, score, content}` instead.
    // Both are accepted so this file does not depend on which one it is
    // talking to.
    var content = typeof item === "string" ? item : item && item.content;
    if (typeof content !== "string" || !content.trim()) continue;
    lines.push(lines.length + 1 + ". " + clamp(content, ENTRY_CAP_CHARS));
  }
  if (!lines.length) return "";
  var block =
    "## memory-wire long-term memory\n" +
    "(recalled by relevance from bank `" + bank() + "`, unverified)\n" +
    lines.join("\n");
  return block.length <= INJECT_CAP_CHARS ? block : block.slice(0, INJECT_CAP_CHARS - 1) + "…";
}

/** The newest assistant text of the session, via the client the harness
 *  hands the plugin -- the one handle guaranteed to reach the server in both
 *  modes. `opencode serve` listens on the `serverUrl` the payload also
 *  carries, but a headless `opencode run` boots an in-process server that
 *  `serverUrl` does not name, and a plain fetch to the reported URL is
 *  refused (verified live); only the client is pointed at the real one.
 *  Bounded by a race, because the generated client takes no request signal.
 *  `""` whenever any of it is missing, refused, or slow. */
async function lastAssistantTextFor(client, sessionID) {
  if (
    !client || !client.session || typeof client.session.messages !== "function" || !sessionID
  ) {
    return "";
  }
  var data;
  try {
    var bounded = Promise.withResolvers();
    setTimeout(bounded.resolve, RETAIN_TIMEOUT_MS);
    var res = await Promise.race([
      client.session.messages({ path: { id: sessionID } }),
      bounded.promise,
    ]);
    data = res && typeof res === "object" && "data" in res ? res.data : undefined;
  } catch {
    return "";
  }
  var list = Array.isArray(data) ? data : [];
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
 *  The state lives in the return closure of the default export, the one
 *  pattern valid in both omp and pi and in opencode's `Plugin = (input) =>
 *  Hooks` contract, and therefore in every loader this file will ever see.
 *  State is per-process per-harness: prompt and answer accumulate for the
 *  life of the harness process, in whatever interleaving the harness drives.
 *  @param {{
 *    project: unknown,
 *    directory: unknown,
 *    worktree: unknown,
 *    client: unknown
 *  }} input */
export default async function memoryWireOpencodePlugin(input) {
  var client = input && typeof input === "object" && "client" in input ? input.client : undefined;
  var lastPrompt = "";
  var lastAnswer = "";
  var lastRetained = "";
  // A multi-step turn calls the transform once per step; one recall per
  // distinct query keeps step N from paying (or duplicating) step 1's recall.
  var injectedQuery = "";
  var injectedBlock = "";

  return {
    /** The user message text for the turn in flight. The hook hands over the
     *  message parts; reading them is the whole capture -- no fetch needed for
     *  what arrives in the event. */
    "chat.message": async function (_input, output) {
      var parts =
        output && typeof output === "object" && Array.isArray(output.parts) ? output.parts : [];
      var text = stripReminders(textOf(parts));
      if (text) lastPrompt = text;
    },

    /** The injection point. Appending to `output.system` is verified to reach
     *  the model as system content. */
    "experimental.chat.system.transform": async function (_input, output) {
      if (!(output && typeof output === "object" && Array.isArray(output.system))) return;
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
        if (injectedBlock) output.system.push(injectedBlock);
        return;
      }
      var items = await post(
        "/banks/" + encodeURIComponent(bank()) + "/recall",
        // `format: "full"` is what puts `score` on the wire; the default shape
        // is a bare array of content strings. `formatRecall` reads both, so
        // this stays compatible with a server that does not know the field.
        { query: query, budget: RECALL_BUDGET, format: "full", exclude_tags: EXCLUDE_TAGS },
        RECALL_TIMEOUT_MS
      );
      var block = Array.isArray(items) ? formatRecall(items) : "";
      injectedQuery = query;
      injectedBlock = block;
      if (block) output.system.push(block);
    },

    /** Turn end -- `session.idle` fires once per completed turn -- is where
     *  the retain happens, because only then does the answer exist. */
    event: async function (arg) {
      var ev = arg && typeof arg === "object" ? arg.event : undefined;
      if (!(ev && typeof ev === "object" && ev.type === "session.idle")) return;
      var props = ev.properties && typeof ev.properties === "object" ? ev.properties : {};
      var sessionID = typeof props.sessionID === "string" ? props.sessionID : "";
      var prompt = lastPrompt;
      // Cleared before the await: a turn that starts before this one finishes
      // must not have its prompt consumed by the earlier retain.
      lastPrompt = "";
      var answer = await lastAssistantTextFor(client, sessionID);
      // The next turn's short-prompt query composition borrows this answer.
      lastAnswer = answer;
      // An unanswered or bare-acknowledgement turn is a question with no
      // knowledge in it: retaining prompt-only rows is how the bank fills with
      // duplicate question texts that recall then injects as if they were
      // memory. The pair is the memory.
      if (!prompt || !answer || answer.length < MIN_ANSWER_CHARS) return;

      var content = [
        "asked: " + clamp(prompt, RETAIN_PART_CAP_CHARS),
        "answered: " + clamp(answer, RETAIN_PART_CAP_CHARS),
      ].join("\n");
      // An auto-retry re-submits the same turn; the server dedupes identical
      // content anyway -- but not before paying for the write.
      if (!content || content === lastRetained) return;

      // `document_id` keyed on the prompt makes a repeated ask replace its row
      // (the server's default update mode) rather than add a twin.
      var stored = await post(
        "/banks/" + encodeURIComponent(bank()) + "/retain",
        { content: content, document_id: "turn-" + hash(prompt) },
        RETAIN_TIMEOUT_MS
      );
      if (stored !== null) lastRetained = content;
    },
  };
}
