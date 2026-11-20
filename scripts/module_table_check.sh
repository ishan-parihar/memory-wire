#!/bin/sh
# Does AGENTS.md §3's module table still describe `src/`?
#
# The table is the map a fresh agent reads before touching anything, so a table
# that drifts is worse than no table: it is believed. Until 2026-11-20 it listed
# 18 modules and claimed "24.8k lines across 22 modules" against a real
# 32 modules / 35,373 lines, and nothing checked, so nobody noticed. Fourteen
# modules were missing outright, five of them whole features (W3 redact, W4
# hygiene, W5 grounding, the skills door, the update path).
#
# This check is deliberately about the *set* and the *numbers*, not the prose:
# a role line is a judgement a human writes and this script cannot grade it.
# What it can grade is whether the table names every module and whether the
# count beside each one is the count in the tree.
#
# Run: sh scripts/module_table_check.sh
set -eu
cd "$(dirname "$0")/.."

fail=0
tbl=$(mktemp)
trap 'rm -f "$tbl"' EXIT

# One `module<TAB>lines` row per table entry. The table's own row shape is
# `| \`name.rs\` | 1234 | role |`, so this reads exactly that and nothing else.
sed -n 's/^| `\([a-z_]*\.rs\)` | \([0-9]\{1,\}\) | .*$/\1\t\2/p' AGENTS.md > "$tbl"

# Every module in src/ must appear, and every table row must name a file that
# exists. A row for a deleted module misleads as much as a module with no row.
for f in src/*.rs; do
  base=$(basename "$f")
  if ! grep -q "^$base	" "$tbl"; then
    echo "MISSING: $base is in src/ but not in AGENTS.md §3" >&2
    fail=1
  fi
done
while IFS='	' read -r base _lines; do
  if [ ! -f "src/$base" ]; then
    echo "STALE: AGENTS.md §3 lists \`$base\`, which is not in src/" >&2
    fail=1
  fi
done < "$tbl"

# The per-module counts must be the counts in the tree.
while IFS='	' read -r base claimed; do
  [ -f "src/$base" ] || continue
  actual=$(wc -l < "src/$base" | tr -d ' ')
  if [ "$claimed" != "$actual" ]; then
    echo "COUNT: $base is $actual lines, AGENTS.md says $claimed" >&2
    fail=1
  fi
done < "$tbl"

# The table's row count must match the tree's module count, so a table that
# dropped rows cannot pass by simply not listing them.
table_rows=$(wc -l < "$tbl" | tr -d ' ')
actual_modules=$(ls src/*.rs | wc -l | tr -d ' ')
if [ "$table_rows" != "$actual_modules" ]; then
  echo "ROWS: table has $table_rows, src/ has $actual_modules" >&2
  fail=1
fi

# The headline sentence must agree with the sum, so "32 modules, 35,370 lines"
# cannot go stale on its own while the rows are right.
# The headline writes the total with a thousands separator ("35,373"), so the
# comparison strips commas from both sides rather than demanding one spelling.
actual_total=$(cat src/*.rs | wc -l | tr -d ' ')
headline=$(sed -n 's/^`src\/`, \([0-9]\{1,\}\) modules, \([0-9,]*\) lines\..*$/\1 \2/p' AGENTS.md | head -1)
claimed_modules=$(printf '%s' "$headline" | cut -d' ' -f1)
claimed_total=$(printf '%s' "$headline" | cut -d' ' -f2 | tr -d ',')
if [ "$claimed_modules" != "$actual_modules" ] || [ "$claimed_total" != "$actual_total" ]; then
  echo "HEADLINE: expected \`${actual_modules}\` modules, ${actual_total} lines" >&2
  echo "          AGENTS.md §3 says: ${headline:-<nothing matching>}" >&2
  fail=1
fi

if [ "$fail" -eq 0 ]; then
  echo "ok  AGENTS.md §3 matches src/ ($actual_modules modules, $actual_total lines)"
fi
exit "$fail"