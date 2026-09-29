"""memory-wire memory provider for Hermes Agent.

Installed by ``memory-wire connect hermes``, which copies this directory
to ``$HERMES_HOME/plugins/memory-wire/`` and sets ``memory.provider: memory-wire``
in ``~/.hermes/config.yaml``.

Everything here is one file on purpose. Hermes loads a user-installed memory
provider as ``_hermes_user_memory.<dir-name>``, and the directory name is the
provider name the user put in ``memory.provider`` -- so the module name carries a
hyphen and is not a valid Python identifier. A sibling module would have to be
imported through a path that is awkward to express, and the failure mode is a
plugin that silently does not load. ``plugins/memory/__init__.py`` explicitly
supports sibling submodules, but hindsight and agentmemory both ship single-file
providers, and single-file is the shape this loader is known to work.

The wire protocol is memory-wire's own HTTP surface -- the same four calls
``memory-wire mcp`` makes, against the same routes ``memory-wire serve`` exposes:

    POST {endpoint}/banks/{bank}/retain   {"content", "context", "tags"}
    POST {endpoint}/banks/{bank}/recall   {"query", "budget", "tags", "format"}
    POST {endpoint}/banks/{bank}/reflect  {"query", "tags"}
    GET  {endpoint}/banks/{bank}/config

No third-party imports. ``urllib`` and ``json`` are in the standard library, and
a plugin that failed to import would leave Hermes with no memory at all.
"""

from __future__ import annotations

import json
import os
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Dict, List, Optional
from urllib.parse import urlparse

try:  # Hermes installs its own memory ABC; the fallback keeps this importable
    # under a bare interpreter for the offline checks in tests/.
    from agent.memory_provider import MemoryProvider
except ImportError:  # pragma: no cover - only reached outside Hermes
    from abc import ABC, abstractmethod

    class MemoryProvider(ABC):  # type: ignore[no-redef]
        @property
        @abstractmethod
        def name(self) -> str: ...

        @abstractmethod
        def is_available(self) -> bool: ...

        @abstractmethod
        def initialize(self, session_id: str, **kwargs: Any) -> None: ...

        @abstractmethod
        def get_tool_schemas(self) -> List[Dict[str, Any]]: ...

        @abstractmethod
        def handle_tool_call(self, name: str, args: Dict[str, Any], **kwargs: Any) -> str: ...


# Endpoint used when neither the environment nor the saved config names one.
# Same literal as `paths::DEFAULT_ENDPOINT`, and the same env var
# (`MEMORY_WIRE_URL`), so one setting moves the Rust clients and this plugin
# together. `serve`'s own default is a different port; that mismatch is
# documented, not papered over here -- if you run on it, set the URL.
DEFAULT_ENDPOINT = "http://127.0.0.1:8888"

# Bank used when nothing names one. Same literal as `paths::DEFAULT_BANK`.
DEFAULT_BANK = "memory-wire"

# Seconds for one HTTP call. Same value as `paths::IO_TIMEOUT` (2s) and it has to
# stay below the ceilings Hermes puts around us: prefetch runs on a worker with
# an 8s deadline, sync_turn on a worker with a 5s drain deadline. A memory
# server that is down must cost one timeout, not the session.
TIMEOUT = 2.0

# Token budget and line cap for a prefetch recall. Both lifted from
# `src/hooks.rs` (BUDGET = 1200, MAX_LINES = 8) so a Hermes session and a
# Claude Code session get the same amount of context for the same bank.
BUDGET = 1200
MAX_LINES = 8

# Characters of prose one retain may carry. Same literal as `FLUSH_CHARS` in
# `src/hooks.rs`, for the same reason: a seeded and a flushed conversation
# should cost the same to store.
FLUSH_CHARS = 2000

