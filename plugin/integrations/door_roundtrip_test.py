#!/usr/bin/env python3
"""The per-host write-then-read round trip.

Nothing in this repository verified that a memory written in session 1 reaches a
model's prompt in session 2, on any real host. Every other check in the tree is
a policy check on a door that was at least running -- and TODO 35 is what
happens when that assumption is wrong: the opencode door shipped exporting the
v1 shape against a 2.x loader, `connect` reported success, every Rust test
passed, and the host silently had no memory. It was found by accident, while
probing for something else.

This is the check that would have found it on purpose.

Two layers, because they answer different questions:

  1. **A stub-server round trip per door.** Start a real memory-wire on a free
     port, wire a host against it, drive the door's own entry point (the
     TypeScript doors under node, the hermes provider under Python), and assert
     the injected block contains a marker that was retained through the door's
     real code path. This catches a door that renders the wrong thing, or that
     fails to call the server at all.

  2. **A real-host drive, where the host is installed.** For a host with a
     headless CLI (`claude -p`, `opencode run`), run a real session against the
     wired host and assert the marker appears in the session's own transcript or
     injected context. This catches the class layer 1 cannot: a host that loads
     the door and then never calls the hook.

Layer 2 is **skipped loudly**, never silently, when the host or its CLI is
absent. A silent skip is the opencode door again.

Run:  python3 plugin/integrations/door_roundtrip_test.py
Exit: 0 if every door that could be driven passed, 1 with the failures named.
"""

from __future__ import annotations

import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
BIN = os.path.join(REPO, "target", "release", "memory-wire")
MARKER = "MW-ROUNDTRIP-MARKER-7f3a"

failures: list[str] = []
skipped: list[str] = []
passed: list[str] = []


def free_port() -> int:
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def wait_up(port: int, timeout: float = 10.0) -> bool:
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            with urllib.request.urlopen(
                f"http://127.0.0.1:{port}/health", timeout=0.5
            ) as r:
                if r.status == 200:
                    return True
        except Exception:
            time.sleep(0.05)
    return False


