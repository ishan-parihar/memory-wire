"""memory-wire over its HTTP API — the shipped Rust binary, not the library.

The server is a separate process (`memory-wire serve --db <path> --addr <host:port>`);
this provider is a thin client over `/banks/:bank/{retain,recall}`. It is started
externally and addressed with $MEMORY_WIRE_URL, the same shape the
`hindsight-http` provider uses, so the binary under test is the one that ships.

Id round-trip: AMB scores retrieval by comparing the ids a provider returns
against `Query.gold_ids`, and memory-wire serves a memory's own uuid, not the
`document_id` the caller supplied. `ingest` therefore keeps the retain response
id alongside the document it came from, and `retrieve` maps hits back through
that table. Without it every returned id is a uuid, every gold id is a document
id, and recall scores exactly 0.0 for a perfect ranking.

What the reader sees (`rag` mode). LoCoMo's, LongMemEval's and LifeBench's
`build_rag_prompt` replace the formatted documents with `json.dumps(raw_response)`
whenever a provider returns one, because Hindsight's raw recall JSON carries the
memories. Ours would carry none of them, so `retrieve` returns `None` and the reader
gets the documents. (Until 2026-10-13 it returned a small metadata dict, and the
reader saw `{"provider": "memory-wire", "returned": 10, "k": 10}` instead of any
memory. `retrieval` mode wraps the documents itself, so PrecisionMemBench was never
affected.)

Occurrence time. `Document.timestamp` is sent as retain's `created_at` and read
back from `format: full` recall, so a session's date round-trips through the
product, as it does through Hindsight's API. (Until 2026-10-14 the HTTP surface had
no such field and this adapter held the dates client-side; TODO.md row 15.) Each hit
is prefixed with `[<context> — <created_at>]` for the reader. `context` is AMB's
session label and is still held client-side: `full` recall deliberately does not
serve capture context. The index is untouched, so ranking is exactly memory-wire's.
"""
import os

import httpx

from ..models import Document
from .base import MemoryProvider


