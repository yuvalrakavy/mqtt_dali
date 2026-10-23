# Managed common fixture settings and protected controls

## Problem, reuse and scope

The checked settings foundation at192c7b7 builds native frames and preserves
query values, but an operator cannot yet read or configure those settings.
Add one usable individual-output path through the existing physical owner,
strict generic MQTT commands, authenticated HTTP jobs and the compact fixture
actions dialog. No separate transport, worker, polling loop or configuration
format. No group/broadcast settings UI, timed simulation, extended/DT6 control,
automatic reset/persistent-save, Store or PanelScript work.

Existing `service::input::settings` and feedback demonstrate Read/Configure/
Report, request-owned permit/context/deadline/completion and settled acceptance.
`output::resolve` already validates saved individual routing. `commissioning::Bus`
already executes correlated 16-bit queries/repeated commands with original epoch.
`physical::Work` is the single owner; `group_output::invalidate_actual` and its
mutation predicate already cut bus-local output/colour/group evidence. Browser
`InputSettingsScope` and its dialog demonstrate frozen original authority,
synchronous cancellation and actual readback. Reuse these concrete mechanisms.
Do not introduce a general settings framework for two dissimilar protocols.

Options considered: expose unchecked native frames; add a settings executor/UI
beside the managed owner; extend the established operation/report path. The last
preserves the approved architecture and existing transport ownership. Prior
memory recall found general BridgeKit decisions, no separate output-settings
decision. Current source and the pinned Part102 source govern the contract.

## Typed API and wire contract

Add `service::output::settings` with strict `Patch` (optional u8 fields:
`minimum_level`, `maximum_level`, `power_on_level`, `system_failure_level`,
`fade_time`, `fade_rate`). Reject unknown fields, explicit null, floats, wrong
types, empty maps and statically invalid values before hardware admission.
Limits1..254, startup/failure0..255, basic time0..15 and rate1..15 reuse the
checked codec. Explicit0 and255 retain their context-specific meanings.

Expose typed `read_output_settings` / `configure_output`; normal target commands
have the same names. Read accepts no params. Configure accepts exactly
`{expected_revision, settings}` and needs the original target generation.
HTTP uses the same parser/typed service; generic MQTT needs no Store consumer.
Advertise read as Read-class before requiring CommandCompletion. Both have an
eight-second total ceiling, also clipped to the original caller deadline.

Reports identify operation/outcome, original target/generation/revision/bus/address,
before/after settings, raw correlated readings, optional reason and
`configuration_changed:false`. Settings include physical minimum, min/max,
startup/failure, raw packed fade and separately decoded basic time/rate; unknown
fields are explicit null. Keep raw0 reserved rate visible without inventing a
rate. Diagnostic readings include raw Version, Status and ActualLevel; status
flags are decoded from the existing Status type. No version interpretation,
photometric measurement or completed-fade claim. No settings State cache or
automatic All refresh. Reports remain bounded under the ordinary HTTP allowance.

Probe Status, Version, ActualLevel, PhysicalMinimum, MinimumLevel, MaximumLevel,
PowerOnLevel, SystemFailureLevel, FadeTimeRate and closing Status. Retain silence/
collision direction/code as readings. Valid closing/opening Status proves current
response presence without requiring the status byte to remain equal while a
fixture fades. Numeric settings stay independent of optional diagnostic quality.
Read can return partial fields. Never turn missing data into defaults/Off.

## Preflight, execution and acceptance

Resolve a continuing saved individual output and capture its original revision,
context, physical stamp, permit, deadline and optional completion once. Write
needs the known verified repeat profile. Re-resolve that original authority at
owner admission and final settlement; never retry under a fresh token or epoch.
Strict reads also retain their captured generation/revision across the await.

Before any DTR write, probe fresh hardware and validate the whole patch. Require
valid presence and readable native evidence for each requested setting family.
A reserved rate0 does not block time edits or fixing the rate itself. Unavailable
unrequested diagnostics/settings do not block an otherwise supported patch.
Limit edits additionally require valid current min/max/physical minimum and
`physical <= desired_min <= desired_max <=254`. If desired minimum exceeds
current maximum, write requested maximum first; otherwise minimum first, then
maximum. This covers raising or lowering both limits across the old range.
No startup/failure restriction to current min/max: native0..255 settings are
stored, with actual transitions limited by fixture settings. Do not silently
rewrite another field. Other supplied settings follow in a deterministic order.

