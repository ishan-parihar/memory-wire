# Send note — what the adapter reported vs what happened

**Reported by `send`:** "the message in the thread is not the one we typed
(78 chars, marker=False, typed 97785) — a stale draft was sent; nothing usable
was asked."

**What actually happened (read off page 192 directly):**

```
[{"i":0,"len":91829,"head":"more_vert\nUser 3:59 AM\nReviewer prompt — memory-wire, the shipped architecture, round 2\n\nYou are rev"},
 {"i":1,"len":78,"head":"more_vert\nModel 3:59 AM\nThinking\n\nExpand to view model thoughts\n\nchevron_right"}]
```

The pack **did** post — turn 0 is the 91,829-char user message, opening with the
reviewer prompt's own title. Turn 1 is the model's "Thinking" chrome (78 chars),
which is all the reply says while `xhigh` reasons.

**The defect is in the adapter's readback, not in the send.** `posted_message_readback_js`
takes `us[us.length - 1]` — the LAST turn — and on a page where the model has
already opened its thinking turn, that is the 78-char thinking placeholder, not
the posted user message. The 0.5-floor check then fails on a correctly-sent
pack. The correct selector is the last **user** turn (the one whose innerText
contains the prompt's opening line), or the last turn long enough to be a posted
message.

**Why this matters beyond this round:** the guard is right to exist (a stale
draft is a real failure mode) and wrong in its witness. A guard that cries wolf
on a correct send is the failure the skill's own `aistudio-hydrogen-venue.md`
§8.1 warns about for the run-settings toggle. Recorded here as a tool defect
with the measurement; the round itself is unaffected.

**Provenance of the reply:** the send never reached its archive step, so
`RESPONSE.md` must be captured by hand from the page (three-dots menu → Copy as
markdown), or by `context_pack.py fetch <round> --page 192` once the reply
settles.