# The historian preamble, verbatim from `REFLECT_SYSTEM_PROMPT` in `src/api.rs`.
# Static by contract -- `MemoryProvider.system_prompt_block` is documented as
# "STATIC provider info (instructions, status)" -- so this costs no network call
# on the system-prompt assembly path.
HISTORIAN_PREAMBLE = (
    "You are a historian reading a memory bank's record of past decisions.\n"
    "\n"
    "Report decisions and rationale, never the current implementation.\n"
    "\n"
    "- Write declarative past-tense prose: what was decided, and on what reasoning.\n"
    "- Never issue instructions or recommendations. Do not write \"you should\",\n"
    '  "remove", "update", "switch to", "add", or any other imperative.\n'
    "- Reproduce literal tables, identifiers, and numbers verbatim. Do not\n"
    "  summarize, round, or paraphrase a recorded figure.\n"
    "- Treat every entry as history. If a decision was later superseded, say it was\n"
    "  superseded — do not restate it as what the code does now.\n"
)

_memory_write_warned = False


# ---------------------------------------------------------------------------
# HTTP
# ---------------------------------------------------------------------------


def _endpoint() -> str:
    """Server URL: ``MEMORY_WIRE_URL``, then the saved config, then the default."""
    url = os.environ.get("MEMORY_WIRE_URL", "").strip()
    if url:
        return url
    cfg = _config()
    url = str(cfg.get("url", "") or "").strip()
    return url or DEFAULT_ENDPOINT


def _bank() -> str:
    """Bank id: ``MEMORY_WIRE_BANK``, then the saved config, then the default."""
    bank = os.environ.get("MEMORY_WIRE_BANK", "").strip()
    if bank:
        return bank
    bank = str(_config().get("bank", "") or "").strip()
    return bank or DEFAULT_BANK


def _config() -> Dict[str, Any]:
    """The plugin's saved config, or ``{}``.

    Read per call rather than cached at import: ``save_config`` runs in a
    different process from the agent loop, and a cached copy would make
    ``hermes memory setup`` look like it had done nothing.
    """
    home = os.environ.get("HERMES_HOME", "").strip() or str(Path.home() / ".hermes")
    try:
        raw = (Path(home) / "memory-wire.json").read_text(encoding="utf-8")
        doc = json.loads(raw)
    except (OSError, ValueError):
        return {}
    return doc if isinstance(doc, dict) else {}


def _valid_url(base: str) -> bool:
    """A URL we can build request paths from."""
    if not base:
        return False
    try:
        parsed = urlparse(base)
        # ``.port`` raises ValueError on a non-numeric or out-of-range port.
        _ = parsed.port
    except ValueError:
        return False
    return parsed.scheme in ("http", "https") and bool(parsed.hostname)


def _request(
    base: str,
    path: str,
    body: Optional[Dict[str, Any]] = None,
    method: str = "GET",
) -> Optional[Any]:
    """One JSON call, or ``None`` on any failure whatsoever.

    Every caller treats ``None`` as "say nothing". That is deliberate and it is
    the whole error contract: a memory server that is down must never put an
    error in front of a model, and ``MemoryManager`` catches provider exceptions
    anyway -- but a caught exception still costs a traceback in the log on every
    turn, and a silent ``None`` costs nothing.
    """
    if not _valid_url(base):
        return None
    data = json.dumps(body).encode("utf-8") if body is not None else None
    headers = {"Content-Type": "application/json"} if data else {}
    req = urllib.request.Request(
        f"{base}{path}", data=data, headers=headers, method=method
    )
    try:
        with urllib.request.urlopen(req, timeout=TIMEOUT) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except (urllib.error.URLError, OSError, ValueError, TimeoutError):
        return None


def _retain(base: str, bank: str, content: str, tag: str, context: str = "") -> bool:
    """Store one memory. Returns whether the server accepted it."""
    if not content.strip():
        return False
    out = _request(
        base,
        f"/banks/{bank}/retain",
        {"content": content, "context": context, "tags": [tag]},
        method="POST",
    )
    return isinstance(out, dict)


