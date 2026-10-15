# aroute defects found by memory-wire's LLM baseline — a handoff for the aroute agent

**Audience:** an agent working in `agentic-utility/proxy/artificial-route` (the
`aroute` binary, crate `ar_route`). You do not need memory-wire context. This file
lives here because memory-wire is the consumer that found the bugs; memory-wire
agents do not edit aroute.

**Build observed:** `aroute 0.1.12` (repo HEAD `6014b7a` at the time), running as the
user systemd unit `aroute.service` (`aroute --config ~/.config/ar/config.yaml serve`)
on `127.0.0.1:20128`, no client auth. All requests targeted the `small-stack` combo
(`strategy=priority`: `opencode-zen/space-bunny-free`, then cline / kilocode /
openrouter serving `poolside/laguna-s-2.1:free` and
`dots-studio/dots-3-note-preview:free`). Dates are 2026-10-13.

**Consumer:** an ordinary OpenAI `chat/completions` client: the OpenAI Python SDK
(vectorize-io AMB) and a `reqwest` client (memory-wire's `answer_quality`). Neither
can tolerate B1 or B2, so memory-wire runs `eval/llm_shim.py` between them and aroute
to retry and unwrap. **Done for aroute means that shim can be deleted.**

Ranked by damage to a client.

---

## B1 — an upstream failure is returned as HTTP 200 with an `{"error": …}` body, logged `outcome=ok`, and not failed over

**Observed** (non-streaming request, `small-stack`, three consecutive calls):

```
status 200
x-ar-decision: strategy=priority;outcome=ok;provider=kilocode;attempts=3
x-ar-fallback-attempts: 3
x-ar-latency-ms: 20159
x-ar-tokens-in: 0   x-ar-tokens-out: 0
body: {"id":"gen-1791895015-Qcx0fS5EQQP2cEulB9Mv",
       "error":{"message":"JSON error injected into SSE stream","code":502,
                "metadata":{"error_type":"provider_unavailable"}}}
```

The upstream (kilocode, which relays OpenRouter) began a stream and then put an
error object into it. aroute assembled that into a 200 response, classified the
attempt `ok`, and stopped walking the combo. Later providers in the chain
(openrouter) were never tried.

**Why it matters:** an OpenAI client sees a 200 with no `choices` and either
crashes (`KeyError: 'choices'`) or records an empty answer. A judged benchmark
scores that as a wrong answer, so the provider outage is silently counted against
the system under test.

**Expected:** an error object in the body, whether in a non-stream JSON body or as
an SSE `data:` event, means the attempt **failed**. aroute should classify it with
the same rules as an HTTP status (map `code` 502/503/429 the way the status-code
path already does), apply cooldown, and fall over to the next combo member. If the
whole chain fails, return a **non-2xx** status (502) with the OpenAI error shape
and `outcome=` something other than `ok`.

**Where to look:** search for where `x-ar-decision`'s `outcome=` is computed, and
for the code that aggregates an upstream SSE stream into a non-streaming reply.
The success test there is probably the HTTP status alone. The log line to compare
against is `ar_route::attempt: attempt failed over provider=… status=…`. It fires
for real 4xx/5xx statuses but never fired for this case.

**Repro:** this is intermittent upstream, so test it with a mock upstream instead.
Add a provider whose base URL is a local stub that answers (a) `200` +
`{"error":{"code":502,"message":"x"}}` and (b) `200` + SSE
`data: {"error":{"code":502,…}}\n\n`. Put it first in a priority combo with a
healthy second member. Then assert all three:

- the second member serves the reply;
- `x-ar-decision` shows `attempts=2`;
- the journal shows `attempt failed over` for the stub.

---

## B2 — cline replies are passed through wrapped as `{"success": …, "data": {…}}`

**Observed** (four consecutive calls, 19:56, all served by cline):

```
status 200
x-ar-provider: cline
x-ar-decision: strategy=priority;outcome=ok;provider=cline;attempts=2
top-level keys: ["data", "success"]
body: {"success": true, "data": {"choices":[{"message":{"content":"pong",…},
       "finish_reason":"stop",…}], "model":"poolside/laguna-s-2.1:free",
       "object":"chat.completion","usage":{"prompt_tokens":49,"completion_tokens":3,…},…}}
```

Every cline-served reply has this shape. The cline gateway wraps its
OpenAI-compatible payload in its own envelope, and aroute forwards the envelope
verbatim instead of unwrapping it.

**Why it matters:** every OpenAI client reads `choices` from the top level, so
every cline-served answer looks empty. cline is the provider that actually serves
most `small-stack` traffic (B4 explains why), so this breaks most calls.

**Expected:** cline's response adapter returns the inner object, a bare
`chat.completion`. If the envelope's `success` is `false`, treat it as a failed
attempt (see B1). Check the streaming path too: the stream may carry the same
envelope per chunk.

**Where to look:** the provider wire-dialect / response-normalisation code for
cline (`aroute providers` lists each provider's dialect).

**Repro:** send `{"model":"small-stack","messages":[{"role":"user","content":"Reply with exactly: pong"}],"max_tokens":64}`
until `x-ar-provider: cline` (today that is every call). Check that the body has
top-level `choices` and no `data`.

---

## B3 — token and cost accounting read 0 for every cline-served call

**Observed:** memory-wire's shim logs aroute's `x-ar-tokens-in` / `x-ar-tokens-out`
headers per call. Over the baseline's first 1,509 calls:

| `x-ar-provider` | calls | tokens headers |
|---|---|---|
| cline | 1,453 | `0` / `0` on **every** call |
| kilocode | 56 | real counts on every call (e.g. `225` / `33`) |

The B2 probe above shows the body carrying `usage.prompt_tokens: 49,
completion_tokens: 3` while the headers say `0 0`.

**Likely cause** (inferred, not read from aroute's code): usage is read from the
top-level `usage`, which the B2 envelope hides. `x-ar-response-cost`, per-key spend
ceilings (`aroute limits`) and any dashboard totals inherit the zero. **That means
spend limits are not enforced for cline traffic.**

**Expected:** fixing B2 should fix this. Add a test asserting non-zero
`x-ar-tokens-*` for a cline-dialect reply that carries `usage`.

---

## B4 — a provider failing with a permanent 403 gets no cooldown and is retried on every request

**Observed** in the journal (`journalctl --user -u aroute --since today`), counted
by the `attempt failed over` line:

```
1658  provider=opencode-zen status=403 cooldown_ms=0
  58  provider=kilocode     status=429 cooldown_ms=3000
  14  provider=cline        status=500 cooldown_ms=3000
   4  provider=kilocode     status=429 cooldown_ms=6000 / 12000 / 24000 / 48000
```

`opencode-zen` sits first in `small-stack`. It answers 403 to every request and
has served 0 of the baseline's 1,509 calls. Because the cooldown is 0, every
request pays one wasted round trip to it first. That is why the B2 probe shows
`attempts=2` on every call.

**Two separate questions:**

1. **Why 403?** Probably the operator's credential or account for opencode-zen
   (an expired key, a free tier withdrawn, a model renamed). That is config, not
   code. Check it with `aroute providers` and the provider's own status. It does
   not belong to memory-wire, and memory-wire agents may not inspect the key
   store.
2. **Why `cooldown_ms=0`?** That one is code. 429 and 500 back off
   (3,000 ms, then doubling), but 401/403 get none, although an auth failure is
   the least likely error to fix itself within a request. **Expected:** 401/403
   put the provider into a long cooldown (minutes) or mark it unhealthy until
   config reload. Either way it should surface in `aroute combo` / health output
   so the operator learns the first link of the chain is dead.

---

**Update, 23:12 the same day.** The same process (pid unchanged, still 0.1.12) has
not attempted `opencode-zen` since `21:05:55`, and calls now show `attempts=1`.
Between 21:00 and 21:05 its 403s were mixed with `transport failure, failing over
provider=opencode-zen`. A network-wide outage on this host (21:00–22:10) hit cline,
kilocode and openrouter with transport failures too, so the outage itself is not
aroute's fault. What stopped the retries is unknown from the outside. A plausible
reading is that the transport-failure path opens a breaker and the 403 path does not;
that is a hypothesis to check in `ar_route::attempt`. If so, the fix for B4 is to give
401/403 the same treatment.

## Not bugs: limitations memory-wire works around and does not ask you to change

- **Only combo names are routable.** A `provider/model` id gets `400
  model_not_found` ("is not a configured combo; routable models are: …"). This is
  by design, but it means a client cannot pin the serving model. If aroute ever
  offers a pinned-model route, memory-wire's baselines could fix the reader model.
  That is a feature request, not a defect.
- **The free models ignore `response_format: json_schema`** (asked for
  `{answer, reasoning}`, one replied `{"capital":"Paris"}`). This is model
  behaviour; the shim restates the schema in a system message.
- **`dots-3-note-preview` is a reasoning model** and returns `content: null` when
  `max_tokens` is small, because the budget goes to reasoning. Also model
  behaviour.

## How memory-wire will verify a fix

`eval/llm_shim.py` logs every call to `llm-calls.jsonl`:

- `attempts` above 1 means it had to retry. B1 shows up this way.
- `enveloped: true` means it had to unwrap. B2 shows up this way.
- `tokens_in`/`tokens_out` come from your headers. B3 shows up this way.

After an aroute fix, a memory-wire agent runs a short LLM job through the shim. If
the log shows no `enveloped`, no retries of 200-with-error, and non-zero tokens for
cline, the shim gets deleted (its `ponytail:` note says so) and harnesses point
straight at `:20128`. Restarting `aroute.service` with a new build is the
operator's call.
