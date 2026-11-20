// The two TypeScript doors carry the same domain core: 13 module-level policy
// constants and 8 helper functions, byte-identical today (measured 2026-11-20,
// `research/audits/context-pack-2026-11-20-rulings/measure_rulings.sh`). A
// change to the recall cap, the tier rule or the clamp shape has to be made in
// both files, and nothing fails if one is missed.
//
// This check is the fence, not the fix. It extracts the shared core from each
// door by name and compares it, so a divergence is a gate failure rather than a
// silent drift discovered by a review months later. It does NOT prove the core
// is correct -- only that the two copies agree, which is the failure this file
// exists to catch.
//
// Why a source check and not an execution check: the two doors run under
// different hosts (omp, opencode) with different plugin APIs, so executing the
// core means standing up two host runtimes. Reading the shipped text costs
// nothing and catches the drift, which is the whole defect.
//
// Run:  node plugin/integrations/door_core_check.mjs
// Exit: 0 if the cores agree, 1 with the differing names if they do not.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const DOORS = [
  "extension/memory-wire.ts",
  "opencode-plugin/memory-wire.ts",
];

// The policy core, by name. Everything else in each door is host glue
// (`lastAssistantText` in extension, `setup` in opencode) and is deliberately
// excluded: it is supposed to differ.
const CONSTANTS = [
  "RECALL_ENTRIES",
  "RECALL_BUDGET",
  "RECALL_TIMEOUT_MS",
  "RETAIN_TIMEOUT_MS",
  "INJECT_CAP_CHARS",
  "ENTRY_CAP_CHARS",
  "RETAIN_PART_CAP_CHARS",
  "SHORT_PROMPT_CHARS",
  "QUERY_CONTEXT_CHARS",
  "MIN_ANSWER_CHARS",
  "EXCLUDE_TAGS",
  "DEFAULT_BANK",
  "UNTRUSTED_TIER",
];
const FUNCTIONS = [
  "endpoint",
  "bank",
  "clamp",
  "hash",
  "stripReminders",
  "textOf",
  "isUntrusted",
  "formatRecall",
];

// A declaration up to the next top-level declaration, comments included. The
// comment is part of the comparison on purpose: a rationale that exists in one
// door and not the other is drift a reader would be misled by.
function extractConstant(src, name) {
  const re = new RegExp(
    `^var ${name} = ([\\s\\S]*?)(?=^var [A-Za-z_$][\\w$]* = |^function |^export |^\\/\\*\\*|^\\/\\/)`,
    "m",
  );
  const m = src.match(re);
  return m ? m[1].trim() : null;
}

function extractFunction(src, name) {
  const re = new RegExp(
    `^(?:export )?(?:async )?function ${name}\\([\\s\\S]*?(?=^(?:export )?(?:async )?function |^var |^\\/\\*\\*|^\\/\\/)`,
    "m",
  );
  const m = src.match(re);
  return m ? m[0].trim() : null;
}

let bad = 0;
const sources = DOORS.map((d) => readFileSync(join(here, d), "utf8"));

for (const name of CONSTANTS) {
  const [a, b] = sources.map((s) => extractConstant(s, name));
  if (a === null || b === null) {
    console.error(`MISSING  ${name} is not declared in both doors (${a === null ? DOORS[0] : DOORS[1]})`);
    bad++;
  } else if (a !== b) {
    console.error(`DIFFER   ${name}\n  ${DOORS[0]}:\n${a}\n  ${DOORS[1]}:\n${b}`);
    bad++;
  }
}

for (const name of FUNCTIONS) {
  const [a, b] = sources.map((s) => extractFunction(s, name));
  if (a === null || b === null) {
    console.error(`MISSING  ${name} is not declared in both doors (${a === null ? DOORS[0] : DOORS[1]})`);
    bad++;
  } else if (a !== b) {
    console.error(`DIFFER   ${name}\n  ${DOORS[0]}:\n${a}\n  ${DOORS[1]}:\n${b}`);
    bad++;
  }
}

if (bad > 0) {
  console.error(`\n${bad} difference(s) between the two TypeScript door cores.`);
  console.error("A change to the shared policy must be made in both, or the doors disagree silently.");
  console.error("If the difference is intended, add the name to the excluded list with the reason.");
  process.exit(1);
}

console.log(`ok  both TypeScript doors carry the same core (${CONSTANTS.length} constants, ${FUNCTIONS.length} functions)`);