def _recall(base: str, bank: str, query: str) -> Optional[List[str]]:
    """Memories for *query*, or ``None`` when the server is unreachable.

    ``None`` and ``[]`` mean different things on purpose: the first is "no
    answer available", the second is "the answer is that there is nothing".
    ``prefetch`` collapses them, because a model cannot act on that difference.
    """
    if not query.strip():
        return None
    out = _request(
        base,
        f"/banks/{bank}/recall",
        {"query": query, "budget": BUDGET},
        method="POST",
    )
    if not isinstance(out, list):
        return None
    return [hit for hit in out if isinstance(hit, str) and hit.strip()]


# ---------------------------------------------------------------------------
# Transcript prose
# ---------------------------------------------------------------------------


def _text_of(content: Any) -> str:
    """The text of one message's ``content``, whatever shape it arrived in.

    A Hermes turn is usually a plain string. It can also be the OpenAI block
    list -- ``[{"type": "text", "text": ...}, {"type": "tool_use", ...}]`` -- and
    the tool blocks are not prose worth storing, so they are skipped rather
    than stringified.
    """
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        parts = [
            block.get("text", "")
            for block in content
            if isinstance(block, dict) and block.get("type") == "text"
        ]
        return "\n".join(p for p in parts if isinstance(p, str))
    return ""


def _prose(messages: List[Dict[str, Any]]) -> str:
    """A conversation's own words, from a Hermes message list.

    This is the plugin's whole answer to "where is the transcript?": there isn't
    one to read. ``on_pre_compress`` and ``on_session_end`` are handed the
    message list in-process, so the conversation is already in memory and the
    words never have to survive a round trip through a file. That is strictly
    better than the file-based hooks, and it is why the 128 KiB tail window in
    `src/hooks.rs` has no equivalent here.

    The last :data:`FLUSH_CHARS` characters are kept, matching
    ``transcript_tail``: the recent turns are the ones a later session needs.
    """
    lines = []
    for message in messages or []:
        if not isinstance(message, dict):
            continue
        text = _text_of(message.get("content")).strip()
        if not text:
            continue
        role = str(message.get("role", "") or "message")
        lines.append(f"{role}: {text}")
    joined = "\n".join(lines)
    return joined[-FLUSH_CHARS:]


# ---------------------------------------------------------------------------
# Provider
# ---------------------------------------------------------------------------


