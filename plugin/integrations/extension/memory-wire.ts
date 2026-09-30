// memory-wire native memory extension for omp and pi.
// MEMORY_WIRE_EXTENSION_VERSION 1
//
// Installed by `memory-wire connect omp`, as
// ~/.omp/agent/extensions/memory-wire.ts   (omp loads flat `*.ts` files there),
// and by `memory-wire connect pi`, as
// ~/.pi/agent/extensions/memory-wire/index.ts   (pi auto-discovers `<dir>/index.ts`).
//
// Installed by memory-wire, managed by memory-wire, do not edit. `connect`
// overwrites this file and `connect --uninstall` deletes it.
//
// What it adds on top of the MCP entry `connect` already writes: automatic
// recall injection. An MCP tool exists only when the model chooses to call it, so
// an MCP-only host never sees a memory unless it decides to look. Returning a
// system prompt from `before_agent_start` is the one moment a harness lets a
// third party put text in front of the model unasked.
//
// Two properties are load-bearing and everything below serves them:
//
//   Never break the agent. Every request is bounded by an AbortController, and
//   every failure -- refused, unreachable, timed out, malformed -- becomes
//   `undefined` from the handler. A memory backend that stalls a turn or puts an
//   error in front of a model is worse than one that has nothing to say.
//
//   Never inject nothing. An empty recall, a down server and a recall of
//   whitespace all return `undefined`, so the harness keeps its own system
//   prompt untouched rather than gaining an empty header.
//
// Three constraints shape the style, and each one is a gate:
//
//   Zero dependencies. This file is imported straight out of a home directory
//   with no `node_modules` beside it, so an import of anything -- even a Node
//   built-in spelled with a package name -- turns the extension into something
//   that silently does not load. `fetch`, `AbortController`, `setTimeout` and
//   `process` are globals, and a global cannot be missing on a host that is
//   already running this file.
//
//   No type syntax. A `.ts` file is transpiled, not type-checked, by the loader
//   that reads it, so annotations here would be erased at load and never
//   verified by anything. The file is therefore written in the intersection of
//   JavaScript and TypeScript, which is what lets `node --check` parse it --
//   making that check a real gate on this file rather than a formality. The
//   `event` payloads are typed by JSDoc where the loader ignores it too, and
//   every field read below is guarded at runtime instead: that is what actually
//   holds, since the payload is only known from a binary that can change.
//
//   No copy of the harness's own type surface. `pi` is typed by the one method
//   this file calls, because restating a guess at the full `ExtensionAPI` would
//   be a second thing to keep in step with an API we cannot see.

/** Upper bound on entries taken from one recall. The REST recall has no `limit`
 *  field, so the cap is enforced here where it is actually in effect. */
var RECALL_ENTRIES = 5;

/** Token budget for the recall, in the server's own `budget` field. A recall
 *  that omits it falls back to the bank's `recallMaxTokens` and then to
 *  DEFAULT_RECALL_BUDGET (8_000) -- enough to return a whole long session, which
 *  is not what a system-prompt block should be. */
var RECALL_BUDGET = 1200;

/** Wall-clock bounds. A recall is on the critical path to the first token, so it
 *  gets the shorter of the two. */
var RECALL_TIMEOUT_MS = 1500;
var RETAIN_TIMEOUT_MS = 2000;

/** Hard ceiling on what this extension may add to a system prompt, so a large
 *  bank cannot grow the context without bound. */
var INJECT_CAP_CHARS = 2000;

/** Per-entry ceiling inside that block. */
var ENTRY_CAP_CHARS = 400;

/** Per-part ceiling on what is retained, so one long answer cannot store 30 KB. */
var RETAIN_PART_CAP_CHARS = 800;

/** Server base URL. Loopback and unauthenticated, which is why this is a default
 *  the environment can move rather than something this file validates. */
function endpoint() {
  return (process.env.MEMORY_WIRE_URL || "http://127.0.0.1:8888").replace(/\/+$/, "");
}

/** Bank to read and write. Not derived from the cwd: a per-project bank is a
 *  deliberate `MEMORY_WIRE_BANK`, and guessing one from a directory name moves
 *  memories between namespaces on the strength of a path. */
function bank() {
  return (process.env.MEMORY_WIRE_BANK || "memory-wire").trim();
}

/** One bounded POST. `null` for every failure, with no distinction between
 *  "the server said no" and "the server never answered".
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
  } catch (_e) {
    return null;
  } finally {
    clearTimeout(timer);
  }
}

/** Collapse to one line and cap, so a multi-paragraph memory cannot take a whole
 *  system prompt. */
function clamp(text, max) {
  var flat = text.replace(/\s+/g, " ").trim();
  return flat.length <= max ? flat : flat.slice(0, max - 1).trimEnd() + "…";
}

/** The text of a message's `content`, which is a string in some events and a
 *  block array in others -- both shapes are present in the omp 18.3.0 binary. */
