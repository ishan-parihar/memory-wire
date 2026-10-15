#!/usr/bin/env python3
"""OpenAI-compatible pass-through for the LLM-judged harnesses, with a call log.

    python3 eval/llm_shim.py --upstream http://127.0.0.1:20128 --port 20199 --log calls.jsonl

Point `MEMORY_WIRE_LLM_URL` / `OPENAI_BASE_URL` at `http://127.0.0.1:20199/v1`.

Why it exists, both measured 2026-10-13 against aroute 0.1.12's `small-stack` combo:
  * a mid-stream provider failure comes back as HTTP 200 with an `{"error": ...}`
    body (aroute logs `outcome=ok` and stops falling back), and
  * some providers' replies arrive wrapped as `{"data": {"choices": ...}}`.
Either breaks every OpenAI client. This shim retries the first and unwraps the
second. The fix belongs in aroute; docs/AROUTE_BUGS.md is the handoff. A third is the models', not aroute's: they
ignore `response_format: json_schema`, so the shim states the schema in a system
message and retries a reply that misses a required key (`state_schema`), and it
defaults `temperature` to 0 when the caller sends none.
ponytail: retry-and-unwrap here; delete this file once aroute does both itself.

It also writes one JSONL line per call — which model and provider actually served
it, after how many retries — because a combo is a fallback chain and the served
model is otherwise invisible to the harness. A judged number is only
interpretable next to that log.
"""
import argparse
import json
import sys
import threading
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

RETRYABLE = {408, 409, 425, 429, 500, 502, 503, 504}


def schema_json(content, required):
    """The reply as a bare JSON object string if it parses and has every required key."""
    text = content.strip()
    if text.startswith("```"):
        text = text.split("\n", 1)[-1].rsplit("```", 1)[0]
    start, end = text.find("{"), text.rfind("}")
    try:
        obj = json.loads(text[start:end + 1])
    except ValueError:
        return None
    return json.dumps(obj) if isinstance(obj, dict) and all(k in obj for k in required) else None


def state_schema(body):
    """The free models ignore `response_format: json_schema` (they answered a
    {answer, reasoning} request with {"capital": "Paris"}), so say it in the prompt.
    Returns (body, required keys or None)."""
    try:
        req = json.loads(body)
    except ValueError:
        return body, None
    # AMB's OpenAI client sends no temperature, so the free models sampled at their
    # default and the same context answered differently twice (2026-10-13 replay).
    # A baseline instrument has to be stable; an explicit caller value still wins.
    req.setdefault("temperature", 0)
    body = json.dumps(req).encode()
    rf = req.get("response_format") or {}
    schema = (rf.get("json_schema") or {}).get("schema") if rf.get("type") == "json_schema" else None
    if not schema:
        return body, None
    req["messages"] = [{"role": "system", "content":
                        "Reply with only a JSON object, no prose and no code fence, matching "
                        "this JSON Schema exactly (every required key present):\n"
                        + json.dumps(schema)}] + req.get("messages", [])
    return json.dumps(req).encode(), schema.get("required", [])


def call_upstream(url, body, headers, attempts, timeout, deadline):
    """Return (status, payload_bytes, meta). Retries transient failures with backoff,
    but never past `deadline` seconds in total: the OpenAI SDK gives up at 600 s and
    raises a timeout that AMB does not retry, which killed a whole run on 2026-10-13.
    A 502 inside the deadline is retried by AMB's own backoff instead."""
    delay, last, t0 = 5.0, None, time.monotonic()
    body, required = state_schema(body)
    for attempt in range(1, attempts + 1):
        left = deadline - (time.monotonic() - t0)
        if left < 5:
            attempts = attempt - 1
            break
        req = urllib.request.Request(url, data=body, headers=headers, method="POST")
        try:
            with urllib.request.urlopen(req, timeout=min(timeout, left)) as r:
                raw, hdrs = r.read(), r.headers
            obj = json.loads(raw)
            enveloped = isinstance(obj.get("data"), dict) and "choices" not in obj
            if enveloped:
                obj = obj["data"]
            content = ((obj.get("choices") or [{}])[0].get("message") or {}).get("content")
            fixed = schema_json(content, required) if content and required is not None else content
            if "error" in obj or not obj.get("choices") or not content:
                last = f"200 with no answer: {str(obj.get('error') or obj)[:160]}"
            elif fixed is None:
                last = f"reply misses required keys {required}: {content[:120]!r}"
            else:
                obj["choices"][0]["message"]["content"] = fixed
                return 200, json.dumps(obj).encode(), {
                    "attempts": attempt,
                    "served_model": obj.get("model"),
                    "provider": hdrs.get("x-ar-provider"),
                    "tokens_in": hdrs.get("x-ar-tokens-in"),
                    "tokens_out": hdrs.get("x-ar-tokens-out"),
                    "schema_stated": required is not None,
                    "enveloped": enveloped,  # docs/AROUTE_BUGS.md B2
                }
        except urllib.error.HTTPError as e:
            last = f"HTTP {e.code}: {e.read()[:160]!r}"
            if e.code not in RETRYABLE:
                return e.code, json.dumps({"error": {"message": last}}).encode(), {"attempts": attempt}
        except Exception as e:  # timeouts, resets, bad JSON: all transient here
            last = repr(e)[:200]
        if attempt < attempts:
            time.sleep(min(delay, max(0.0, deadline - (time.monotonic() - t0))))
            delay = min(delay * 2, 60.0)
    return 502, json.dumps({"error": {"message": f"shim gave up: {last}"}}).encode(), {
        "attempts": attempts, "error": last}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--upstream", default="http://127.0.0.1:20128")
    ap.add_argument("--port", type=int, default=20199)
    ap.add_argument("--log", required=True)
    ap.add_argument("--attempts", type=int, default=8)
    ap.add_argument("--timeout", type=float, default=180)
    ap.add_argument("--deadline", type=float, default=480,
                    help="total seconds per call, retries included; below the SDK's 600")
    a = ap.parse_args()
    lock = threading.Lock()

    class H(BaseHTTPRequestHandler):
        def do_POST(self):
            body = self.rfile.read(int(self.headers.get("content-length", 0)))
            headers = {"content-type": "application/json"}
            if self.headers.get("authorization"):
                headers["authorization"] = self.headers["authorization"]
            t = time.time()
            status, payload, meta = call_upstream(
                a.upstream + self.path, body, headers, a.attempts, a.timeout, a.deadline)
            try:
                req_model = json.loads(body).get("model")
            except ValueError:
                req_model = None
            meta.update(ts=round(t, 3), ms=int((time.time() - t) * 1000),
                        status=status, path=self.path, requested=req_model)
            with lock, open(a.log, "a") as f:
                f.write(json.dumps(meta) + "\n")
            self.send_response(status)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

        def log_message(self, format, *args):  # noqa: A002 — base-class signature
            pass

    print(f"llm_shim: 127.0.0.1:{a.port} -> {a.upstream}, log {a.log}", file=sys.stderr)
    ThreadingHTTPServer(("127.0.0.1", a.port), H).serve_forever()


if __name__ == "__main__":
    main()
