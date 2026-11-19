#!/usr/bin/env python3
# The hermes plugin carries the same trust gate as `src/hooks.rs` and the two
# TypeScript plugins, and it was the one door with the policy and no executable
# check. This exercises the real `MemoryWireProvider.prefetch` against a local
# HTTP stub -- no hermes runtime, no network -- and asserts both directions:
#
#   * `untrusted_external` never reaches the injected block
#   * a row with no `trust` field is KEPT (absence is not evidence)
#   * a row below the cap that is dropped does not cost a trusted row its slot
#
# Run:  python3 plugin/integrations/hermes/trust_gate_test.py
import importlib.util
import json
import os
import sys
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer

PLUGIN = os.path.join(
    os.path.dirname(os.path.abspath(__file__)), "memory-wire", "__init__.py"
)

# What the stub server will answer to /banks/<bank>/recall.
HITS = [
    {"id": "a", "content": "TRUSTED-ONE", "score": 0.9, "trust": "user_verified"},
    {"id": "b", "content": "LEAK-B", "score": 0.8, "trust": "untrusted_external"},
    {"id": "c", "content": "TRUSTED-TWO", "score": 0.7, "trust": "agent_inferred"},
    {"id": "d", "content": "LEAK-D", "score": 0.6, "trust": "untrusted_external"},
]
NO_TRUST = [{"id": "x", "content": "NO-TRUST-FIELD", "score": 0.5}]


class Handler(BaseHTTPRequestHandler):
    payload = HITS

    def do_POST(self):  # noqa: N802 - BaseHTTPRequestHandler's name
        length = int(self.headers.get("content-length") or 0)
        self.rfile.read(length)
        body = json.dumps(self.payload).encode()
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


def load_plugin():
    spec = importlib.util.spec_from_file_location("mw_plugin_under_test", PLUGIN)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def main():
    srv = HTTPServer(("127.0.0.1", 0), Handler)
    port = srv.server_address[1]
    threading.Thread(target=srv.serve_forever, daemon=True).start()

    os.environ["MEMORY_WIRE_URL"] = f"http://127.0.0.1:{port}"
    mod = load_plugin()
    provider = mod.MemoryWireProvider()
    # `prefetch` reads `self._base`, which `initialize` binds. Calling it is not
    # a formality: a provider used without it would answer "" for every query,
    # and a test that skipped it would pass while proving nothing.
    provider.initialize("test-session")

    fail = []

    out = provider.prefetch("anything", session_id="test")
    if "LEAK-" in out:
        fail.append("untrusted_external reached the injected block")
    if "TRUSTED-ONE" not in out or "TRUSTED-TWO" not in out:
        fail.append("a trusted row was dropped")

    Handler.payload = NO_TRUST
    kept = provider.prefetch("anything", session_id="test")
    if "NO-TRUST-FIELD" not in kept:
        fail.append("an absent trust field was withheld (absence must not read as evidence)")

    srv.shutdown()
    if fail:
        print("FAIL plugin/integrations/hermes/memory-wire/__init__.py")
        for f in fail:
            print("  -", f)
        return 1
    print("ok  plugin/integrations/hermes/memory-wire/__init__.py")
    return 0


if __name__ == "__main__":
    sys.exit(main())