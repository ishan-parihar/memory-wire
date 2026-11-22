#!/bin/sh
# Does README's tool TABLE enumerate the tools the server actually serves?
#
# `count_claim_check.sh` grades a NUMBER in prose and deliberately skips every
# table row, because INTEGRATION_GAPS' comparison table mixes projects and its
# "54" is agentmemory's, not ours. That skip is right for a *comparison* table
# and wrong for an *enumeration* one -- a table whose rows are our own tool
# names.
#
# `README.md` had exactly that table and it was wrong twice over:
#
#   1. It listed FIVE rows for a server that serves six. `memory_redact`
#      (added 2026-11-20 by TODO #34) had no row, while the sentence directly
#      above the table said "six tools". Every gate passed.
#   2. The `memory_supersede` row named its argument `memory_id`; the server's
#      schema says `id`. A client following the table got
#      `id is required and must be a string` -- a caller-visible failure.
#
# Neither is a count, which is why the count gate never saw either. They were
# found on 2026-11-22 by reading the running server against its own docs.
#
# THE RULE. A row whose first cell is a backticked `memory_*` name is a claim
# about our surface, so it must name a tool `src/mcp.rs` serves. And every
# served tool must have such a row. A competitor's tool count cannot appear in
# this form, so the INTEGRATION_GAPS false positives the other script guards
# against do not come back.
#
# THE SERVED SET is read from the constants the tool list is built from, so this
# cannot disagree with `count_claim_check.sh` about what the server offers.
#
# MUTATION BATTERY (run it with `--battery`; each case is run against a copy, so
# the working tree is never modified):
#
#   1. delete the `memory_redact` row         -> must fail: a served tool with no row
#   2. add a row for `memory_nope`            -> must fail: a row for an unserved tool
#   3. write `memory_id` instead of `id`      -> must fail: the row names a wrong required arg
#   4. the unmodified tree                    -> must pass
#
# All four were run before this script was committed. A gate that has never been
# shown to fail is a comment.
#
# Run: sh scripts/tool_table_check.sh [--battery]
set -eu
cd "$(dirname "$0")/.."

if [ "${1:-}" = "--battery" ]; then
  tmp=$(mktemp)
  cp README.md "$tmp"
  trap 'cp "$tmp" README.md; rm -f "$tmp"' EXIT INT TERM

  bad=0

  sed -i '/^| `memory_redact` | yes |/d' README.md
  if sh scripts/tool_table_check.sh >/dev/null 2>&1; then
    echo "BATTERY: case 1 passed when it must fail (missing row)" >&2; bad=1
  else echo "ok  battery 1: a missing row is caught"; fi
  cp "$tmp" README.md

  sed -i 's/^| `memory_supersede` |/| `memory_nope` | no | none |\n| `memory_supersede` |/' README.md
  if sh scripts/tool_table_check.sh >/dev/null 2>&1; then
    echo "BATTERY: case 2 passed when it must fail (unserved row)" >&2; bad=1
  else echo "ok  battery 2: a row for an unserved tool is caught"; fi
  cp "$tmp" README.md

  sed -i 's/^| `memory_supersede` | yes | `id` (required)/| `memory_supersede` | yes | `memory_id` (required)/' README.md
  if sh scripts/tool_table_check.sh >/dev/null 2>&1; then
    echo "BATTERY: case 3 passed when it must fail (wrong arg name)" >&2; bad=1
  else echo "ok  battery 3: a row naming a wrong required arg is caught"; fi
  cp "$tmp" README.md

  if sh scripts/tool_table_check.sh >/dev/null 2>&1; then
    echo "ok  battery 4: the real tree passes"
  else
    echo "BATTERY: the real tree FAILED" >&2; bad=1
  fi
  exit "$bad"
fi

fail=0

# The tool constants, minus HISTORIAN: that one is a prompt, advertised through
# `prompts/list`, and `Server::tools()` does not register it. `src/mcp.rs`
# documents the offset at length; a count of constants here would be one too
# high by construction.
served='
memory_retain
memory_recall
memory_reflect
memory_bank_config_get
memory_supersede
memory_redact
'

# The authoritative count, so this script's own list cannot drift from the
# gate that grades the prose. If they ever disagree, this fails loudly rather
# than quietly grading against a stale list.
mcp_tools=$(grep -oE 'name: "[a-z_]+"' src/mcp.rs 2>/dev/null | wc -l | tr -d ' ')
if [ "$mcp_tools" -eq 0 ]; then
  mcp_tools=$(sed -n '/pub fn tools() -> Vec<Tool>/,/^    }/p' src/mcp.rs \
    | grep -cE '^\s+(RETAIN|RECALL|REFLECT|CONFIG_GET|SUPERSEDE|REDACT),')
fi
listed=$(printf '%s' "$served" | grep -c '^memory_')
if [ "$listed" != "$mcp_tools" ]; then
  echo "TABLE: internal error -- this script lists $listed tools, src/mcp.rs serves $mcp_tools" >&2
  exit 1
fi

# 1. A row for a tool the server does not serve.
rows=$(grep -oE '^\| `(memory_[a-z_]+)`' README.md 2>/dev/null | tr -d '`|' || true)
for row in $rows; do
  case "$served" in
    *"
$row
"*) ;;
    *)
      echo "TABLE: README.md has a row for \`$row\`, which src/mcp.rs does not serve" >&2
      fail=1 ;;
  esac
done

# 2. A served tool with no row.
for tool in $served; do
  grep -qE "^\| \`$tool\`" README.md 2>/dev/null || {
    echo "TABLE: README.md has no row for \`$tool\`, which the server serves" >&2
    fail=1
  }
done

# 3. A row that names an argument the tool does not take. This is the second
#    defect the README table had: `memory_supersede` was documented as taking
#    `memory_id`, and the server's schema says `id`. A client that followed the
#    table got an error, so a row's REQUIRED arguments are checked against the
#    schema rather than merely counted.
#
#    The mapping is table -> required arg names. It is deliberately explicit:
#    a generic parse of the prose would have to guess which word is the
#    argument, and a guess that is wrong in the lenient direction is the defect
#    this whole script exists to prevent.
check_row_args() {
  tool=$1
  expected=$2
  line=$(grep -E "^\| \`$tool\`" README.md 2>/dev/null || true)
  [ -n "$line" ] || return 0   # absence is case 2's job
  for arg in $expected; do
    case "$line" in
      *"\`$arg\`"*) ;;
      *)
        echo "TABLE: README.md's \`$tool\` row does not name its required argument \`$arg\`" >&2
        fail=1 ;;
    esac
  done
}

check_row_args memory_retain   content
check_row_args memory_recall    query
check_row_args memory_reflect   query
check_row_args memory_supersede id
check_row_args memory_redact    'id pattern'

if [ "$fail" -eq 0 ]; then
  echo "ok  README's tool table has one row per served tool ($listed), no row for an unserved one, and each row names its required arguments"
fi
exit "$fail"
