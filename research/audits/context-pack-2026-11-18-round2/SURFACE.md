# Authored surface — memory-wire at 555d1aa

## Excluded, by name
  target/            build output
  research/          audit rounds (this pack's own directory tree)
  _audit/, _audit_scratch/   read-only competitor checkouts, not our code
  eval/              datasets and generated result artifacts
  genome/, scripts/genome/    vendored third-party CLI
  tests/fixtures/    deliberate secret fixtures

## src/ — lines of Rust per module (the binary)
  34891 total
   5127 src/store.rs
   4106 src/api.rs
   2867 src/main.rs
   2841 src/connect.rs
   2429 src/recall.rs
   1873 src/mcp.rs
   1664 src/hooks.rs
   1132 src/daemon.rs
   1123 src/doctor.rs
   1103 src/connect_plugin.rs
    965 src/vector.rs
    948 src/cli_home.rs
    857 src/connect_codex.rs
    830 src/connect_ext.rs
    810 src/paths.rs
    655 src/render.rs
    649 src/seed.rs
    645 src/hygiene.rs
    613 src/update.rs
    484 src/mcp_http.rs
    355 src/capture.rs
    349 src/grounding.rs
    348 src/connect_opencode.rs
    324 src/connect_claude.rs
    306 src/connect_copilot.rs
    280 src/http.rs
    278 src/skills.rs
    276 src/sweep.rs
    251 src/guidelines.rs
    177 src/embed.rs
    175 src/memory.rs
     51 src/lib.rs

## src/ — total
34891

## tests/ + examples/ — the harness surface
 1307 total
 11648 total

## Dependency count (the default build must not gain one)
17
--- feature-gated (embed, OFF by default) ---
[features]
default = []
# Vendored, offline, dense-vector retrieval. Adds ~23 MB of model weights to the
# binary and the ONNX runtime that runs them; turns on nothing at runtime by
# itself, because `FusionWeights::vector` still ships at 0.0.
#
# OPT-IN AND NOT PART OF THE DEPLOYMENT. The shipped artifact is the default
# build; `install/get-memory-wire.sh` fetches a release tarball and the release
# is cut without this feature, so nothing that ships contains ONNX Runtime or the
# vendored weights. Build it with `cargo build --release --features embed`; it
# needs no network at build time beyond ort-sys's one binary download, and none
# at run time. See `docs/CONSISTENCY.md` §16 for the measurements.
embed = ["dep:fastembed"]

[dev-dependencies]

## Shared libraries the release binary links
 0x0000000000000001 (NEEDED)             Shared library: [libgcc_s.so.1]
 0x0000000000000001 (NEEDED)             Shared library: [libm.so.6]
 0x0000000000000001 (NEEDED)             Shared library: [libc.so.6]

## Binary size
10937128 bytes

## Public MCP tool surface (the count the docs must not misstate)
"memory_retain"
"memory_recall"
"memory_reflect"
"memory_bank_config_get"
"memory_supersede"
"memory_historian"

## Public HTTP routes (the count the docs must not misstate)
"/banks/{b}/config"
"/banks/b/config"
"/banks/{b}/memories"
"/banks/{b}/memories/nope"
"/banks/{b}/recall"
"/banks/b/recall"
"/banks/{b}/reflect"
"/banks/{b}/retain"
"/banks/b/retain"
"/banks/{b}/stats"
"/banks/c/memories"
"/banks/c/recall"
"/banks/c/retain"
"/banks/demo/config"
"/banks/demo/retain"
"/banks/d/recall"
"/banks/d/retain"
"/banks/{exists}/memories"
"/banks/{exists}/memories/{temp}"
"/banks/{exists}/retain"
"/banks/f2/recall"
"/banks/f2/retain"
"/banks/f/recall"
"/banks/f/retain"
"/banks/ghost/config"
"/banks/l/memories"
"/banks/l/memories/does-not-exist"
"/banks/l/memories/{id}"
"/banks/l/memories?limit=1"
"/banks/l/memories?limit=1&offset=1"
"/banks/l/memories?limit=99999"
"/banks/l/retain"
"/banks/l/stats"
"/banks/pb/memories"
"/banks/pb/memories?limit=10000"
"/banks/r/reflect"
"/banks/r/retain"
"/banks/s/recall"
"/banks/s/retain"
"/banks/t/recall"
"/banks/t/retain"
"/banks/u/memories"
"/banks/u/retain"
id/config"
id/hygiene"
id/memories"
id/recall"
id/reflect"
id/retain"
id/stats"
mid"

## Authored doc surface
 12021 total
