#!/usr/bin/env python3
"""Verify the agent-memory -> memory-wire port on racknerd.

Three claims are checked, each against the export rather than against a
self-report of the import:

  1. losslessness  - every exported record's text is present in the store,
                     byte for byte, and the count matches
  2. timestamps     - the real conversation time survived, spread over the
                     days the corpus actually covers
  3. retrievability - FTS is populated, so recall has something to search
"""

import json
import os
import sqlite3
import sys

EXPORT = os.path.expanduser("~/agentmemory-export-20260930")
DB = os.path.expanduser("~/.local/share/memory-wire/memory.db")
BANK = "agentmemory"

exported = {}
for name in ("obs", "memories", "summaries", "semantic"):
    for rec in json.load(open(os.path.join(EXPORT, name + ".json"))):
        exported[rec["document_id"]] = rec

c = sqlite3.connect(DB)
c.row_factory = sqlite3.Row
rows = {r["document_id"]: r for r in c.execute(
    "select document_id, content, created_at from memories where bank_id=?", (BANK,))}
tags = {r["memory_id"] for r in c.execute(
    "select memory_id from memory_tags where memory_id in "
    "(select id from memories where bank_id=?)", (BANK,))}

fail = 0

# 1. losslessness
#
# Compared by CONTENT, not by document_id. An earlier version of this script
# matched on the raw `document_id`, which the importer namespaces with a source
# prefix (`agentmemory:agentmemory:obs:...`). The two key sets were therefore
# disjoint, every lookup missed, and the mismatch check reported 0 while
# comparing an empty intersection -- a check that cannot fail is not a check.
# The content set is the actual claim and does not depend on id formatting.
store_text = {}
for _d, r in rows.items():
    store_text.setdefault(r["content"], []).append(_d)

export_text = {}
for d, rec in exported.items():
    export_text.setdefault(rec["original_text"], []).append(d)

missing = [t for t in export_text if t not in store_text]
mismatched = [t for t in export_text
              if t in store_text and len(store_text[t]) != len(export_text[t])]
extra = [t for t in store_text if t not in export_text]

print("  records exported      : %d" % len(exported))
print("  rows in bank          : %d" % len(rows))
print("  distinct texts export : %d" % len(export_text))
print("  distinct texts stored : %d" % len(store_text))
print("  texts missing from store: %d" % len(missing))
print("  text count mismatches : %d" % len(mismatched))
print("  stored texts not in export: %d" % len(extra))
if missing or mismatched or extra:
    fail += 1
    for t in (missing + mismatched + extra)[:3]:
        print("    e.g. %.70r" % t[:70])

# 2. timestamps
stamps = sorted(r["created_at"] for r in rows.values() if r["created_at"])
days = sorted({s[:10] for s in stamps})
expected = sorted(r["created_at"] for r in exported.values() if r["created_at"])
print("  timestamp range       : %s -> %s" % (stamps[0], stamps[-1]))
print("  distinct days         : %d" % len(days))
print("  timestamps match export: %s" % (stamps == expected))
if stamps != expected:
    fail += 1
bad = [s for s in stamps if not s.endswith("Z") or s.endswith("ZZ")]
print("  malformed timestamps  : %d" % len(bad))
if bad:
    fail += 1

# 3. tags + FTS
print("  tagged rows           : %d / %d" % (len(tags), len(rows)))
fts = c.execute(
    "select count(*) from memories_fts where memories_fts match 'memory'").fetchone()[0]
print("  FTS rows matching 'memory': %d" % fts)
if fts == 0:
    fail += 1

print("  VERDICT: %s" % ("PASS" if fail == 0 else "FAIL (%d checks)" % fail))
sys.exit(1 if fail else 0)