function textOf(content) {
  if (typeof content === "string") return content;
  if (!Array.isArray(content)) return "";
  return content
    .map(function (block) {
      return block && block.type === "text" && typeof block.text === "string" ? block.text : "";
    })
    .filter(Boolean)
    .join("\n")
    .trim();
}

/** The newest assistant text in an `agent_end` event's `messages`, or `""`.
 *
 *  `agent_end` carries `messages: [assistantMessage, ...toolResults]` (verified
 *  in omp 18.3.0), which is the same field agentmemory's pi extension reads, so
 *  this is one accessor rather than a per-host guess. Walking backwards stops at
 *  the first assistant turn that has any text, so a trailing tool result cannot
 *  stand in for the answer. */
function lastAssistantText(messages) {
  if (!Array.isArray(messages)) return "";
  for (var i = messages.length - 1; i >= 0; i--) {
    var m = messages[i];
    if (!m || m.role !== "assistant") continue;
    var text = textOf(m.content);
    if (text) return text;
  }
  return "";
}

/** The block appended to the system prompt, or `""` when there is nothing worth
 *  saying. Empty is the signal the caller uses to inject nothing at all.
 *
 *  `items` are the server's `{id, score, content}` entries, so each carries the
 *  fused RRF score the ranking used. It is *relative* — the top hit of an
 *  irrelevant set scores well by construction, because BM25 is min-max
 *  normalised per query — and nothing branches on it here, because a cut on that
 *  number is a fitted constant and no dev set exists to choose one. The top
 *  `RECALL_ENTRIES` are injected unconditionally either way; see
 *  `docs/OPEN_HOOK_RECALL_RELEVANCE.md`. */
function formatRecall(items) {
  var lines = [];
  for (var i = 0; i < items.length && i < RECALL_ENTRIES; i++) {
    var item = items[i];
    // The default recall response is a bare array of content strings; a
    // `format: "full"` server answers with `{id, score, content}` instead. Both
    // are accepted so this file does not depend on which one it is talking to.
    var content = typeof item === "string" ? item : item && item.content;
    if (typeof content !== "string" || !content.trim()) continue;
    lines.push(lines.length + 1 + ". " + clamp(content, ENTRY_CAP_CHARS));
  }
  if (!lines.length) return "";
  var block = "## memory-wire long-term memory\n" + lines.join("\n");
  return block.length <= INJECT_CAP_CHARS ? block : block.slice(0, INJECT_CAP_CHARS - 1) + "…";
}

/** memory-wire's memory extension: recall injected before each turn, the turn
 *  retained after it.
 *  @param {{ on: (event: string, handler: Function) => void }} pi */
export default function memoryWireExtension(pi) {
  var lastPrompt = "";
  var lastRetained = "";

  pi.on("before_agent_start", async function (event) {
    var prompt = event && typeof event.prompt === "string" ? event.prompt.trim() : "";
    if (!prompt) return undefined;
    // Recorded before the await, so a recall that fails still leaves the prompt
    // available to `agent_end`.
    lastPrompt = prompt;

    var items = await post(
      "/banks/" + encodeURIComponent(bank()) + "/recall",
      // `format: "full"` is what puts `score` on the wire; the default shape is a
      // bare array of content strings. `formatRecall` reads both, so this stays
      // compatible with a server that does not know the field.
      { query: prompt, budget: RECALL_BUDGET, format: "full" },
      RECALL_TIMEOUT_MS
    );
    if (!Array.isArray(items)) return undefined;
    var block = formatRecall(items);
    if (!block) return undefined;

    // Returning `systemPrompt` REPLACES it, so the harness's own prompt has to be
    // carried through or its instructions are silently erased. A host may hand
    // it over as a string or as a list of segments; both are joined rather than
    // stringified, which would collapse a list into one comma-riddled line.
    var own = event.systemPrompt;
    if (Array.isArray(own)) own = own.join("\n\n");
    return { systemPrompt: [own, block].filter(Boolean).join("\n\n") };
  });

  pi.on("agent_end", async function (event) {
    var prompt = lastPrompt;
    // Cleared before the await: a turn that starts before this one finishes must
    // not have its prompt consumed by the earlier retain.
    lastPrompt = "";
    var answer = lastAssistantText(event && event.messages);
    if (!prompt && !answer) return undefined;

    var content = [
      prompt ? "asked: " + clamp(prompt, RETAIN_PART_CAP_CHARS) : "",
      answer ? "answered: " + clamp(answer, RETAIN_PART_CAP_CHARS) : "",
    ]
      .filter(Boolean)
      .join("\n");
    // An auto-retry re-submits the same turn, and the server dedupes identical
    // content anyway -- but not before paying for the write, so it is checked here.
    if (!content || content === lastRetained) return undefined;

    var stored = await post(
      "/banks/" + encodeURIComponent(bank()) + "/retain",
      { content: content },
      RETAIN_TIMEOUT_MS
    );
    // Only a confirmed store updates the marker, so a retain that failed is
    // retried on the next turn rather than dropped for the rest of the session.
    if (stored) lastRetained = content;
    return undefined;
  });
}
