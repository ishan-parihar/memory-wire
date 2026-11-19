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
  { id: 'a', score: 1, content: 'TRUSTED-ONE', trust: 'user_verified' },
  { id: 'b', score: 2, content: 'LEAK-B', trust: 'untrusted_external' },
  { id: 'c', score: 3, content: 'TRUSTED-TWO', trust: 'agent_inferred' },
  { id: 'd', score: 4, content: 'LEAK-D', trust: 'untrusted_external' },
];
const out = formatRecall(items);
const fail = [];
if (/LEAK-/.test(out)) fail.push('untrusted_external reached the block');
if (!/TRUSTED-ONE/.test(out) || !/TRUSTED-TWO/.test(out)) fail.push('a trusted row was dropped');
if (!/no trust field/.test(formatRecall([{ id: 'x', content: 'no trust field' }])))
  fail.push('an absent trust field was withheld (absence must not read as evidence)');
if (fail.length) { console.error(file + ': ' + fail.join('; ')); process.exit(1); }
console.log('ok  ' + file);
JS
done
