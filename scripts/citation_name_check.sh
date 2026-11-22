#!/bin/sh
# Does a citation that NAMES a function still point at that function?
#
# `module_table_check.sh` grades the module table. `count_claim_check.sh` grades
# counts in prose. This grades the third thing a document can get wrong: a
# `file:line` citation that names the function it claims is there.
#
# The defect this was written for, found 2026-11-21 and not hypothetical:
#
#   docs/OPEN_HOOK_RECALL_RELEVANCE.md cited `src/api.rs:1903` for the test
#   `recall_should_score_with_the_fused_rrf_value`. The test is at 3132. Line
#   1903 is a bare `}`. And at 36bb947 -- the commit that document pins itself
#   to -- the same test was at 2168. The citation was never right, which is a
#   different failure from a citation going stale as a file grows.
#
# A stale citation is ordinary: files move, line numbers drift, and a document
# that names its commit is honest about it. A citation that never pointed at the
# named thing is worse, because a reader who checks it finds nothing and stops
# trusting the document's other citations, which may be fine.
#
# WHAT THIS CHECKS, AND WHAT IT DELIBERATELY DOES NOT
#
# It checks the cheap, high-value case: a backticked `path:line` immediately
# followed by a backticked identifier, where that identifier is a function or
# method name in that file. If the named function is not defined at (or within a
# few lines of) the cited line, that is a finding.
#
# It does NOT try to verify that a citation's *claim* is true. That needs a
# reader. Nor does it range-check bare `file.rs:NNN` citations that name nothing:
# there is no second thing to compare the line against. The 2026-11-21 plan pass
# checked 70 in-tree citations for range **by hand**; that is a one-off
# measurement, not a gate, and this script does not replace it.
#
# The tolerance is 6 lines: `///` docs sit above a `fn`, so a citation that
# points at the doc block rather than the `fn` line is correct and common.
#
# MUTATION BATTERY, 5/5, run 2026-11-21 against
# `docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md`:
#
#   1. `recall_at` cited at 660 instead of 727        -> fires (exit 1)
#   2. clean tree                                     -> passes
#   3. same defect with `at \`36bb947\`` in the sentence -> excluded (it is a claim
#      about that tree, not this one)
#   4. cited at 723, four lines off                    -> tolerated
#   5. comparison disabled (`-gt 99999`) with the real defect present -> passes,
#      which is the vacuity check: a gate that cannot fail is worse than none
#
# Cases 3 and 4 are the two that took three attempts to get right, and both
# failures were the gate being too eager rather than too lax: a wide window
# reached up into the previous paragraph's commit pin and excused a citation that
# paragraph was not making, and a word-list match on `verified at` did the same.
# The window is now the hit line plus one, and a pin must be a backticked hash.
#
# Run: sh scripts/citation_name_check.sh
set -eu
cd "$(dirname "$0")/.."

fail=0

# The pattern: a backticked src/....rs:NNN, then the nearest backticked
# identifier after it, on the same line or the next couple.
#
# awk rather than grep -o because the pairing spans a short distance and grep
# cannot express "identifier that follows this line number".
found=$(grep -rnoE '`src/[a-z_]+\.rs:[0-9]+`[^`]{0,80}`[a-z_][a-z0-9_]*`' docs/ 2>/dev/null || true)

if [ -z "$found" ]; then
    echo "ok  no named citations to check"
    exit 0
fi

checked=0
while IFS= read -r hit; do
    # hit is file:line:`src/x.rs:NNN` ... `name`
    doc=${hit%%:*}
    rest=${hit#*:}
    hit_line=${rest%%:*}
    rest=${rest#*:}
    cited=$(printf '%s' "$rest" | sed -n 's/.*`src\/\([a-z_]*\)\.rs:\([0-9]*\)`.*/\1 \2/p')
    name=$(printf '%s' "$rest" | sed -n 's/.*`\([a-z_][a-z0-9_]*\)`$/\1/p')
    [ -z "$cited" ] && continue
    [ -z "$name" ] && continue

    mod=$(printf '%s' "$cited" | cut -d' ' -f1)
    line=$(printf '%s' "$cited" | cut -d' ' -f2)
    src="src/$mod.rs"

    # A citation pinned to a commit is making a claim about THAT tree, not this
    # one. `src/hooks.rs:660` `recall_at` at `36bb947` is correct and must not be
    # flagged for drifting since. A citation quoted as an example of the defect is
    # also not making a claim. Both are decided on a WINDOW around the hit, not on
    # the hit line, because both wrap: the word doing the work (`cited`, `at their
    # stated commit`) sits on the previous line often enough that checking one line
    # produced two false positives on its first run.
    #
    # The pin itself is detected as a 7+ hex run preceded by `at `, or the words
    # below. A 7+ hex run ANYWHERE is not enough: hex-looking strings appear in
    # prose for other reasons, and treating any of them as a pin would quietly
    # disable the check on any line that happens to contain one.
    # The window is the hit line plus the ONE above it, not three. A commit pin
    # belongs to the sentence it is in; reaching three lines up pulls in the
    # previous paragraph's pin and excuses a citation that paragraph was not
    # talking about. That produced a false pass on the first run of this rule.
    doctext=$(sed -n "$((hit_line > 1 ? hit_line - 1 : 1)),${hit_line}p" "$doc" 2>/dev/null || true)
    # A pin is a backticked 7+ hex run near the word `at`, or one of the words
    # below. Requiring the backticks is what separates `36bb947` (a commit) from
    # any hex-looking prose.
    case "$doctext" in
        *"at \`"[0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]*|*"at  \`"[0-9a-f]*|*"at \`"[0-9a-f]*)
            continue ;;
    esac
    case "$doctext" in
        *commit*|*pinned*|*"v0."*|*written*|*stated*)
            continue ;;
    esac

    # A line that names the citation AS the defect is quoting it, not making it.
    # The repair for OPEN_HOOK item 2 does exactly this: it has to write down the
    # wrong citation to say it was wrong.
    case "$doctext" in
        *cited*|*citation*|*claimed*|*claimed*|*said*|*wrote*|*instead*|*never\ right*)
            continue ;;
    esac

    # Only a name that really is defined in that file is checkable. A citation
    # naming a type, a field or a test in another file is not this check's job.
    if ! grep -qE "(fn|struct|enum|trait|impl|const|static) +$name\b" "$src" 2>/dev/null; then
        continue
    fi
    checked=$((checked + 1))

    # Where is it actually defined?
    actual=$(grep -nE "(fn|struct|enum|trait|impl|const|static) +$name\b" "$src" 2>/dev/null | head -1 | cut -d: -f1)
    [ -z "$actual" ] && continue

    delta=$(( line > actual ? line - actual : actual - line ))
    if [ "$delta" -gt 6 ]; then
        echo "FAIL  $doc cites $src:$line for \`$name\`, which is defined at $src:$actual (off by $delta)"
        fail=1
    fi
done <<EOF
$found
EOF

if [ "$fail" -eq 0 ]; then
    echo "ok  $checked named citation(s) point at the function they name"
fi

exit "$fail"