#!/bin/sh
# All three plugin doors carry the same trust gate as `src/hooks.rs`: the two
# TypeScript plugins and the hermes Python provider. They had none, and each
# injected every row a recall returned, so an untrusted mount's web content
# could reach a model's system prompt as rules. The hermes gate landed first
# and had no executable check until 2026-11-19, which is the shape this file
# exists to prevent: a policy enforced in N places is only enforced if every
# place is checked.
#
# Each check executes the door's real code -- no transcription -- and asserts
# both directions: `untrusted_external` is withheld, and an absent `trust`
# field is KEPT (a server old enough not to send one is not evidence).
#
# Run:  sh plugin/integrations/trust_gate_test.sh
set -eu

cd "$(dirname "$0")/../.."

# The hermes provider is Python, so it gets its own hermetic runner: it drives
# the real `MemoryWireProvider.prefetch` against a local HTTP stub, with no
# hermes runtime and no network.
python3 plugin/integrations/hermes/trust_gate_test.py

for f in plugin/integrations/extension/memory-wire.ts \
         plugin/integrations/opencode-plugin/memory-wire.ts; do
  node - "$f" <<'JS'
const fs = require('fs');
const file = process.argv[2];
const lines = fs.readFileSync(file, 'utf8').split('\n');

let s = lines.findIndex(l => l.trim() === 'var UNTRUSTED_TIER = "untrusted_external";');
if (s < 0) { console.error(file + ': no UNTRUSTED_TIER'); process.exit(1); }
for (let i = s; i >= 0; i--) { if (lines[i].includes('/**')) { s = i; break; } }

const start = lines.findIndex((l, i) => i > s && l.startsWith('function formatRecall'));
if (start < 0) { console.error(file + ': no formatRecall'); process.exit(1); }
let depth = 0, e = -1;
for (let i = start; i < lines.length; i++) {
  for (const ch of lines[i]) { if (ch === '{') depth++; else if (ch === '}') depth--; }
  if (depth === 0 && i > start) { e = i; break; }
}

const body = lines.slice(s, e + 1).join('\n');
const formatRecall = new Function(
  'bank', 'clamp', 'RECALL_ENTRIES', 'ENTRY_CAP_CHARS', 'INJECT_CAP_CHARS',
  body + '\nreturn formatRecall;'
)(() => 'bank', (x, n) => x.slice(0, n), 5, 400, 2000);

const items = [
  { id: 'a', score: 1, content: 'TRUSTED-ONE', trust: 'user_verified',
    created_at: '2026-11-19T09:30:00.000Z' },
  { id: 'b', score: 2, content: 'LEAK-B', trust: 'untrusted_external',
    created_at: '2026-11-18T09:30:00.000Z' },
  { id: 'c', score: 3, content: 'TRUSTED-TWO', trust: 'agent_inferred',
    created_at: '2026-11-17T09:30:00.000Z' },
  { id: 'd', score: 4, content: 'LEAK-D', trust: 'untrusted_external',
    created_at: '2026-11-16T09:30:00.000Z' },
];
const out = formatRecall(items);
const fail = [];
if (/LEAK-/.test(out)) fail.push('untrusted_external reached the block');
if (!/TRUSTED-ONE/.test(out) || !/TRUSTED-TWO/.test(out)) fail.push('a trusted row was dropped');
if (!/no trust field/.test(formatRecall([{ id: 'x', content: 'no trust field' }])))
  fail.push('an absent trust field was withheld (absence must not read as evidence)');
