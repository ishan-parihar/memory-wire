#!/bin/sh
# Round-4 surface measurement. Every number in the pack comes from here, so the
# reviewer can re-derive any of them instead of trusting a claim.
#
# Run from the repo root:  sh research/audits/context-pack-2026-11-19-round4/measure_surface.sh
set -eu
cd "$(dirname "$0")/../../.."

echo "## src/ — authored Rust, per module (line counts include inline #[test] blocks)"
wc -l src/*.rs | sort -rn

echo
echo "## src/ — totals"
printf 'files: %s\n' "$(ls src/*.rs | wc -l | tr -d ' ')"
printf 'lines: %s\n' "$(cat src/*.rs | wc -l | tr -d ' ')"
printf 'inline #[test] blocks: %s\n' "$(grep -h '#\[test\]' src/*.rs | wc -l | tr -d ' ')"

echo
echo "## connect_* — the per-host adapters"
wc -l src/connect*.rs | sort -rn

echo
echo "## plugin/ — the injection path (runs inside a model's context)"
find plugin -name '*.ts' -not -path '*/node_modules/*' | xargs wc -l 2>/dev/null | tail -1
find plugin -name '*.py' -not -path '*/node_modules/*' | xargs wc -l 2>/dev/null | tail -1
echo "  files:"
find plugin -name '*.ts' -not -path '*/node_modules/*' -o -name '*.py' -not -path '*/node_modules/*' | sed 's/^/    /'

echo
echo "## TS plugin duplication — lines present verbatim in BOTH files"
A=plugin/integrations/extension/memory-wire.ts
B=plugin/integrations/opencode-plugin/memory-wire.ts
shared=$(comm -12 <(sort "$A") <(sort "$B") | grep -vE '^[[:space:]]*$|^[[:space:]]*//|^[[:space:]]*\*' | wc -l | tr -d ' ')
a_code=$(grep -vE '^[[:space:]]*$|^[[:space:]]*//|^[[:space:]]*\*' "$A" | wc -l | tr -d ' ')
b_code=$(grep -vE '^[[:space:]]*$|^[[:space:]]*//|^[[:space:]]*\*' "$B" | wc -l | tr -d ' ')
echo "  shared verbatim: $shared"
echo "  extension non-comment lines: $a_code"
echo "  opencode  non-comment lines: $b_code"

echo
echo "## HTTP routes (every .route() registration, nested routers included)"
grep -rhoE '\.route\("[^"]+"' src/*.rs | sed 's/.*route("//;s/"//' | sort -u

echo
echo "## MCP tools and prompts"
# The declared tool-name constants, not every string literal that looks like one:
# a bare grep also catches `memory_nope`, which is a test's unknown-tool probe.
grep -ohE 'pub const [A-Z_]+: &str = "memory_[a-z_]+"' src/mcp.rs \
  | sed 's/.*= "//;s/"//' | sort -u
echo "  (memory_historian is declared as a prompt, not a tool — see the table)"

echo
echo "## CLI subcommands"
sed -n '/^enum Cmd {/,/^}/p' src/main.rs | grep -E '^[[:space:]]{4}[A-Z][A-Za-z]+' | sed 's/[[:space:]]//g' | tr -d '{,'

echo
echo "## Agent hosts detected by connect"
grep -ohE '"(claude-code|codex|copilot-cli|cursor|opencode|hermes)"' src/connect*.rs | sort -u | tr -d '"'

echo
echo "## Authored docs"
printf 'docs/*.md count: %s\n' "$(ls docs/*.md 2>/dev/null | wc -l | tr -d ' ')"
wc -l AGENTS.md README.md TODO.md | tail -1

echo
echo "## AGENTS.md module table vs tree"
printf 'AGENTS.md claims: %s\n' "$(grep -oE '`src/`, [0-9.]+k lines across [0-9]+ modules' AGENTS.md)"
printf 'AGENTS.md table rows: %s\n' "$(grep -cE '^\| `[a-z_]+\.rs` \| [0-9]+ \|' AGENTS.md)"
printf 'actual modules: %s\n' "$(ls src/*.rs | wc -l | tr -d ' ')"
echo "  modules absent from the AGENTS.md table:"
for f in $(ls src/*.rs | xargs -n1 basename); do
  grep -q "| \`$f\` |" AGENTS.md || echo "    $f"
done

echo
echo "## Gate status"
printf 'cargo test (release-mode harness): %s\n' "see docs/CONSISTENCY.md"

echo
echo "## Dependency count (the default build must not gain one)"
# Count only the [dependencies] block's own entries: the range above runs to the
# next `[` header, which on this manifest is a feature table full of `x = [...]`
# lines that would otherwise be counted as crates.
awk '/^\[dependencies\]/{f=1;next} /^\[/{f=0} f && /^[a-zA-Z0-9_-]+ *=/' Cargo.toml | wc -l | tr -d ' '
echo "  (the [features] block below is not dependencies)"
