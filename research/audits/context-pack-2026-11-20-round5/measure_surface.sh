#!/bin/sh
# Round-5 surface measurement. Every number packed in round 5 comes from this
# script, so the reviewer re-derives a figure instead of trusting a claim.
#
# This is round 4's script with its measured defects fixed:
#   - the host regex listed 6 of the 8 hosts connect.rs actually declares
#     (round 4 §2: "wouldn't even catch a 7th host or pi/omp"). The comment in
#     the first draft of this file said 9 -- the reviewer caught it, and the
#     enum is the authority, not a count in a header.
#   - the AGENTS.md claim grep targeted the pre-2026-11-20 wording
#   - the code-vs-test split was never measured, which is what the round-4
#     falsifier (">50% of connect_* is inline #[test] fixtures") needed
#
# Run from the repo root: sh research/audits/context-pack-2026-11-20-round5/measure_surface.sh
set -eu
cd "$(dirname "$0")/../../.."

echo "## src/ — authored Rust, per module (wc -l; includes inline #[test] blocks)"
wc -l src/*.rs | sort -rn

echo
echo "## src/ — totals"
printf 'files: %s\n' "$(ls src/*.rs | wc -l | tr -d ' ')"
printf 'lines: %s\n' "$(cat src/*.rs | wc -l | tr -d ' ')"

echo
echo "## src/ — code vs inline tests, per module (the split round 4 flagged as unevidenced)"
# Everything above the module's `#[cfg(test)]` is production code; everything
# below is test. A per-file awk because the split is not derivable from wc.
for f in src/*.rs; do
  at=$(grep -n '^#\[cfg(test)\]' "$f" | head -1 | cut -d: -f1)
  total=$(wc -l < "$f" | tr -d ' ')
  if [ -n "$at" ]; then code=$((at - 1)); else code=$total; fi
  printf '%s total=%s code=%s test=%s\n' "$(basename "$f")" "$total" "$code" "$((total - code))"
done | sort -t= -k3 -rn

echo
echo "## connect_* — the per-host adapters"
wc -l src/connect*.rs | sort -rn

echo
echo "## Doors (retrieval / injection surfaces)"
printf 'HTTP route registrations: %s\n' "$(grep -rhoE '\.route\("[^"]+"' src/*.rs | wc -l | tr -d ' ')"
grep -rhoE '\.route\("[^"]+"' src/*.rs | sed 's/.*route("//;s/"//' | sort -u
printf 'MCP tools registered in tools(): %s\n' "$(sed -n '/pub fn tools()/,/^    }/p' src/mcp.rs | grep -cE 'Tool::new\(')"
printf 'MCP prompts: %s\n' "$(sed -n '/fn prompts()/,/^    }/p' src/mcp.rs | grep -cE 'Prompt::new\(')"
printf 'CLI subcommands: %s\n' "$(sed -n '/^enum Cmd {/,/^}/p' src/main.rs | grep -cE '^    [A-Z]')"
sed -n '/^enum Cmd {/,/^}/p' src/main.rs | grep -E '^    [A-Z]' | sed 's/^ *//;s/[{,(].*//' | tr '\n' ' '
echo

echo
echo "## MCP tools and prompts"
grep -ohE 'pub const [A-Z_]+: &str = "memory_[a-z_]+"' src/mcp.rs \
  | sed 's/.*= "//;s/"//' | sort -u
echo "  (memory_historian is a PROMPT constant, not a tool — see tools() above)"

echo
echo "## Agent hosts — sourced from the enum connect.rs declares, not a literal list"
# Round 4's regex hardcoded six names and missed pi, omp and the plugin hosts.
# The Host enum is the authority; its variant arms are at one indent level.
sed -n '/^pub enum Host {/,/^}/p' src/connect.rs | grep -E '^    [A-Z]' | sed 's/^ *//;s/[{,(].*//' | tr '\n' ' '
echo
printf 'ALL_HOSTS / IMPLICIT count: %s / %s\n' \
  "$(sed -n '/pub const ALL: &\[Host\]/,/;/p' src/connect.rs | grep -c 'Host::')" \
  "$(sed -n '/pub const IMPLICIT: &\[Host\]/,/;/p' src/connect.rs | grep -c 'Host::')"

echo
echo "## plugin/ — the injection path (runs inside a model's context)"
find plugin -name '*.ts' -o -name '*.py' | grep -v node_modules | xargs wc -l 2>/dev/null | sort -rn

echo
echo "## TS plugin duplication — lines present verbatim in BOTH files"
A=plugin/integrations/extension/memory-wire.ts
B=plugin/integrations/opencode-plugin/memory-wire.ts
shared=$(comm -12 <(sort "$A") <(sort "$B") | grep -vE '^[[:space:]]*$|^[[:space:]]*(//|\*)' | wc -l | tr -d ' ')
a_code=$(grep -vE '^[[:space:]]*$|^[[:space:]]*(//|\*)' "$A" | wc -l | tr -d ' ')
b_code=$(grep -vE '^[[:space:]]*$|^[[:space:]]*(//|\*)' "$B" | wc -l | tr -d ' ')
printf 'shared verbatim: %s of %s (%.0f%%)\n' "$shared" "$b_code" \
  "$(python3 -c "print(100*$shared/$b_code)")"
printf 'extension non-comment: %s\n  opencode non-comment: %s\n' "$a_code" "$b_code"
echo "  (a shared line is shared syntax by construction: comm overcounts '}')"

echo
echo "## Authored docs (target/, research/ excluded)"
printf 'docs/*.md: %s\n' "$(ls docs/*.md | wc -l | tr -d ' ')"
printf 'docs/**/*.md: %s\n' "$(find docs -name '*.md' | wc -l | tr -d ' ')"

echo
echo "## AGENTS.md module table vs tree (gate: scripts/module_table_check.sh)"
sh scripts/module_table_check.sh || echo "  DRIFT"

echo
echo "## Dependency count (the default build must not gain one — AGENTS 1)"
awk '/^\[dependencies\]/{f=1;next} /^\[/{f=0} f && /^[a-zA-Z0-9_-]+ *=/' Cargo.toml \
  | sed 's/[[:space:]]*=.*//' | tr '\n' ' '
echo
printf 'count: %s\n' "$(awk '/^\[dependencies\]/{f=1;next} /^\[/{f=0} f && /^[a-zA-Z0-9_-]+ *=/' Cargo.toml | wc -l | tr -d ' ')"

echo
echo "## Footprint (AGENTS 1: one static binary, 3 shared libs)"
ls -l target/release/memory-wire | awk '{printf "binary bytes: %s\n", $5}'
readelf -d target/release/memory-wire 2>/dev/null | grep -c NEEDED || echo 0
echo "  ^ DT_NEEDED count"