// Assert on the ENTRY line, not the block: the header legitimately contains
// backticks and a bank name, so a whole-block search for `[` would fail on a
// correct render.
const entryOf = (block) => block.split('\n').filter((l) => /^[0-9]+\. /.test(l));
// Starvation: the cap must count EMITTED lines, not scanned input. Five
// untrusted rows ranked above one trusted row must still inject the trusted
// one. This is the bug round 4 found: looping on `i < RECALL_ENTRIES` bounded
// the input scan, so five untrusted hits at ranks 0..4 silently consumed the
// whole budget and the trusted row at rank 5 was never reached. A cap on
// output and a cap on input look identical until the input is adversarial.
const starve = [];
for (let i = 0; i < 5; i++) starve.push({ id: 'u' + i, content: 'UNTRUSTED-' + i, trust: 'untrusted_external' });
starve.push({ id: 't', content: 'TRUSTED-AT-RANK-5', trust: 'user_verified' });
if (!/TRUSTED-AT-RANK-5/.test(formatRecall(starve)))
  fail.push('a trusted row below five untrusted rows was starved (cap counts input, not output)');
// ...and the cap itself must still hold: five trusted rows is five, not six.
const cap = [];
for (let i = 0; i < 7; i++) cap.push({ id: 'c' + i, content: 'TRUSTED-' + i, trust: 'user_verified' });
if (entryOf(formatRecall(cap)).length !== 5)
  fail.push('the entry cap no longer holds at RECALL_ENTRIES');
// Dates, and the degradation guarantee: present when the wire carried one,
// byte-identical to the pre-2026-11-19 rendering when it did not.
if (!/\[2026-11-19\] TRUSTED-ONE/.test(out)) fail.push('a dated row did not render [<YYYY-MM-DD>] content');
// Assert on the ENTRY line, not the block: the header legitimately contains
// backticks and a bank name, so a whole-block search for `[` would fail on a
// correct render.
const undated = entryOf(formatRecall([{ id: 'z', content: 'UNDATED-ROW' }]))[0];
if (undated !== '1. UNDATED-ROW')
  fail.push('a row without created_at did not render byte-identically to before');
const short = entryOf(formatRecall([{ id: 's', content: 'SHORT-STAMP-ROW', created_at: '2026' }]))[0];
if (short !== '1. SHORT-STAMP-ROW')
  fail.push('a stamp shorter than 10 chars rendered a partial bracket');
if (fail.length) { console.error(file + ': ' + fail.join('; ')); process.exit(1); }
console.log('ok  ' + file);
JS
done

# ---------------------------------------------------------------------------
# The export-shape check. Added 2026-11-20 after the opencode door was found
# silently dead on 2.x (CONSISTENCY §62): the plugin content was fine, the file
# was correct for the host it was written against, and the host had changed.
#
# This is deliberately a *shape* check rather than a load test, because a load
# test needs the host binary and this gate has to run where none is installed.
# What it catches is the class that actually bit: an export the host's loader
# will reject. It cannot catch a host that stops calling a hook -- nothing
# static can -- and that limit is stated rather than papered over.
#
# Rule per door, each taken from the host's own documented contract:
#   opencode 2.x  -> default export is an OBJECT with `id` and `setup`
#                    (opencode.ai/docs/build/plugins/migrate-v1)
#   omp / pi      -> default export is a FUNCTION taking the harness object
#
# A bare `export default async function` where an object is required is exactly
# the v1 shape 2.x refuses, so it fails here at commit time instead of silently
# in a user's editor.
if command -v node > /dev/null 2>&1; then
  for spec in \
    "plugin/integrations/opencode-plugin/memory-wire.ts:object:opencode 2.x requires { id, setup }" \
    "plugin/integrations/extension/memory-wire.ts:function:omp/pi requires a factory function"
  do
    file=$(printf '%s' "$spec" | cut -d: -f1)
    want=$(printf '%s' "$spec" | cut -d: -f2)
    why=$(printf '%s' "$spec" | cut -d: -f3-)
    node - "$file" "$want" "$why" <<'JS'