class MemoryWireProvider(MemoryProvider):
    name = "memory-wire"
    description = "memory-wire Rust binary over HTTP: SQLite FTS5 BM25 + token-overlap RRF, one bank per user."
    kind = "local"

    # The server is one process over one SQLite store; the harness's own default
    # is 4 and recall is a bounded read, so there is nothing to raise it to.
    concurrency = 4

    # memory-wire supports tag filtering on recall (HTTP `tags` = include-any,
    # `exclude_tags` = drop-any), so the "any" scope groups AMB's filter shape
    # carries are honoured in the store (see retrieve). The `narrow_any`
    # clauses — identity tags resolved against the store's tag vocabulary by
    # trigram similarity — are Hindsight-native machinery this store does not
    # have; they are not honoured, and PrecisionMemBench's misspelled-name cases
    # will over-return accordingly (recorded with the run, not hidden).
    supports_filters = True

    def __init__(self) -> None:
        self._base = os.environ.get("MEMORY_WIRE_URL", "http://127.0.0.1:8899").rstrip("/")
        self._timeout = float(os.environ.get("MEMORY_WIRE_TIMEOUT", "60"))
        # memory id (uuid) -> AMB document id, for the conversation's bank.
        self._doc_by_memory: dict[str, str] = {}
        # AMB document id -> its session label, for the reader's header (see above).
        self._context_by_doc: dict[str, str] = {}
        # AMB document ids that were retained with a timestamp (see retrieve).
        self._dated: set[str] = set()
        # AMB document id -> the tags it was retained with, lowercased. The
        # store owns the truth; this mirror exists only so `retrieve` can
        # reason about what a filter matches while the HTTP response (which the
        # frozen contract keeps tag-free) cannot say it.
        self._tags_by_doc: dict[str, set[str]] = {}

    def _bank(self, user_id: str | None) -> str:
        # LoComo's isolation_unit is the conversation, so user_id is a real
        # per-conversation namespace here. Default mirrors the server's seeded
        # bank so a query with no user_id still has somewhere to read.
        return (user_id or "default").strip() or "default"

    def _client(self) -> httpx.Client:
        return httpx.Client(base_url=self._base, timeout=self._timeout)

    def ingest(self, documents: list[Document]) -> None:
        with self._client() as c:
            for doc in documents:
                r = c.post(
                    f"/banks/{self._bank(doc.user_id)}/retain",
                    json={
                        "content": doc.content,
                        "context": doc.context,
                        # document_id makes the write an upsert keyed on the
                        # AMB document, so a re-run of the same conversation
                        # replaces rather than duplicates.
                        "document_id": doc.id,
                        # AMB normalizes every dataset's date to ISO 8601; a value
                        # the server cannot date is a 400 here, not a silent loss.
                        "created_at": doc.timestamp,
                        # Dataset-supplied labels (PMB's `scope:*` tags). Without
                        # them on the row there is nothing for a recall filter
                        # to match, so supports_filters would be a lie.
                        "tags": sorted({t for t in (doc.tags or [])}),
                    },
                )
                r.raise_for_status()
                self._doc_by_memory[r.json()["id"]] = doc.id
                if doc.context:
                    self._context_by_doc[doc.id] = doc.context
                if doc.timestamp:
                    self._dated.add(doc.id)
                self._tags_by_doc[doc.id] = {t.lower() for t in (doc.tags or [])}

    def retrieve(self, query: str, k: int = 10, user_id: str | None = None,
                 query_timestamp: str | None = None, filters: dict | None = None) -> tuple[list[Document], dict | None]:
        # H4's reader-drowning axis: MEMORY_WIRE_TOPK caps how many documents
        # the reader is allowed to see, independent of the k the harness asks
        # for. Unset = no cap, the behaviour every earlier run measured.
        topk = os.environ.get("MEMORY_WIRE_TOPK")
        k_eff = min(k, int(topk)) if topk else k
        # AMB's provider-neutral filter. Only the "any" groups are honoured,
        # as a single include-any set in the store — PMB sends exactly one group
        # (its `scope:*` tags); more than one would need AND-of-ANY semantics
        # the HTTP contract does not carry, so that case fails loudly here
        # rather than silently under-filtering. "narrow_any"/"narrow"/"all"/
        # "none" are not honoured: the narrow clauses resolve candidate tags
        # against a tag vocabulary the store does not expose (see the class doc).
        body = {
            "query": query,
            # `format: full` is the only response shape that carries the
            # memory id. The budget is what the ranker fills, not a
            # result cap: memory-wire hard-caps one recall at 100 hits
            # and trims by tokens, so a budget that cannot hold k
            # documents silently returns fewer than k.
            "format": "full",
            "budget": int(os.environ.get("MEMORY_WIRE_BUDGET", "200000")),
        }
        if filters is not None:
            groups = list(filters.get("any") or [])
            if len(groups) > 1:
                raise ValueError(
                    f"memory-wire adapter: {len(groups)} 'any' groups cannot be "
                    "AND-expressed over the HTTP recall contract; refusing to "
                    "silently under-filter"
                )
            if groups:
                body["tags"] = sorted({t for t in groups[0]})
        with self._client() as c:
            r = c.post(
                f"/banks/{self._bank(user_id)}/recall",
                json=body,
            )
            r.raise_for_status()
            hits = r.json()

        docs = []
        for h in hits[:k_eff]:
            doc_id = self._doc_by_memory.get(h["id"], h["id"])
            # A memory retained without a timestamp is stamped at ingest, which is
            # the harness's clock, not the conversation's: serve no date for it.
            dated = doc_id in self._dated
            header = " — ".join(x for x in (self._context_by_doc.get(doc_id),
                                            h.get("created_at") if dated else None) if x)
            content = f"[{header}]\n{h['content']}" if header else h["content"]
            docs.append(Document(id=doc_id, content=content, user_id=user_id,
                                 source_ids=[doc_id]))
        return docs, None
