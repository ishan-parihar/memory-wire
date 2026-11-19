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