class MemoryWireProvider(MemoryProvider):
    """memory-wire as a Hermes memory provider."""

    @property
    def name(self) -> str:
        return "memory-wire"

    def is_available(self) -> bool:
        """Whether this provider could serve a session. No network call.

        Hermes calls this during agent init and inside
        ``discover_memory_providers()``, which runs for *every* candidate
        plugin on the box -- a probe here would put a 2s timeout in front of
        starting the agent. So this answers the question it can answer without
        the network: is there a URL to talk to, and is it one we can build a
        path from. A server that is down still reports available, and the hooks
        then say nothing, which is the documented never-fail contract.
        """
        return _valid_url(_endpoint())

    def initialize(self, session_id: str, **kwargs: Any) -> None:
        """Bind per-session state. No network call, and no failure mode.

        Hermes passes ``hermes_home`` and ``platform`` in every case, and may
        pass ``agent_context`` (``primary`` / ``subagent`` / ``cron`` / ``flush``)
        for some. The context matters: a cron employee replaying a system prompt
        would otherwise write the system prompt into a user representation, which
        the base class warns about by name, so writes are skipped for anything
        that is not the primary agent.
        """
        self._session_id = session_id
        self._base = _endpoint()
        self._bank = _bank()
        self._home = kwargs.get("hermes_home") or _hermes_home()
        self._platform = kwargs.get("platform", "")
        self._context = kwargs.get("agent_context", "primary")
        self._identity = kwargs.get("agent_identity", "")
        self._cwd = kwargs.get("cwd", "")
        self._started = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())

    def _writable(self) -> bool:
        """Whether this session should write, per Hermes' own context rule."""
        return self._context in ("primary", "")

    # -- lifecycle -----------------------------------------------------------

    def system_prompt_block(self) -> str:
        """The historian framing, plus which bank this session is bound to.

        Static, per the base class: "This is for STATIC provider info
        (instructions, status)." ``MemoryManager.build_system_prompt`` calls
        this inline while assembling the system prompt, so a network call here
        would sit on the critical path of every session start.

        Note the base class also says prefetched recall is injected *separately*
        -- through ``prefetch`` -- so this block is instructions, not content,
        and must not try to smuggle recalled memories in through it.
        """
        if not self._base:
            self._base = _endpoint()
        return (
            f"## memory-wire — bank `{self._bank}`\n\n"
            f"{HISTORIAN_PREAMBLE}\n"
            "Memories in this bank are read with `memory_recall`, written with\n"
            "`memory_retain`, and cited with `memory_reflect`. The store is local\n"
            f"and unauthenticated; it is served at `{self._base}`.\n"
        )

    def prefetch(self, query: str, *, session_id: str = "") -> str:
        """Recall for the upcoming turn, as injected context.

        The signature is keyword-only on ``session_id`` because that is what
        ``MemoryManager._prefetch_provider`` calls
        (``provider.prefetch(query, session_id=session_id)``). A provider that
        took it positionally would be called correctly and then be wrong about
        which argument it received.

        Returns "" for a blank query, an unreachable server, an empty bank, and
        a server that errored. Hermes wraps this in its own thread with an 8s
        deadline and logs a warning if we exceed it, so one TIMEOUT is the whole
        cost of a down server.
        """
        if not _valid_url(self._base):
            return ""
        hits = _recall(self._base, self._bank, query)
        if not hits:
            return ""
        out = f"### memory-wire recall — `{self._bank}`\n\n"
        for hit in hits[:MAX_LINES]:
            out += f"- {hit}\n"
        if len(hits) > MAX_LINES:
            out += f"- …and {len(hits) - MAX_LINES} more\n"
        return out

    def sync_turn(self, user_content: str, assistant_content: str, *, session_id: str = "", messages: Optional[List[Dict[str, Any]]] = None) -> None:
        """Retain the turn that just completed.

        Runs on Hermes' background sync worker, so blocking here costs the drain
        deadline rather than the turn. The user half is stored and the assistant
        half is kept as a short trailing line, because what a later session needs
        to find a decision is the question that produced it more than the
        thousand-token answer.

        Capped at :data:`FLUSH_CHARS` and tagged, so a bank of per-turn memories
        stays rankable: recall reads a bounded candidate window, so the cost of
        a chatty session is ranking quality, not scan time.
        """
        if not self._writable():
            return
        user = (user_content or "").strip()
        if not user:
            return
        answer = (assistant_content or "").strip()
        head = f"turn {self._session_id}"
        if self._identity:
            head += f" [{self._identity}]"
        body = f"{head}\nuser: {user}"
        if answer:
            body += f"\nassistant: {answer}"
        _retain(self._base, self._bank, body[-FLUSH_CHARS:], "hook:sync-turn")

    def on_session_end(self, messages: List[Dict[str, Any]]) -> None:
        """Retain the conversation's own words as the session ends.

        The counterpart of `memory-wire hook session-end`, and it stores the same
        thing: prose, not a pointer to a file. The file-based hook has to be
        handed a path in its payload; here the conversation is already in memory,
        so the words are retained directly.

        Positional-only, one argument, because that is how
        ``MemoryManager.end_session`` calls it
        (``provider.on_session_end(messages)``).
        """
        if not self._writable():
            return
        prose = _prose(messages)
        if not prose.strip():
            return
        head = f"session-end session {self._session_id}"
        _retain(
            self._base,
            self._bank,
            f"{head}\n{prose}"[-FLUSH_CHARS:],
            "hook:session-end",
        )

    def on_pre_compress(self, messages: List[Dict[str, Any]]) -> str:
        """Retain the conversation's prose, and return what to keep of it.

        This is the contract the other provider plugins on this box get wrong.
        ``MemoryManager.on_pre_compress`` collects each provider's **return
        value** and hands it to the compressor as ``memory_context``
        (``agent/conversation_compression.py``, "The provider's on_pre_compress()
        may return a string of insights it wants surfaced inside the compression
        summary"). The `messages` list is the transcript the compressor is about
        to read anyway, so mutating it in place buys nothing and risks writing a
        synthetic turn into the very transcript being summarised.

        So: the retain is the work, and the return is a short truthful marker --
        what was stored, into which bank, under which session -- so the
        compressor's summary can preserve the fact instead of dropping it.

        Returns "" when there was nothing to store, which the base class and
        `MemoryManager` both treat as "no contribution".
        """
        if not self._writable():
            return ""
        prose = _prose(messages)
        if not prose.strip():
            return ""
        kept = f"pre-compact session {self._session_id}\n{prose}"[-FLUSH_CHARS:]
        if not _retain(self._base, self._bank, kept, "hook:pre-compact"):
            return ""
        return (
            f"[memory-wire] The {len(prose)} characters of conversation above "
            f"were retained to bank `{self._bank}` (session {self._session_id}) "
            "before this context was compressed. That record is outside this "
            "conversation; a later session can recall it with `memory_recall`."
        )

    def on_memory_write(self, action: str, target: str, content: str, metadata: Optional[Dict[str, Any]] = None) -> None:
        """Not supported. Says so, once, out loud.

        The sixth event on the competitor's manifest. Mirroring it is not
        something this build can do honestly:

        * ``action='remove'`` has no counterpart. memory-wire's HTTP surface
          deletes by memory id, and a retain of the same content is a content
          hash no-op rather than a handle we were given -- so a remove either
          cannot be expressed or would silently delete the wrong row.
        * ``target`` is ``'memory'`` or ``'user'``, which are two files
          (MEMORY.md, USER.md) in Hermes' store. memory-wire has banks, not two
          files, and inventing a MEMORY.md-to-bank mapping would be a namespace
          decision this crate has not made.
        * Retaining on ``add`` alone would be half the contract under a name
          that promises all of it.

        So the hook says nothing to the model, does not write, and prints one
        line to stderr so the gap is visible in a log rather than inferred from
        a silence. See README.md, "The sixth event".
        """
        global _memory_write_warned
        if _memory_write_warned or not content:
            return
        _memory_write_warned = True
        print(
            "memory-wire: Hermes wrote to its built-in "
            f"{target!r} store ({action}) and that write was not mirrored. "
            "memory-wire has no supported mapping from MEMORY.md/USER.md to a "
            "bank, and cannot express a remove. Use `memory_retain` instead.",
            file=sys.stderr,
        )

    def shutdown(self) -> None:
        """Nothing to drain. Every call here is synchronous and bounded."""
        return

    # -- tools ---------------------------------------------------------------

    def get_tool_schemas(self) -> List[Dict[str, Any]]:
        """The same four tools ``memory-wire mcp`` serves over stdio.

        Names, descriptions, parameter objects and annotations are copied from
        ``Server::tools()`` in ``src/mcp.rs`` rather than restated, so a Hermes
        agent and an MCP client see one surface. Hermes wraps each entry as
        ``{"type": "function", "function": schema}`` after
        ``normalize_tool_schema`` drops anything that is not a dict, so the
        ``annotations`` block rides along harmlessly and is honest about what
        each tool does.

        None of the four names is in Hermes' reserved core set -- which is
        ``memory`` (singular), not ``memory_recall`` -- so none of these is
        silently dropped at registration.
        """
        return [
            {
                "name": "memory_retain",
                "description": (
                    "Retain a memory in a bank. Content and context are redacted "
                    "server-side before the write. Creates the bank if it is new."
                ),
                "parameters": {
                    "type": "object",
                    "properties": {
                        "bank": {"type": "string", "description": "Bank id; defaults to the session's bank."},
                        "content": {"type": "string", "description": "What to remember. Required."},
                        "context": {"type": "string", "description": "Optional capture context; redacted too."},
                        "tags": {"type": "array", "items": {"type": "string"}, "description": "Tags for this memory."},
                        "document_id": {"type": "string", "description": "Re-write this document instead of adding another memory."},
                        "update_mode": {"type": "string", "enum": ["replace", "append"], "description": "Whether a document_id write overwrites or extends it. Defaults to replace."},
                    },
                    "required": ["content"],
                },
                "annotations": _annotations(False),
            },
            {
                "name": "memory_recall",
                "description": (
                    'Recall memories from a bank, most relevant first, as a JSON '
                    'array of content strings. `budget` is a hard token cap, '
                    '`tags` restricts the search to memories carrying any of '
                    'them, and `format: "full"` returns {id, score, content} '
                    "objects instead so the caller can cite what it got."
                ),
                "parameters": {
                    "type": "object",
                    "properties": {
                        "bank": {"type": "string", "description": "Bank id; defaults to the session's bank."},
                        "query": {"type": "string", "description": "Search text. Required."},
                        "budget": {"type": "integer", "description": "Token cap; falls back to the bank's recallMaxTokens, then 2000."},
                        "tags": {"type": "array", "items": {"type": "string"}, "description": "Only memories carrying any of these tags."},
                        "format": {"type": "string", "enum": ["full"], "description": "Return {id, score, content} objects instead of bare strings."},
                    },
                    "required": ["query"],
                },
                "annotations": _annotations(True),
            },
            {
                "name": "memory_reflect",
                "description": (
                    "Answer a question from a bank, citing the memory it came "
                    "from. Currently top-hit citation, not synthesis — there is "
                    "no LLM in the loop."
                ),
                "parameters": {
                    "type": "object",
                    "properties": {
                        "bank": {"type": "string", "description": "Bank id; defaults to the session's bank."},
                        "query": {"type": "string", "description": "The question. Required."},
                        "tags": {"type": "array", "items": {"type": "string"}, "description": "Only consider memories carrying any of these tags."},
                    },
                    "required": ["query"],
                },
                "annotations": _annotations(True),
            },
            {
                "name": "memory_bank_config_get",
                "description": (
                    "Read a bank's stored config object (recallMaxTokens, "
                    "retainTags, and any keys this build does not act on, served "
                    "verbatim)."
                ),
                "parameters": {
                    "type": "object",
                    "properties": {
                        "bank": {"type": "string", "description": "Bank id; defaults to the session's bank."}
                    },
                    "required": [],
                },
                "annotations": _annotations(True),
            },
        ]

    def handle_tool_call(self, tool_name: str, args: Dict[str, Any], **kwargs: Any) -> str:
        """Run one tool and return a JSON string.

        A string is mandatory, not a convenience: Hermes stores the return value
        as the tool result ``content`` in the session history, and
        Anthropic-protocol providers reject non-string content with a 400 on the
        *next* request. So a dict returned from here would break the turn after
        the one that called it.

        Bank resolution is the same as everywhere else: an explicit ``bank``
        wins, else the session's. A present-but-blank ``bank`` is refused rather
        than treated as absent, because silently writing to a different bank than
        the caller named is the failure this guards against.
        """
        args = args or {}
        bank = args.get("bank")
        if bank is None:
            bank = self._bank
        if not isinstance(bank, str) or not bank.strip():
            return json.dumps({"error": "invalid bank id"})
        bank = bank.strip()

        if tool_name == "memory_retain":
            content = args.get("content")
            if not isinstance(content, str) or not content.strip():
                return json.dumps({"error": "invalid content"})
            body: Dict[str, Any] = {"content": content}
            if isinstance(args.get("context"), str):
                body["context"] = args["context"]
            if isinstance(args.get("document_id"), str):
                body["document_id"] = args["document_id"]
            if isinstance(args.get("update_mode"), str):
                body["update_mode"] = args["update_mode"]
            tags = args.get("tags")
            if isinstance(tags, list):
                body["tags"] = [t for t in tags if isinstance(t, str)]
            out = _request(self._base, f"/banks/{bank}/retain", body, method="POST")
            if out is None:
                return json.dumps({"error": "memory-wire server unreachable"})
            return json.dumps(out)

        if tool_name == "memory_recall":
            query = args.get("query")
            if not isinstance(query, str) or not query.strip():
                return json.dumps({"error": "invalid query"})
            body = {"query": query}
            if isinstance(args.get("budget"), int):
                body["budget"] = args["budget"]
            if args.get("format") == "full":
                body["format"] = "full"
            tags = args.get("tags")
            if isinstance(tags, list):
                body["tags"] = [t for t in tags if isinstance(t, str)]
            out = _request(self._base, f"/banks/{bank}/recall", body, method="POST")
            if out is None:
                return json.dumps({"error": "memory-wire server unreachable"})
            return json.dumps(out)

        if tool_name == "memory_reflect":
            query = args.get("query")
            if not isinstance(query, str) or not query.strip():
                return json.dumps({"error": "invalid query"})
            body = {"query": query}
            tags = args.get("tags")
            if isinstance(tags, list):
                body["tags"] = [t for t in tags if isinstance(t, str)]
            out = _request(self._base, f"/banks/{bank}/reflect", body, method="POST")
            if out is None:
                return json.dumps({"error": "memory-wire server unreachable"})
            return json.dumps(out)

        if tool_name == "memory_bank_config_get":
            out = _request(self._base, f"/banks/{bank}/config")
            if out is None:
                # The route 404s a bank that was never created, which is the
                # documented rule, and a config read is not worth an error for.
                return json.dumps({})
            return json.dumps(out)

        return json.dumps({"error": f"Unknown tool: {tool_name}"})

    # -- configuration -------------------------------------------------------

    def get_config_schema(self) -> List[Dict[str, Any]]:
        """Fields for ``hermes memory setup``.

        Both carry an explicit ``env_var``, which is what lets ``save_config``
        stay a no-op for the environment half and is the arrangement the base
        class asks for when a provider uses env vars.
        """
        return [
            {
                "key": "url",
                "description": "memory-wire server URL",
                "default": DEFAULT_ENDPOINT,
                "env_var": "MEMORY_WIRE_URL",
            },
            {
                "key": "bank",
                "description": "Bank id this Hermes session reads and writes",
                "default": DEFAULT_BANK,
                "env_var": "MEMORY_WIRE_BANK",
            },
        ]

    def save_config(self, values: Dict[str, Any], hermes_home: str) -> None:
        """Write the non-secret fields to ``$HERMES_HOME/memory-wire.json``.

        Keys this plugin does not act on are preserved, so a value written by a
        newer build is not destroyed by an older one re-saving the bank.
        """
        path = Path(hermes_home) / "memory-wire.json"
        doc = _read_config_file(path)
        for key in ("url", "bank"):
            if key in values:
                doc[key] = values[key]
        path.write_text(json.dumps(doc, indent=2) + "\n", encoding="utf-8")


def _hermes_home() -> str:
    home = os.environ.get("HERMES_HOME", "").strip()
    return home or str(Path.home() / ".hermes")


def _read_config_file(path: Path) -> Dict[str, Any]:
    try:
        doc = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}
    return doc if isinstance(doc, dict) else {}


def _annotations(read_only: bool) -> Dict[str, Any]:
    """Tool annotations, matching ``fn annotations()`` in ``src/mcp.rs``.

    ``readOnlyHint`` is the only one that varies. Nothing here is destructive --
    ``memory_retain`` adds a memory and never removes or overwrites one without
    a ``document_id`` -- and none of the four is idempotent, because a repeated
    retain of *different* content is a second memory.
    """
    return {
        "readOnlyHint": read_only,
        "destructiveHint": False,
        "idempotentHint": False,
        "openWorldHint": False,
    }


def register(ctx: Any) -> None:
    """Entry point the loader calls (``plugins/memory/__init__.py``).

    ``_ProviderCollector`` captures the instance. If this ever raised, the
    loader logs at debug and falls back to scanning for a ``MemoryProvider``
    subclass -- which would find the same class -- so the failure mode is
    degraded logging, not a dead plugin.
    """
    ctx.register_memory_provider(MemoryWireProvider())
