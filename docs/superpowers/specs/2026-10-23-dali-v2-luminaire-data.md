# Read-only luminaire data (Part251)

Tier T2: a new standard-data decoder and public read report, reusing the accepted
memory owner. Implements approved architecture §8's DALI Data family incrementally;
Parts252/253 remain separate following slices because dynamic banks need their
own scaling/status and snapshot semantics. The owner authorized continuous
implementation of the approved architecture without routine approval requests.
No new product decision, Store/schema work, v1 change or hardware write is needed.

## Purpose, grounding and alternatives

Device identity reports describe the driver. Operators also need the luminaire's
OEM identity, ratings and descriptive text. Current `dali::memory` reads exact raw
banks, `service::memory` owns one bounded physical operation and prepared commands
already carry their original revision. The emulator can represent any bank using
optional independent raw profiles; its scenario editor already configures them.
There is no typed Part251 decoder/report in current source. Memory recall found
no earlier Part251 decision; BridgeKit topology/generic-consumer rules still apply.

Choose a pure decoder plus one `Action::Luminaire` in that existing Memory Work.
Reject a separate worker/cache/poller, unsafe manufacturer writes, inferred names
or a dependency on Store. Read reports are explicit captures like identity.
Do not bundle dynamic energy/diagnostic snapshot writes into this static-data slice.
Reuse existing scratch DTR serialization, trace and final acceptance.

## Protocol and report

Add `read_luminaire_data`, no parameters, individual saved output target only.
Input modules, instances, groups and scenes reject. Read class, no mutation
completion or expected-revision payload; supplied generation must match. Parse
pins the original inventory revision for queued MQTT/HTTP commands. Direct typed
submission captures once before its first await. Keep the15s absolute cap and
existing owner/epoch/reply checks. No retry, setting write or automatic read.

Bank1 header location0 limits reads. Load DTR1 once and DTR0 for every byte, using
the existing literal C3/A3/addressed-C5 plans. Read header, then locations through
min(last,0x12) to observe the content-format ID at0x11..0x12. Only format0x0003
selects the supported Part251 v1.1 layout through0x77. For another/missing format,
preserve raw prefix and explicitly report unrecognized/unavailable format; do
not decode extension fields or guess a legacy layout. A silent/reservedFF header
keeps the same unavailable/partial diagnostic behavior as identity.

After positive format proof, extend the same owned capture through min(last,0x77).
This is a bounded120-byte internal plan; the public raw `read_memory` limit stays
64 bytes. Do not split ownership into separate service calls. Raw byte1 has no
assumed checksum; byte2 is captured only, never unlocked or written. Missing and
collided bytes retain their location; FF is interpreted only by a specific field.

Extend the common report with an optional `luminaire` object (omitted on existing
identity/raw reads). Keep operation/outcome/route/revision/bank/range/quality/raw
readings/configuration_changed:false. Luminaire is `{format_id:u16|null,
format_status:supported|unrecognized|unavailable,fields:{<key>:Field}}`, where Field
is `{status:value|unknown|unavailable|invalid,value:number|string|null,unit:string|null}`.
Only status=value has a nonnull value. All supported-format field keys are present;
unknown/unavailable formats have no interpreted field map. Raw evidence remains
inspectable. CCT's FFFE uses text value `part_209` with no unit, not a temperature.
Distribution values use text `not_specified`/`type_i`..`type_v`; reserved06..FE
are invalid interpretation with original raw evidence retained. FF alone is the
unknown mask, not a reserved distribution code.
Quality complete means all requested bytes answered, not all OEM data populated.
Unknown format or truncated supported layout is partial. No bank response is
unavailable, never a positive unsupported-device claim.

The supported format's field map, grounded in the primary reference:

| Field | Locations | Interpretation |
|---|---|---|
| OEM GTIN |03..08|six-byte unsigned, padded14-digit decimal string when valid|
| Luminaire ID |09..10|exact16-character uppercase hex, not a JSON number|
| Content format |11..12|0003 selects this layout|
| Manufacture year/week |13/14|year00..99 without an invented century; week1..53|
| Nominal/minimum-dim input power |15..16/17..18|unsigned16-bit watts; FFFF unknown|
| Nominal AC voltage minimum/maximum |19..1A/1B..1C|90..480 volts|
| Nominal light output |1D..1F|unsigned24-bit lumens; FFFFFF unknown|
| CRI |20|0..100|
| CCT |21..22|0..17000 kelvin; FFFE means Part209 indication, not65534K|
| Distribution |23|0 unspecified;1..5 named types; reserved codes stay explicit|
| OEM color text |24..3B|up to24 ASCII bytes, independent of emitted-light color|
| Identification text |3C..77|up to60 ASCII bytes|

All-FF numeric/identity fields have unknown interpretation, not synthetic maxima.
Missing/colliding bytes prevent that field's value. Standard-defined unknown
ranges (including year/week/CRI/voltage) stay unknown; malformed encoding is
invalid. Never clip or substitute numeric defaults. Parse text up to its first
NUL; unavailable bytes before termination cannot be silently skipped. Preserve
empty/unspecified text distinctly from a fabricated name. Render text safely;
do not interpret markup, fill Description or rename any bridge/Store item.
Do not infer usable DT8 capabilities from the CCT indication alone.

## Operator UI and emulator

Add **Read luminaire data** within the existing output Device information dialog.
It replaces the current capture with a read-only OEM/ratings/text result; identity
and raw reads remain available. Reuse frozen target/generation/revision/epoch and
synchronous close/navigation cancellation. Clear previous results before reads
and on failure. Show field units and unknown/invalid/unavailable distinctions;
label readings as captures. Advanced raw table remains collapsible and scrollable.
The action is absent for input modules. Phone and desktop use the same Modal.

Use existing optional emulator bank1 arrays, with no new model or UI state.
Add a ready-to-use scenario containing modern bank0 and populated bank1 plus
raw examples for unknown/invalid fields. Scenario import/capture/reboot/reset
preserve the profile. The native model must continue to decode literal memory
commands independently of the bridge decoder. No manufacturer programming UI.

## Implementation and verification phases

1. Pure field/status decoder and fixed producer-shaped bank fixtures. Fail-first
   for exact64-bit IDs, masked/ranged numbers, variable-CCT indication, reserved
   distribution0/5/6/254/255, NUL/ASCII/empty text, holes and short/unknown format. Confirm the
   primary field map before coding. No semantic dependence on emulator IDs.
2. Extend existing Memory Action/report/execute, shared strict parser/commands and
   typed API. Preserve original revision at preparation through final reply.
   Literal UART and independent TCP/MQTT cases prove bank1 selection, nonzero
   offsets, format/header bounds, no writes, target refusal and unchanged native
   levels/colors/settings/groups/scenes/input data/config/caches. No64-byte raw
   API regression. Mid-checkpoint/self-review before UI wiring.
3. Extend protected DeviceInfo scope/decoder/dialog with focused authority/status
   tests and real phone/desktop Chrome. Read/reopen/unknown-format/ranges/text,
   input-module absence, raw-table scrolling and retired dialogs. Restore the
   caller's viewport before returning from a composed browser flow.
4. Documentation/skills phase: operator+MQTT/API guide and scenario, CLAUDE/service
   pointers, support matrix and live tracker. No Store/user skill or memory writes.
5. Meaningful negative controls, isolated mutation finder first, frozen fresh
   source gate and full native/frontend/browser/Clippy/Pi/wait/format/assets checks.
   No physical certification claim; continue Parts252/253 and other bridge rows.

Self-review: one existing owner and read-only scratch plans; no new persistence,
identity, runtime task, discovery behavior, dependency or recovery policy. Strict
known-format decoding bounds the claim. Generic MQTT consumer and independent
emulator remain first-class. User architecture §8, ways-of-working T2 and memory
notes on generic-framework support/topology are applied in these sections.

Primary source, verified2026-10-23:
[python-dali Part251 definitions](https://github.com/sde1000/python-dali/blob/master/dali/memory/oem.py),
[memory value mechanics](https://github.com/sde1000/python-dali/blob/master/dali/memory/location.py).
