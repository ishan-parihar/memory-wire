#!/usr/bin/env python3
"""One-time corpus migration for the retrieval-efficacy plan (GAP-3/7).

Banks carry rows the auto-injection surfaces must stop emitting, without
losing them from an explicit search's reach:

- every one-time Hindsight transcript import (``context LIKE 'imported%'``)
  gains the ``transcript`` and ``imported`` tags;
- every bare ``hook:stop`` session marker gains the ``marker`` tag;
- the "quick brown fox" test rows are deleted — through the HTTP surface, not
  raw SQL, because FTS5 external-content rows must be removed the way the
  product removes them.

Dry-run by default; ``--apply`` writes. The store is snapshotted with
SQLite's online-backup API before anything is written, so the migration is
reversible by restoring the snapshot. Re-running is a no-op: the tag inserts
are ``OR IGNORE`` and the deletes are by id.

Usage:
    scripts/retag_imports.py                      # dry-run, prints counts
    scripts/retag_imports.py --apply              # snapshot + write
    scripts/retag_imports.py --db /path --bank omp --apply
"""

from __future__ import annotations

import argparse
import json
import pathlib
import sqlite3
import sys
import time
import urllib.request

DEFAULT_DB = str(pathlib.Path.home() / ".local/share/memory-wire/memory.db")
DEFAULT_ENDPOINT = "http://127.0.0.1:8888"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--db", default=DEFAULT_DB, help="memory-wire SQLite file")
    ap.add_argument("--bank", default="omp", help="bank id to migrate")
    ap.add_argument("--endpoint", default=DEFAULT_ENDPOINT, help="memory-wire serve URL")
    ap.add_argument("--apply", action="store_true", help="write; without it, only report")
    args = ap.parse_args()

    # Open read-write even for the dry-run: a WAL database's read-only mode
    # needs -shm write access anyway, and nothing is written before --apply.
    conn = sqlite3.connect(args.db, timeout=10)
    conn.execute("PRAGMA busy_timeout=5000")

    imported = conn.execute(
        "SELECT COUNT(*) FROM memories WHERE bank_id=? AND context LIKE 'imported%'",
        (args.bank,),
    ).fetchone()[0]
    markers = conn.execute(
        "SELECT COUNT(*) FROM memories WHERE bank_id=? AND context='hook:stop'",
        (args.bank,),
    ).fetchone()[0]
    fox_ids = [
        row[0]
        for row in conn.execute(
            "SELECT id FROM memories WHERE bank_id=? AND content LIKE '%quick brown fox%'",
            (args.bank,),
        )
    ]
    print(f"bank {args.bank!r}: {imported} transcript imports, {markers} stop markers, "
          f"{len(fox_ids)} test rows {fox_ids}")

    if not args.apply:
        print("dry-run: nothing written (pass --apply)")
        return 0

    # Snapshot first, via the online-backup API: consistent even with serve
    # writing, and restorable by file copy.
    backup = pathlib.Path(args.db).with_suffix(
        f".pre-retag-{time.strftime('%Y%m%d-%H%M%S')}.sqlite"
    )
    dest = sqlite3.connect(backup)
    conn.backup(dest)
    dest.close()
    print(f"snapshot: {backup}")

    for sql, tag in (
        ("SELECT id FROM memories WHERE bank_id=? AND context LIKE 'imported%'", "transcript"),
        ("SELECT id FROM memories WHERE bank_id=? AND context LIKE 'imported%'", "imported"),
        ("SELECT id FROM memories WHERE bank_id=? AND context='hook:stop'", "marker"),
    ):
        ids = [row[0] for row in conn.execute(sql, (args.bank,))]
        conn.executemany(
            "INSERT OR IGNORE INTO memory_tags(memory_id, tag) VALUES (?, ?)",
            [(mid, tag) for mid in ids],
        )
        print(f"tagged {len(ids)} rows: {tag}")
    conn.commit()

    # Test rows go through the product's delete path so the FTS index stays
    # consistent; raw SQL against memories would orphan the external-content rows.
    for mid in fox_ids:
        req = urllib.request.Request(
            f"{args.endpoint}/banks/{args.bank}/memories/{mid}", method="DELETE"
        )
        try:
            with urllib.request.urlopen(req, timeout=5) as resp:
                print(f"deleted {mid}: {resp.read().decode().strip()}")
        except Exception as e:  # noqa: BLE001 - an ops script reports, it does not crash
            print(f"delete {mid} FAILED: {e}", file=sys.stderr)
            return 1
    conn.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