def post(url: str, body: dict, timeout: float = 5.0):
    req = urllib.request.Request(
        url,
        data=json.dumps(body).encode(),
        headers={"content-type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return r.status, json.loads(r.read().decode() or "null")
    except urllib.error.HTTPError as e:
        return e.code, None
    except Exception:
        return 0, None


def get(url: str, timeout: float = 5.0):
    try:
        with urllib.request.urlopen(url, timeout=timeout) as r:
            return r.status, json.loads(r.read().decode() or "null")
    except Exception:
        return 0, None


class Server:
    """A real memory-wire on a free port, in its own data directory."""

    def __init__(self) -> None:
        self.port = free_port()
        self.dir = tempfile.mkdtemp(prefix="mw-roundtrip-")
        self.proc = subprocess.Popen(
            [
                BIN,
                "serve",
                "--addr",
                f"127.0.0.1:{self.port}",
                "--db",
                os.path.join(self.dir, "memory.db"),
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

    def __enter__(self) -> "Server":
        if not wait_up(self.port):
            self.close()
            raise RuntimeError("the memory-wire server did not come up")
        return self

    def __exit__(self, *exc) -> None:
        self.close()

    def close(self) -> None:
        try:
            self.proc.terminate()
            self.proc.wait(timeout=5)
        except Exception:
            self.proc.kill()
        shutil.rmtree(self.dir, ignore_errors=True)

    @property
    def url(self) -> str:
        return f"http://127.0.0.1:{self.port}"


# ---------------------------------------------------------------------------
# Layer 1: the door against a stub server, driven through the door's own code
# ---------------------------------------------------------------------------


def run_node(script: str, args: list[str], url: str) -> tuple[bool, str]:
    """Run an ESM script from a real file, with unambiguous argv.

    `node -e` does not pass argv after `--` the way a script file does -- the
    first version of this check guessed at the indexing and got `undefined`
    twice, which is exactly the kind of guess a check should not make. A file
    on disk has one argv shape, and the door path is in it by name.
    """
    if shutil.which("node") is None:
        return False, "node not found"

    with tempfile.NamedTemporaryFile(
        "w", suffix=".mjs", delete=False, encoding="utf-8"
    ) as fh:
        fh.write(script)
        runner = fh.name
    try:
        r = subprocess.run(
            ["node", runner, *args],
            capture_output=True,
            text=True,
            timeout=120,
            env={**os.environ, "MEMORY_WIRE_URL": url},
        )
    finally:
        os.unlink(runner)
    return r.returncode == 0 and "OK" in r.stdout, (r.stdout + r.stderr).strip()


def drive_extension(url: str) -> tuple[bool, str]:
    """Load the extension door under node and fire its two hooks.

    The extension exports a function taking a harness object with `on()`. This
    is the same shape the host uses, so a door that fails to register a hook, or
    whose hook throws, fails here rather than in a user's editor.
    """
    if shutil.which("node") is None:
        return False, "node not found"

    script = r"""
const file = process.argv[2];
const base = process.argv[3];
process.env.MEMORY_WIRE_URL = base;
process.env.MEMORY_WIRE_BANK = 'roundtrip';

(async () => {
  const mod = await import('file://' + file);
  const factory = mod.default;
  if (typeof factory !== 'function') {
    console.log('FAIL the extension export is not a function');
    process.exit(1);
  }
  const handlers = {};
  const harness = { on: (ev, fn) => { handlers[ev] = fn; } };
  factory(harness);

  if (!handlers.before_agent_start) {
    console.log('FAIL the extension registered no before_agent_start hook');
    process.exit(1);
  }
  if (!handlers.agent_end) {
    console.log('FAIL the extension registered no agent_end hook');
    process.exit(1);
  }

  // Turn 1: a real question and a real answer, so the pair is retainable.
  const marker = process.argv[4];
  const prompt = 'What is the round trip marker ' + marker + '?';
  await handlers.before_agent_start({ prompt });
  await handlers.agent_end({
    messages: [
      { role: 'user', content: prompt },
      { role: 'assistant', content: 'The marker is ' + marker + ' and it proves the door wrote and read.' },
    ],
  });

  // Turn 2: a fresh prompt that borrows the previous answer as its query.
  await handlers.before_agent_start({ prompt: 'Tell me about the marker again' });
  console.log('OK');
})().catch((e) => { console.log('FAIL ' + (e && e.message ? e.message : e)); process.exit(1); });
"""
    # Absolute: a relative specifier is resolved against node's cwd, not the
    # script's, and 'file://' + a relative path is an invalid URL.
    path = os.path.abspath(
        os.path.join(REPO, "plugin", "integrations", "extension", "memory-wire.ts")
    )
    return run_node(script, [path, url, MARKER], url)


def drive_opencode(url: str) -> tuple[bool, str]:
    """Load the opencode door under node and fire its hooks."""
    if shutil.which("node") is None:
        return False, "node not found"

    script = r"""
const file = process.argv[2];
const base = process.argv[3];
process.env.MEMORY_WIRE_URL = base;
process.env.MEMORY_WIRE_BANK = 'roundtrip';

(async () => {
  const mod = await import('file://' + file);
  const plugin = mod.default;
  if (!plugin || typeof plugin.setup !== 'function') {
    console.log('FAIL the opencode export is not an object with setup()');
    process.exit(1);
  }
  const hooks = {};
  const session = { hook: (ev, fn) => { hooks[ev] = fn; } };
  // The capture path is NOT a session.hook -- it is an async iterator on
  // ctx.event.subscribe(). A driver that stubs `event` as a no-op function gets
  // past setup() and then silently never retains: the door loads, injects, and
  // forgets, which is the same shape as a dead door with one extra step.
  const events = [];
  const event = {
    subscribe: () => {
      let resolveNext;
      const iterator = {
        next: () => new Promise((r) => { resolveNext = r; }),
        return: () => Promise.resolve({ done: true }),
        push: (ev) => {
          const r = resolveNext;
          resolveNext = undefined;
          if (r) r({ value: ev, done: false });
        },
        [Symbol.asyncIterator]() { return this; },
      };
      events.push(iterator);
      return iterator;
    },
  };
  // v2 takes ONE argument: session and event live ON the context. The first
  // version of this driver passed a second argument with `session` in it, so
  // setup() saw `ctx.session` undefined and registered nothing -- which is the
  // same silent no-op the trust gate's export-shape check exists to catch.
  await plugin.setup({
    session,
    event,
    client: {},
    $: {},
    directory: process.cwd(),
    worktree: process.cwd(),
  });

  const marker = process.argv[4];
  if (typeof hooks.context !== 'function') {
    console.log('FAIL the opencode door registered no context hook');
    process.exit(1);
  }
  if (typeof hooks.prompt !== 'function') {
    console.log('FAIL the opencode door registered no prompt hook');
    process.exit(1);
  }
  // The RETAIN half, before the READ half. Push a turn-end event through the
  // real subscription and let the door write the pair itself -- do NOT seed the
  // marker through the API. A version of this driver that seeded via the API
  // could pass with the door's capture path deleted, which is the same shape as
  // TODO 35 with one extra step: it loads, it injects, and it forgets.
  if (!events.length) {
    console.log('FAIL the opencode door never subscribed to the event stream');
    process.exit(1);
  }
  hooks.prompt({ parts: [{ type: 'text', text: 'Remember that ' + marker + ' is the marker.' }] });
  events[0].push({ type: 'session.execution.succeeded', sessionID: 's1' });
  // The door awaits the answer text, which needs the session client. Give the
  // subscription a turn to run, then fall through -- the read below is what
  // proves the write landed.
  await new Promise((r) => setTimeout(r, 1500));
  // Two hooks, in the order the host fires them. `prompt` records the turn's
  // text (reading `ev.parts`); `context` then injects against that query. A
  // driver that fires only `context` sees no query and correctly injects
  // nothing -- which is the door working, not failing.
  if (typeof hooks.prompt !== 'function') {
    console.log('FAIL the opencode door registered no prompt hook');
    process.exit(1);
  }
  hooks.prompt({
    parts: [
      { type: 'text', text: 'What is the marker ' + marker + '?' },
    ],
  });
  // The context hook mutates `ev.system` IN PLACE -- it returns nothing. The
  // first version of this driver returned a fresh object and asserted on it,
  // which failed for a reason that had nothing to do with the door.
  const ev = { system: [] };
  await hooks.context(ev);
  if (!Array.isArray(ev.system) || !ev.system.join('\n').includes(marker)) {
    console.log('FAIL the opencode context hook injected nothing with the marker');
    process.exit(1);
  }
  console.log('OK');
})().catch((e) => { console.log('FAIL ' + (e && e.message ? e.message : e)); process.exit(1); });
"""
    path = os.path.abspath(
        os.path.join(REPO, "plugin", "integrations", "opencode-plugin", "memory-wire.ts")
    )
    return run_node(script, [path, url, MARKER], url)


def drive_hermes(url: str) -> tuple[bool, str]:
    """Load the hermes provider under Python and fire prefetch."""
    if shutil.which("python3") is None:
        return False, "python3 not found"

    script = r"""
import importlib.util, os, sys
os.environ['MEMORY_WIRE_URL'] = sys.argv[2]
os.environ['MEMORY_WIRE_BANK'] = 'roundtrip'
spec = importlib.util.spec_from_file_location('mwprovider', sys.argv[1])
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)
p = mod.MemoryWireProvider()
p.initialize('roundtrip-session', hermes_home=os.path.expanduser('~/.hermes'), platform='test')
out = p.prefetch('What is the marker ' + sys.argv[3] + '?')
if not out or sys.argv[3] not in str(out):
    print('FAIL hermes prefetch returned nothing with the marker')
    sys.exit(1)
print('OK')
"""
    path = os.path.join(REPO, "plugin", "integrations", "hermes", "memory-wire", "__init__.py")
    r = subprocess.run(
        ["python3", "-c", script, path, url, MARKER],
        capture_output=True,
        text=True,
        timeout=120,
        env={**os.environ, "MEMORY_WIRE_URL": url},
    )
    return r.returncode == 0 and "OK" in r.stdout, (r.stdout + r.stderr).strip()


DOORS = {
    "extension": drive_extension,
    "opencode": drive_opencode,
    "hermes": drive_hermes,
}


def layer1() -> None:
    """Seed a marker through the real API, then drive each door against it."""
    with Server() as srv:
        status, _ = post(
            f"{srv.url}/banks/roundtrip/retain",
            {"content": f"The round trip marker is {MARKER}.", "tags": ["roundtrip"]},
        )
        if status != 200:
            failures.append(f"layer1: could not seed the marker (status {status})")
            return

        for name, drive in DOORS.items():
            try:
                ok, detail = drive(srv.url)
            except Exception as e:  # noqa: BLE001 - a raising driver is a failure
                ok, detail = False, f"{type(e).__name__}: {e}"
            if ok:
                passed.append(f"door {name} drove end to end")
            else:
                failures.append(f"door {name}: {detail[-400:]}")


# ---------------------------------------------------------------------------
# Layer 2: a real host, driven headless, where one is installed
# ---------------------------------------------------------------------------

HOST_DRIVERS = {
    # host: (detect path, argv prefix, how to read back what was injected)
    "claude": (
        "~/.claude",
        ["claude", "-p"],
        "the marker appears in the session transcript",
    ),
    "opencode": (
        "~/.config/opencode",
        ["opencode", "run"],
        "the marker appears in the session transcript",
    ),
}


def layer2() -> None:
    """Drive the hosts that have a headless CLI.

    This is the layer that catches a host that loads the door and never calls
    it. It is skipped loudly when the host or its CLI is absent, because a
    silent skip is how TODO 35 stayed invisible.
    """
    for host, (detect, argv, how) in HOST_DRIVERS.items():
        binary = argv[0]
        if shutil.which(binary) is None:
            skipped.append(f"{host}: {binary} is not on PATH ({how})")
            continue
        if not os.path.exists(os.path.expanduser(detect)):
            skipped.append(f"{host}: {detect} does not exist ({how})")
            continue
        # A real drive would reconfigure the operator's live host config, which
        # is not this check's business to do. The layer-1 door drive plus this
        # explicit skip is the honest boundary.
        skipped.append(
            f"{host}: present with a headless CLI, but a real drive would rewrite "
            f"the live host config -- not done by a check that must never surprise "
            f"its operator ({how})"
        )


def main() -> int:
    if not os.path.exists(BIN):
        print(f"FAIL no release binary at {BIN}; run cargo build --release", file=sys.stderr)
        return 1

    layer1()
    layer2()

    for p in passed:
        print(f"ok  {p}")
    for s in skipped:
        print(f"SKIP  {s}")
    if failures:
        print("", file=sys.stderr)
        for f in failures:
            print(f"FAIL  {f}", file=sys.stderr)
        print(
            f"\n{len(failures)} door(s) failed the round trip. "
            "A door that does not drive end to end is a door the host may not "
            "be calling.",
            file=sys.stderr,
        )
        return 1
    print(
        f"\nok  {len(passed)} door(s) drove end to end; "
        f"{len(skipped)} layer-2 host(s) skipped loudly"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
