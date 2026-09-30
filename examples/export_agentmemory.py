#!/usr/bin/env python3
"""Export an agent-memory state_store into memory-wire's import JSON shape.

WHY THIS EXISTS
---------------
agent-memory on this host stores its corpus as ~5,700 `.bin` files under
`~/.agentmemory/data/state_store.db/` (a directory, not a file). Each file is a
JSON object of records followed by an 11-byte binary trailer:

    b'{"obs_ms854lq0_d...pe":"other"}}'  +  b'\x00\x00\x00\xad\x8d\x01\x00\x90\x9c\xff\xff'

The trailer is not documented anywhere in the repo and the two length-looking
fields in it do not correspond to the JSON length (the file is 25,464 B, the JSON
prefix decodes cleanly at 25,453 B, and 0x00018dad is 101,805). So rather than
guess at a frame layout, each file is decoded by finding its **largest valid JSON
prefix** -- robust to whatever the trailer is, and impossible to get wrong by
assuming a format that has not been verified.

OUTPUT
------
The shape `memory-wire`'s `examples/import_hindsight.rs` already consumes, so the
committed importer does the writing, the chunking and the timestamp handling and
this script only translates:

    [{"id": ..., "original_text": ..., "created_at": ..., "tags": [...],
      "document_id": ..., "layer": "obs"|"memory"|...}, ...]

Two layers are exported, and kept distinguishable by tag rather than merged:

  obs     5,996 raw observations, each with a `narrative`. These are the actual
          session content and are what a query would legitimately want back.
  memory    660 distilled memories with a `content` field, produced by
          agent-memory's consolidation pass.

Both are exported because memory-wire has no consolidation of its own, so
throwing away either layer loses real text. The `layer:` tag keeps them
separable after the fact, which is the point of tagging rather than merging.
"""

import glob
import json
import os
import sys
import urllib.parse

STORE = os.path.expanduser("~/.agentmemory/data/state_store.db")
OUT_DIR = os.path.expanduser("~/agentmemory-export-20260930")


def load_records(path):
    """Decode one .bin into a list of record dicts, or [] if undecodable.

    The largest-valid-JSON-prefix scan is O(n^2) in the worst case, so the file
    is first truncated to the last '}' that could plausibly close the object,
    which is where every one of these files actually ends.
    """
    raw = open(path, "rb").read()
    if not raw:
        return []
    end = raw.rfind(b"}")
    while end > 0:
        try:
            obj = json.loads(raw[: end + 1].decode("utf-8"))
        except Exception:
            end = raw.rfind(b"}", 0, end)
            continue
        if isinstance(obj, dict):
            # Yield (key, record) pairs: for some layers the JSON key IS the
            # identity. Summaries carry no `id` field, and keying those on the
            # filename collapsed all six into one row -- the key is what
            # distinguishes them.
            return [(k, v) for k, v in obj.items() if isinstance(v, dict)]
        return []
    return []


def norm_ts(value):
    """Normalise to RFC 3339 Z with at most 3 fractional digits.

    memory-wire's store orders and compares created_at as bytes, so a mixed set
    of formats would sort wrongly.
    """
    if not isinstance(value, str) or not value.strip():
        return None
    s = value.strip()
    body = s[:-1] if s.endswith("Z") else (s[:-6] if s.endswith("+00:00") else s)
    if "." in body:
        secs, frac = body.split(".", 1)
        digits = "".join(c for c in frac if c.isdigit())[:3]
        return "%s.%sZ" % (secs, digits) if digits else "%sZ" % secs
    return body + "Z"


def text_of(rec):
    """Prefer the richest prose field present, in the order agent-memory uses."""
    for field in ("narrative", "content", "fact"):
        val = rec.get(field)
        if isinstance(val, str) and val.strip():
            title, subtitle = rec.get("title"), rec.get("subtitle")
            head = " ".join(x for x in (title, subtitle) if isinstance(x, str) and x.strip())
            return (head + "\n" + val) if head else val
    return None


def tags_for(rec, layer):
    tags = ["layer:" + layer]
    for field in ("type", "project", "source", "sessionId", "id"):
        val = rec.get(field)
        if isinstance(val, str) and val.strip():
            tags.append("%s:%s" % (field, val.strip()[:120]))
    for field in ("concepts", "files", "facts", "filesModified"):
        for item in rec.get(field) or []:
            if isinstance(item, str) and item.strip():
                tags.append(item.strip()[:120])
    return tags


