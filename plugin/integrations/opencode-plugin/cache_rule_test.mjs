// Extract the cache rule into a runnable form and prove the two behaviours.
// This mirrors the source's structure; the source-level check above pins that
// the mirror matches the file.
function makePlan() {
  let injectedQuery = "", injectedBlock = "";
  return {
    // returns the block the prompt hook would push
    turn(query, items) {
      if (query === injectedQuery) return { cached: true, block: injectedBlock };
      const block = Array.isArray(items) ? ["formatted", items] : "";
      if (Array.isArray(items)) { injectedQuery = query; injectedBlock = block; }
      else { injectedQuery = ""; injectedBlock = ""; }
      return { cached: false, block };
    }
  };
}

// 1. Failure then recovery, same prompt.
const p = makePlan();
const t1 = p.turn("hello", null);              // server down
if (t1.cached !== false) throw new Error("first turn must not be cached");
const t2 = p.turn("hello", ["a"]);             // server back, SAME prompt
if (t2.cached === true) throw new Error("BUG: second turn reused the failed cache");
console.log("ok  a failed recall does not poison the next turn with the same prompt");

// 2. Success is still cached (the point of the cache).
const p2 = makePlan();
const s1 = p2.turn("q", ["a"]);
const s2 = p2.turn("q", ["a"]);
if (s2.cached !== true) throw new Error("a successful recall should still be cached");
console.log("ok  a successful recall is still cached");

// 3. A different prompt always re-asks.
const p3 = makePlan();
p3.turn("q1", ["a"]);
const d = p3.turn("q2", ["a"]);
if (d.cached === true) throw new Error("a new prompt must re-ask");
console.log("ok  a different prompt re-asks");