const fs = require('fs');
const [file, want, why] = process.argv.slice(2);
const src = fs.readFileSync(file, 'utf8');
// Anchored to the start of a line on purpose. A doc comment that quotes the
// old v1 shape (`export default async (input) => ({...hooks})`) would otherwise
// match first and report the shape the file *left*, which is how the first
// version of this check failed on a correct file.
const m = src.match(/^export\s+default\s+([^\n;]*)/m);
if (!m) { console.error(file + ': no default export'); process.exit(1); }
const decl = m[1].trim();
const isObject = /^\{/.test(decl);
const looksLikeFn = /^(?:async\s+)?function\b|^(?:async\s*)?\(/.test(decl);
if (want === 'object' && !isObject) {
  console.error(file + ': default export is ' + (looksLikeFn ? 'a function' : JSON.stringify(decl)) +
    ' but ' + why);
  process.exit(1);
}
if (want === 'function' && isObject) {
  console.error(file + ': default export is an object but ' + why);
  process.exit(1);
}
// An object export that forgets `setup` fails the same loader, one step later.
if (want === 'object' && !/\bsetup\s*:/.test(src)) {
  console.error(file + ': object export has no `setup` -- ' + why);
  process.exit(1);
}
console.log('ok  ' + file + ' (export shape: ' + want + ')');
JS
  done
fi

# The opencode prompt hook caches its recall block by query, and the cache write
# has to be conditional on the recall having SUCCEEDED.
#
# A failed `post` yields a non-array, so the block is "" -- and writing that
# under the query meant the next turn with the same prompt hit the
# `query === injectedQuery` short-circuit and injected nothing. A transient
# outage froze recall for that query until the user retyped something.
#
# This is the third bug this file catches that the Rust side cannot: the first
# two were the export shape and the post-slice starvation. All three were found
# by a review or a live probe, not by the code's own tests -- which is the
# argument for checking the shipped text here rather than trusting a unit test
# that exercises a copy of the logic.
if command -v node > /dev/null 2>&1; then
  node plugin/integrations/opencode-plugin/cache_source_check.mjs || exit 1
  node plugin/integrations/opencode-plugin/cache_rule_test.mjs || exit 1

  # The two TypeScript doors carry one policy core: 13 constants and 8 helper
  # functions. This fails when the copies drift apart, which is the failure that
  # produced the round-5 footer bug in Rust and hermes -- a shape ported by hand
  # into a second place, with nothing to notice the copy diverging.
  #
  # It found a real divergence the first time it ran: `textOf` accepted a bare
  # string in the extension door and returned "" for one in opencode. Neither
  # opencode caller can pass a string today, so it was unreachable there, but it
  # is the exact class of drift this exists to catch before it becomes reachable.
  node plugin/integrations/door_core_check.mjs || exit 1
fi

# Every auto-injection door skips a blank-content row before the take, so the
# four of them agree on one policy. This is TODO 37, which the round-5 review
# found by reading all four doors side by side: the two TypeScript doors skipped
# blank rows and Rust and Python counted them against the cap.
#
# The row is not hypothetical. `src/store.rs` has a test that inserts one
# through SQL -- the schema is `content TEXT NOT NULL` with no `CHECK`, so a
# blank row is schema-legal even though every retain path rejects one. Rendered
# naively it is a "- " with nothing after it, and it still counts toward the
# footer's "and N more", so the block claims rows were dropped that were never
# renderable.
#
# The check reads each door's own source for the guard rather than executing it:
# the four are in three languages and two of them need a host runtime to run.
# A source check is weaker than an execution check, so the Rust side carries the
# real test (hooks::tests::a_blank_row_is_skipped_rather_than_rendered_as_an_empty_bullet)
# and this only pins that the policy did not quietly disappear from a door.
printf '%s\n' \
  'plugin/integrations/extension/memory-wire.ts:      !content.trim()' \
  'plugin/integrations/opencode-plugin/memory-wire.ts: !content.trim()' \
  'src/hooks.rs:                                     content.trim().is_empty()' \
  'plugin/integrations/hermes/memory-wire/__init__.py: strip()' \
| while IFS= read -r line; do
    f=${line%%:*}
    needle=$(printf '%s' "${line#*:}" | sed 's/^ *//')
    if ! grep -qF "$needle" "$f"; then
      echo "FAIL  $f does not skip blank content (expected: $needle)" >&2
      exit 1
    fi
  done
echo "ok  every auto-injection door skips a blank-content row"