Execute complete checked DTR0/setter plans under that same bus permit and HAT
repeat policy. Then re-probe; success requires fresh presence and every requested
field matching actual readback, independent of unrelated missing diagnostics.
Preflight rejection sends no setting frame. After possible mutation, timeout,
collision, stale authority or mismatched readback produces an explicit unconfirmed
report with no claimed old actual values and no blind retry/rollback.

Add concrete Work/Sample/Completion/Message variants and enumerate all owner,
physical, shutdown and proxy branches. Request.started records possible writes.
Conservatively retire bus-local output/colour/group evidence before admission
of configuring work and at settlement, including uncertain exits; read-only work
preserves caches and native input Notices/values. Reuse existing cuts/predicate;
no input-instance settings cuts. Retain the original permit until settled
acceptance/publication and reply, even if the caller goes away.

## Protected browser workflow

Fixture actions gains Settings. It reads on entry, freezing original row, target,
generation, address, bus, inventory revision and view epoch. No mutation on open.
Cancel/Close/navigation synchronously revoke its scope; late replies cannot revive
the form or submit work. A configure failure/unconfirmed response retires the
displayed settings until explicit Refresh; no automatic resend.

Compact form: minimum/maximum brightness in percent (converted to native codes
with exact code roundtrip), physical minimum read-only, power-on Off/Last active/
Level and bus-failure Off/Keep current/Level, basic fade-time dropdown with native
nominal durations and explicit code0 caveat, fade-rate dropdown in steps/second.
Unavailable fields stay unknown and disabled, while readable families remain
usable. Show read-only actual brightness and status/version diagnostics without
forcing navigation. Save submits only changed fields with original authority,
requires matching returned values, then closes; Refresh makes a fresh explicit
read under the still-original scope. Keep row height unchanged.

## Build, evidence and review

1. Fail-first literal native peer tests for exact query/setter plans, both crossing
   limit orders, a later invalid field preventing all writes, physical-minimum
   refusal, independent special255 meanings, reserved packed rate0, silence/
   collisions/mismatch, original token/revision/epoch/deadline, held completion,
   cache cuts versus reads/input Notices. Include normal independent model/PTY
   roundtrip, unchanged inventory and strict MQTT/HTTP advertisement/parameters.
2. Implement owner/proxy/API and preserve old strict peer oracles. New explicit
   operations must not add queries to ordinary All refresh. No model rewrite
   unless a literal test exposes a real missing behavior.
3. Browser unit cases cover strict decoding, changed-field percentages/native
   roundtrip, partial families, original authority and retired read/write replies.
   Actual phone/desktop tests open each menu, read/change/reopen/restore all fields,
   inspect independent physical snapshot and unchanged saved inventory, verify
   Off stays Off and both limit crossing directions, failed replies/refresh and
   canceled/navigation-held reads. Inspect fresh screenshots and overflow/dialogs.
4. Update settings/API/operator/emulator guides, support matrix, CLAUDE, progress
   and wait registry. Measure serial intentional faults for preflight, ordering,
   confirmation, original acceptance/cache cuts and UI authority/conversion.
   Freeze, then sequential owner/native and contract/UI source gates. Full native,
   strict Clippy/Pi, changed formatting, independent waits, Node/build/assets,
   affected controls and actual phone/desktop acceptance. No live hardware or
   release/publication/SDK/memory/skill changes. Same-family review is the available
   limitation.

Two self-review rounds checked existing owner/executor/cancellation branches,
then physical-limit crossing, stored startup/failure versus actual clamping,
reserved rate0, partial-family usability and whole-patch no-write preflight.
Primary sources: [pinned Part102 commands](https://github.com/sde1000/python-dali/blob/4b21c18e79d02d2a97f91d135f0eee450e8c2fb9/dali/gear/general.py)
and [native variable semantics](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/5908382347.html).
One fresh combined design gate precedes implementation. No owner decision is
needed within the already approved settings/UI architecture.
