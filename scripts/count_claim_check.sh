#!/bin/sh
# Do the COUNTS in AGENTS.md still match the code?
#
# `module_table_check.sh` grades one table: every module named, every line count
# right. This grades the other counts -- the ones written in prose, which is
# where they actually drift.
#
# The evidence is eleven instances, not a hunch. TODO 34 corrected the MCP tool
# count in nine places and missed a tenth (hermes' `get_tool_schemas` still said
# "the same four tools" when MCP serves six). Then on 2026-11-21, writing the
# next plan found an eleventh: AGENTS.md opened "**Agent hosts.** Six" and listed
# six, while `Host::ALL` holds eight and `IMPLICIT` holds seven.
#
# Every one of those is a number a human wrote in a sentence. None is checked by
# the compiler, and a wrong one is believed -- an agent reading "six tools" does
# not go and count them.
#
# So each count here is sourced from the code and compared against the prose.
# The rule is that a count in AGENTS.md must be the count the enum or the
# function actually produces. When it drifts, this fails and names both numbers.
#
# Run: sh scripts/count_claim_check.sh
set -eu
cd "$(dirname "$0")/.."

fail=0

# --- MCP tools ---------------------------------------------------------------
#
# The authoritative count is `Server::tools()`, and `src/mcp.rs` already has a
# test pinning the exact set (tools_list_should_show_six_annotated_tools). What
# that test cannot see is prose in another file claiming a different number.
#
# The names are read from the tool constants rather than counted by grepping the
# whole file, because `src/mcp.rs` mentions tool names in docs and tests too.
mcp_tools=$(grep -oE 'name: "[a-z_]+"' src/mcp.rs 2>/dev/null | wc -l | tr -d ' ')
if [ "$mcp_tools" -eq 0 ]; then
  # Fall back to the constants the tool list is built from.
  mcp_tools=$(sed -n '/pub fn tools() -> Vec<Tool>/,/^    }/p' src/mcp.rs \
    | grep -cE '^\s+(RETAIN|RECALL|REFLECT|CONFIG_GET|SUPERSEDE|REDACT),')
fi

# Every "N tools" claim ABOUT OUR OWN SURFACE.
#
# The distinction is load-bearing. INTEGRATION_GAPS compares our surface against
# hindsight's 8 and agentmemory's 54, and those numbers are correct and must not
# be "fixed". Only a claim about memory-wire's own tool count is this script's
# business -- which is why the first version of this check, which flagged 54 and
# 4 as errors, was wrong until it learned to tell them apart.
for f in AGENTS.md README.md docs/INTEGRATION_GAPS.md; do
  [ -f "$f" ] || continue
  while IFS= read -r line; do
    # Skip a table row: in INTEGRATION_GAPS' comparison table each cell is a
    # different project, so "54 tools" there is agentmemory's count and
    # correct. A table row is not a claim about this project.
    case "$line" in
      *'|'*)
        continue ;;
    esac
    # Skip a line that names a competitor in the same breath.
    case "$line" in
      *hindsight*|*agentmemory*|*Hindsight*|*agent-memory*|*OpenViking*|*MemOS*|*Honcho*)
        continue ;;
    esac
    n=$(printf '%s' "$line" | grep -oE '[0-9]+ tools' | head -1 | grep -oE '^[0-9]+')
    [ -n "${n:-}" ] || continue
    if [ "$n" != "$mcp_tools" ]; then
      echo "TOOLS: $f says \"$n tools\" about this project, src/mcp.rs serves $mcp_tools" >&2
      fail=1
    fi
  done <<EOF
$(grep -nE '[0-9]+ tools' "$f" 2>/dev/null || true)
EOF
done

# --- Agent hosts -------------------------------------------------------------
#
# `Host::ALL` is the authority. The prose must say eight, and the number of
# host names in the "Agent hosts" sentence must match.
hosts=$(sed -n '/pub const ALL: &\[Host\]/,/^];/p' src/connect.rs | grep -c 'Host::')
implicit=$(sed -n '/pub const IMPLICIT/,/^];/p' src/connect.rs | grep -c 'Host::')

claim=$(grep -oE '\*\*[A-Za-z ]*hosts\.\*\* [A-Za-z]+' AGENTS.md 2>/dev/null | head -1 | grep -oE '[A-Za-z]+$' || true)
if [ -n "${claim:-}" ]; then
  case "$claim" in
    Eight|eight) n=8 ;;
    Seven|seven) n=7 ;;
    Six|six)     n=6 ;;
    *)           n="" ;;
  esac
  if [ -n "${n:-}" ] && [ "$n" != "$hosts" ]; then
    echo "HOSTS: AGENTS.md says \"$claim\", Host::ALL has $hosts" >&2
    fail=1
  fi
fi

# The additive count, when the prose states one.
# `|| true` on both: a grep that matches nothing exits 1, and under `set -e`
# that would abort the whole check rather than simply finding no claim.
claim_imp=$(grep -oE '\*\*[A-Za-z]+ are additive\*\*' AGENTS.md 2>/dev/null | head -1 | grep -oE '^\*\*[A-Za-z]+' || true)
if [ -n "${claim_imp:-}" ]; then
  case "$claim_imp" in
    Eight) n=8 ;; Seven) n=7 ;; Six) n=6 ;; *) n="" ;;
  esac
  if [ -n "${n:-}" ] && [ "$n" != "$implicit" ]; then
    echo "IMPLICIT: AGENTS.md says \"$claim_imp are additive\", IMPLICIT has $implicit" >&2
    fail=1
  fi
fi

# --- Module count ------------------------------------------------------------
#
# Already covered by module_table_check.sh, repeated here so this script is the
# one place a reader checks every count.
mods=$(ls src/*.rs | wc -l | tr -d ' ')
claim_mod=$(sed -n 's/^`src\/`, \([0-9]\{1,\}\) modules,.*$/\1/p' AGENTS.md 2>/dev/null | head -1 || true)
if [ -n "${claim_mod:-}" ] && [ "$claim_mod" != "$mods" ]; then
  echo "MODULES: AGENTS.md says \"$claim_mod modules\", src/ has $mods" >&2
  fail=1
fi

if [ "$fail" -eq 0 ]; then
  echo "ok  AGENTS.md counts match the code ($mcp_tools tools, $hosts hosts, $implicit implicit, $mods modules)"
fi
exit "$fail"
