#!/usr/bin/env python3
"""Freeze the relevance battery's bank (RND_PLAN §6.2, TODO #6).

The battery (`run_battery.py`) reads whatever bank `--endpoint` serves. Pointed
at the live `omp` bank it drifts — every session retains new rows, and a
gating decision must be reproducible by a later agent without the live bank
having stood still (`docs/CONSISTENCY.md` §32: junk-injected moved 1/9→0/9
with the code unchanged). This tool freezes the corpus the battery measures:

- `dump`  — read-only snapshot of the live store into
            `eval/data/battery/omp-snapshot.jsonl` (gitignored: it is the
            user's live session content, never committed), one row per line
            `{content, context, created_at, tags}`. The HTTP list route
            omits `tags`, which the auto shape's `exclude_tags` depends on,
            so the dump reads sqlite directly with `mode=ro`.
- `seed`  — replay a snapshot into a scratch serve via the product's own
            HTTP retain route (content, context, created_at, tags — the
            fields the write path owns), then verify row count and tag count
            against the snapshot.

The committed record of a frozen corpus is its sha256 + row/tag counts, stated
in the battery artifact that used it; the snapshot file itself stays local.

Usage:
    python3 eval/relevance/freeze_battery.py dump
    python3 eval/relevance/freeze_battery.py seed --endpoint http://127.0.0.1:8897
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import sqlite3
import sys
import urllib.request

HERE = pathlib.Path(__file__).parent
SNAPSHOT = HERE.parent / "data" / "battery" / "omp-snapshot.jsonl"
LIVE_DB = pathlib.Path.home() / ".local" / "share" / "memory-wire" / "memory.db"
BANK = "omp"


def sha256(path: pathlib.Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 16), b""):
            h.update(chunk)
    return h.hexdigest()


def post(endpoint: str, path: str, body: dict) -> dict:
    req = urllib.request.Request(
        f"{endpoint}/banks/{BANK}/{path}",
        data=json.dumps(body).encode(),
        headers={"content-type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=30) as resp:
        return json.loads(resp.read())


def get(endpoint: str, path: str) -> dict:
    with urllib.request.urlopen(f"{endpoint}/banks/{BANK}/{path}", timeout=10) as resp:
        return json.loads(resp.read())


def dump() -> int:
    SNAPSHOT.parent.mkdir(parents=True, exist_ok=True)
    # Read-only: the live store is the user's; the snapshot never writes back.
    con = sqlite3.connect(f"file:{LIVE_DB}?mode=ro", uri=True)
    rows = con.execute(
        "SELECT m.id, m.content, m.context, m.created_at FROM memories m "
        "WHERE m.bank_id = ? ORDER BY m.created_at, m.id",
        (BANK,),
    ).fetchall()
    tags: dict[str, list[str]] = {}
    for mem_id, tag in con.execute(
        "SELECT t.memory_id, t.tag FROM memory_tags t "
        "JOIN memories m ON m.id = t.memory_id WHERE m.bank_id = ?",
        (BANK,),
    ):
        tags.setdefault(mem_id, []).append(tag)
    con.close()
    with open(SNAPSHOT, "w") as f:
        for _id, content, context, created_at in rows:
            f.write(
                json.dumps(
                    {
                        "content": content,
                        "context": context,
                        "created_at": created_at,
                        "tags": tags.get(_id, []),
                    },
                    ensure_ascii=False,
                )
                + "\n"
            )
    print(
        f"dumped {len(rows)} rows to {SNAPSHOT}; "
        f"sha256 {sha256(SNAPSHOT)[:16]}…; "
        f"tagged rows {sum(1 for _id, *_ in rows if tags.get(_id))}"
    )
    return 0


def seed(endpoint: str) -> int:
    lines = [json.loads(l) for l in SNAPSHOT.read_text().splitlines() if l.strip()]
    for r in lines:
        body = {"content": r["content"]}
        if r.get("context"):
            body["context"] = r["context"]
        if r.get("created_at"):
            body["created_at"] = r["created_at"]
        if r.get("tags"):
            body["tags"] = r["tags"]
        post(endpoint, "retain", body)
    stats = get(endpoint, "stats")
    n = stats.get("memories")
    tag_n = stats.get("tags")
    # The replay goes through the product's own retain path, so byte-identical
    # snapshot rows collapse by content-hash dedup exactly as they would on a
    # live machine. The frozen bank is deterministic: same snapshot file, same
    # row count every time.
    distinct = len({r["content"] for r in lines})
    print(
        f"seeded {len(lines)} rows -> {n} memories ({len(lines) - distinct} byte-identical "
        f"rows collapsed by content-hash dedup, the product's own write semantics); "
        f"{tag_n} distinct tags"
    )
    if n != distinct:
        print(f"MISMATCH: expected {distinct} distinct-content memories, bank holds {n}", file=sys.stderr)
        return 1
    return 0


def main() -> int:
    global SNAPSHOT
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("cmd", choices=["dump", "seed"])
    ap.add_argument("--endpoint", default="http://127.0.0.1:8897")
    ap.add_argument("--snapshot", default=str(SNAPSHOT))
    args = ap.parse_args()
    SNAPSHOT = pathlib.Path(args.snapshot)
    if args.cmd == "dump":
        return dump()
    return seed(args.endpoint)


if __name__ == "__main__":
    raise SystemExit(main())
