# Use an input's indicator feedback

Some DALI-2 switches and sensors provide indicator LEDs or audible feedback.
Support is reported by the device; a button does not automatically have an LED.
Feedback can belong to a whole input module or to one of its instances.

## Protected web interface

1. Sign in, open **controller/Bus1 → Devices** and find the saved input.
2. Choose **⋮ → Feedback** on the module or individual instance row. The bridge
   reads its current feature list, capability and active state. Opening changes
   nothing.
3. If supported, use the compact On/Off toggle. It displays the state verified
   by a fresh hardware read after the command. Reopen or choose **Refresh
   feedback** to read it again.
4. **Close** discards the dialog. A command already sent is not undone by closing.

Unsupported inputs offer no toggle. Missing, collided or incomplete evidence
also keeps the toggle unavailable; Refresh performs a new read. Module feedback
is distinct from every instance's feedback. Indicator changes preserve input
measurements, queued button activity, names, bindings and saved configuration.
Activating feedback does not imply changing any light or assigning button actions.

## MQTT and typed API

Use the ordinary command envelope at `DALI/Command/Controller`, with an MQTT v5
response topic. Use the exact current input target and original `target-generation`
for writes. Module and instance examples:

```json
{"command":"read_input_feedback","target":"Bus1.Inputs.WallPanel"}
```

```json
{"command":"set_input_feedback","target":"Bus1.Inputs.WallPanel.Instances.Door",
 "expected_revision":23,"active":true}
```

Read is Read-class, accepts no parameters and may omit generation. Set is
WithReply and requires the original target generation, current inventory revision
and a boolean active value. Null, numeric/string booleans, unknown fields, output
targets and disabled buses reject. Authenticated HTTP jobs share this parser,
CSRF/session checks and original target authority. Typed callers use
`ServiceHandle::read_input_feedback` / `set_input_feedback`.

Each Set reads fresh support before sending one addressed Activate/Stop and
verifies the requested actual state afterward. Even an equal active value sends
the explicit command: on real devices activation can restart a feedback pattern.
There is no retry, rollback, raw-address job or saved settings shadow.

## Scalar indicator bindings

Publish a JSON boolean to the existing input target's `FeedbackActive` property:

```text
DALI/Desired/Controller/Bus1.Inputs.WallPanel.Instances.Door/FeedbackActive
```

An atomic Desired object at the target topic accepts only `{"FeedbackActive":true}`
or false. Both forms require the exact original MQTT v5 `target-generation`.
Module targets work the same way. Input measurements remain read-only; mixed
indicator/measurement writes reject before native IO.

The target's ordinary State includes independent feedback properties:

| Property | Meaning |
|---|---|
| FeedbackActive | Verified actual boolean, otherwise null |
| FeedbackAvailable | True only with complete supported actual-state evidence |
| FeedbackSupported | Fresh proven support boolean, otherwise null |
| FeedbackCapabilityRaw | Fresh raw capability byte, otherwise null |
| FeedbackQuality | unobserved, current, unsupported, partial, unavailable, type_mismatch or stale |

These fields never replace a button's Status, a sensor's value, Available/Quality
or native action events. Unsupported and unknown do not mean Off. Actual state
is accepted and reported before a scalar write completes; a successful send alone
does not confirm the requested value. An admitted failed read/write retires its
old feedback evidence. Inventory revisions, transport/bus changes and native
address clearing/reassignment also retire affected evidence.

Target `refresh` reads module feedback within eight seconds. For an instance it
first reads the input value (at most four seconds), then feedback (at most eight),
under one twelve-second ceiling. Aggregate refresh keeps its fifteen-second
fair cursor and adds only feedback scopes already sampled by a read or write.
Cold startup does not query every possible indicator.

An instance publishes one merged State object. Its trace identifies the latest
accepted input observation/action or feedback observation. Same-value reads can
update that cause without producing a duplicate MQTT value. Unrelated snapshot
replays preserve it; unsolicited input data clears an older request trace.
Command replies keep their own original request trace.

Reports include operation/outcome, target/generation, revision, bus/address,
optional instance/reported_type, before/current feedback, reason and
configuration_changed=false. A module has instance=null and reported_type=null.
Current feedback contains features (native feature numbers), raw capability,
supported, active and independent complete/partial quality. Unknown evidence is
null. Capability retains unfamiliar bits without inventing their meaning.

The feature list's FE means known none/end; FF means multiple features followed
by an ascending terminated list. Missing or malformed enumeration is null, never
an empty list. A complete list without feature32 is known unsupported. With32,
the bridge reads capability; recognized visible/audible support permits an Active
query. An inactive device returns no backward byte, so closing Type/Status proof
is essential before interpreting silence as false. Collisions and invalid reply
bytes stay unknown. Unknown-only capability bits do not authorize activation.

Read outcomes are current, unsupported, unavailable or type_mismatch. Set reports
set only after complete matching actual evidence; ignored writes or possible
failed sends remain unconfirmed. Known unsupported preflight sends no action.
All work retains the original authority, eight-second maximum caller budget and
physical owner/stamp through final acceptance. Replies describe evidence and do
not replace the authoritative inventory/State stream.

Brightness, colour, timing and audible parameter settings, group feedback and
physical HAT/fixture validation remain separate work. The independent emulator
currently provides an explicit visible on/off profile, not audible output or
physical certification. See [protocol plans](dali-input-contract.md#feature-enumeration-and-basic-feedback-plans),
[emulator profile](emulator.md#visible-feedback-profile) and
[event configuration](input-event-settings.md).