def collect(pattern, layer, timestamp_field):
    """Read one layer's records into export rows. Writes nothing."""
    files = sorted(glob.glob(os.path.join(STORE, pattern)))
    rows, empty, dupes = [], 0, 0
    seen = set()
    for path in files:
        for key, rec in load_records(path):
            body = text_of(rec)
            if not body:
                empty += 1
                continue
            # `id` when the record has one, else the JSON key, else the filename.
            # The middle case is load-bearing: without it, every record in a
            # single-file layer shares a document_id and dedup eats all but one.
            #
            # This is the BARE record id, not a namespaced one. The importer
            # namespaces it with `--source` on the way in, so pre-namespacing
            # here produced `agentmemory:agentmemory:obs:...` in the store.
            rid = rec.get("id") or key or os.path.basename(path)
            doc_id = "agentmemory:%s:%s" % (layer, rid)
            if doc_id in seen:
                dupes += 1
                continue
            seen.add(doc_id)
            rows.append(
                {
                    "id": rid,
                    "original_text": body,
                    "created_at": norm_ts(rec.get(timestamp_field)),
                    "tags": tags_for(rec, layer),
                    "document_id": doc_id,
                    "layer": layer,
                }
            )
    print(
        "  %-8s %6d collected (%d no text, %d duplicate ids)"
        % (layer, len(rows), empty, dupes)
    )
    return rows


def dedupe_by_text(rows):
    """Collapse byte-identical records, keeping the earliest timestamp.

    WHY THIS IS NOT OPTIONAL
    ------------------------
    4,720 of the 6,674 exported records are the *same* 534-byte string: the
    `[IMPORTANT: You are running as a scheduled cron job...]` banner the harness
    injects into every scheduled run. A further 296 are two more fixed banners.
    That is 5,016 of 6,674 records -- 75% -- carrying no session content at all.

    Imported as-is they are actively harmful, not merely redundant: memory-wire
    returns five results per query, so a query matching "conversation" or
    "scheduled" would fill the entire window with copies of a cron banner and
    bury the real memories. The duplicates are byte-identical, so collapsing them
    discards no information; the earliest timestamp is kept because that is when
    the text first became true.
    """
    best = {}
    for rec in rows:
        key = rec["original_text"]
        prev = best.get(key)
        if prev is None:
            best[key] = rec
            continue
        a, b = rec.get("created_at"), prev.get("created_at")
        # keep whichever is earlier; fall back to first-seen when either is None
        if a and b and a < b:
            best[key] = rec
        elif a and not b:
            best[key] = rec
    return list(best.values()), len(rows) - len(best)


def main():
    if not os.path.isdir(STORE):
        sys.exit("FAIL: %s is not a directory; refusing to export." % STORE)
    os.makedirs(OUT_DIR, exist_ok=True)
    print("agent-memory -> memory-wire export")
    print("  source: %s (%d files)" % (STORE, len(os.listdir(STORE))))

    layers = (
        # pattern,                        layer,     output,     timestamp field
        ("mem%3Aobs%3A*.bin", "obs", "obs.json", "timestamp"),
        ("mem%3Amemories*.bin", "memory", "memories.json", "createdAt"),
        ("mem%3Asummaries*.bin", "summary", "summaries.json", "createdAt"),
        ("mem%3Asemantic*.bin", "semantic", "semantic.json", "createdAt"),
    )
    # Collect every layer first so deduplication can see across them: a distilled
    # memory and the observation it came from are frequently the same string, and
    # the tag on the surviving row records which layer it was found in.
    everything = []
    for pattern, layer, _out_name, ts_field in layers:
        everything.extend(collect(pattern, layer, ts_field))

    kept, collapsed = dedupe_by_text(everything)
    print("  dedupe    : collapsed %d byte-identical records" % collapsed)
    print("  KEPT      : %d distinct memories" % len(kept))

    by_layer = {}
    for rec in kept:
        by_layer.setdefault(rec["layer"], []).append(rec)
    for _pattern, layer, out_name, _ts in layers:
        rows = by_layer.get(layer, [])
        path = os.path.join(OUT_DIR, out_name)
        with open(path, "w") as fh:
            json.dump(rows, fh)
        stamps = sorted(r["created_at"] for r in rows if r["created_at"])
        print(
            "  %-9s %6d -> %s  (%s -> %s)"
            % (layer, len(rows), out_name,
               stamps[0] if stamps else "-", stamps[-1] if stamps else "-")
        )
    print("  TOTAL: %d records" % len(kept))
    print("  written to %s" % OUT_DIR)


if __name__ == "__main__":
    main()
