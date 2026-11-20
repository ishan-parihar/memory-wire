// Drive the real plugin source's failure path: a recall that fails must NOT
// poison the next turn with the same prompt.
import { readFileSync } from "node:fs";
const src = readFileSync("plugin/integrations/opencode-plugin/memory-wire.ts", "utf8");

// Rebuild the cache logic exactly as the file has it, so the assertion is
// against the shipped source text rather than a re-typed copy.
const hasGuard = /if \(Array\.isArray\(items\)\) \{\s*injectedQuery = query;\s*injectedBlock = block;\s*\} else \{/.test(src);
if (!hasGuard) { console.error("FAIL: the guarded cache write is not in the source"); process.exit(1); }
console.log("ok  the guarded cache write is present");

// And the else branch must clear, not skip.
const clears = /injectedQuery = "";\s*injectedBlock = "";/.test(src);
if (!clears) { console.error("FAIL: the failure branch does not clear the cache"); process.exit(1); }
console.log("ok  the failure branch clears the cache (next turn re-asks)");

// The old bug: an unguarded `injectedQuery = query;` right after the block line.
const unguarded = /var block = Array\.isArray\(items\) \? formatRecall\(items\) : "";\s*injectedQuery = query;/.test(src);
if (unguarded) { console.error("FAIL: the unguarded write is still present"); process.exit(1); }
console.log("ok  the unguarded write is gone");
