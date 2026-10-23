# Managed input event settings

Continue the approved comprehensive input mission after foundation `f71719c`.
T2: new MQTT/HTTP commands, hardware settings and protected browser workflow.
The selected shape is explicit read/patch/readback of hardware-owned settings.
Reject implicit setup during discovery (changes installed behavior) and a new
persisted shadow (duplicates queryable device state). No inventory format edit.

## API and owner

`read_input_settings` is Read-class, accepts no parameters and a saved instance
target. `configure_input` is WithReply with strict `{expected_revision, settings}`.
Both use the original target context. Writes require its exact nonempty generation
and root revision. `settings` is a nonempty patch of optional `enabled` (bool),
`event_priority` (2..5), `event_scheme` (0..4), `event_filter` (known common mask),
`groups` (exactly three null-or-group0..31 entries). Null groups unassign with FF.
Unknown types may use generic Part103 properties but refuse semantic filter writes.
No raw address, module, group, output or controller target. Reject unknown fields,
invalid numbers, empty/null patches and stale authority before physical admission.

Typed `ServiceHandle::read_input_settings`/`configure_input` share one private
settings request path and new `physical::Work::InputSettings`. Pattern follows
input readback and scene/group owned work, without a ConfigStore save phase.
The caller's one original deadline is capped at8s and never renewed; retain permit,
request, context and optional MQTT CommandCompletion through native settlement.
Read dispatch happens before requiring a mutation completion. HTTP reuses parser
and typed calls; it never manufactures an MQTT completion. Trace operation names
remain command names, and free-text reasons use existing HTTP sanitization.

Resolve the saved instance using readback routing plus mandatory write authority.
Capture all routes into the physical plan; no mutable snapshot references during
I/O. No extra UART owner, retries, quiet policies, journal or detached tasks.
The existing physical lifecycle provides cancellation, expiry, stamp checks and
settlement. Add the new variant to every Work/Run/Sample/Completion arm.

At actual write admission and settlement, permanently revoke queued Notices and
cut current observations for the affected input module using the existing
`cut_for_restore` mechanism. Whole-module cut is conservative: changing source
schemes can change ambiguity among sibling instances. Reads never cut events and
never replace current input values or create actions. Reinstall the normal
receive boundary before accepting subsequent activity. Keep both cuts even for
failed/possibly sent writes; unchanged inventory generation cannot authorize old
queued presses. No saved identities or consumer bindings change.

## Physical plan and evidence

Probe Type before any setting, compare to the captured discovered type. A missing,
colliding or changed type refuses writes. Read enabled, priority, scheme, three
groups and filter bytes (low only for common types1..4; all3 for unknown types).
Bracket the read with a matching final Type response. Enabled true requires FF;
false requires **NoResponse**, distinct from receive/transmit collision and an
invalid numeric byte. Without both type proofs enabled remains unknown. Numeric
filter FF is valid, not MASK; missing required bytes never become zero.

For configure, preflight the complete patch before the first write, then build
plans for only requested fields. Use foundation DTR/addressed sequences under
one owner. A `repeat` step means two ordinary24-bit sends whose *combined* budget
is min(original deadline, now+100ms). The ATX repeat flag is16-bit only. Stop on
error; no automatic rollback/retry. Native configuration writes expect no backward
reply; collision cannot prove a send. Read all settings afterward and require
exact patch agreement plus complete valid evidence before saying configured.
There is no power-cycle persistence claim from immediate readback.

Report: operation, outcome, target, target_generation, revision, bus, address,
instance, reported_type, before, settings, reason and configuration_changed=false
(saved bridge inventory was not edited). Values contain nullable enabled/priority/
scheme/filter and three nullable **raw group bytes** (FF means unassigned; null
means unavailable), raw filter bytes and filter_width (8 or24). Quality is
complete/partial; unavailable/type-mismatched devices have no settings. Retain
diagnostic bytes when fields are partially available. Outcomes are
current/configured/unconfirmed/unavailable/type_mismatch. At final owner acceptance,
recheck target/type/generation, original write revision, current physical stamp and
deadline; stale samples cannot report current/configured. Uncertainty after possible
writing returns unconfirmed and no purported current evidence. Before physical
admission failures use existing ServiceError. Never merge report into native State.

## Browser

Add Event settings to each input-instance row's existing menu. Reuse the approved
Modal, compact form and responsive layout. Freeze the original row/token/root and
connection epoch for the dialog. Read on entry; display enable toggle, priority,
scheme, three groups and type-specific event checkboxes. Scheme2 device/instance
is described as uniquely identifying the source; other schemes warn about source
ambiguity. Explicitly describe schemes0/3/4 as currently diagnostic-only in the
bridge; scheme1 routes only a unique saved module/type. Unknown types show raw
filter read-only. No settings are changed on read.
Save submits only changed fields, with frozen authority, then validates command
report and final actual settings. Partial/unavailable data cannot enable Save.
Close/Cancel revokes synchronously; layout cleanup revokes on navigation/unmount.
A late read result cannot submit a write or replace a reopened dialog's authority.
No global snapshot authority comes from job replies. Explicit Refresh can read again
within a continuing dialog; an inventory/root change requires reopening.

## Build and verify

1. Fail-first service tests with private PTY/emulator, typed calls and strict MQTT:
   unchanged saved bytes/tokens, common type matrix, every field, disabled vs absent,
   type mismatch, partial filter/collision, bad/stale input and final stamp refusal.
2. Implement owner/helper/proxy/HTTP; mid-checkpoint and targeted native checks.
3. Add browser scope unit tests (frozen root/token/epoch, late cancellation,
   malformed/unconfirmed result, patch matching) and real phone/desktop flow via
   row menu/read/save/read-again. Use existing browser harness, no live endpoints.
4. Intentional faults for authority, filter/yes-no evidence, patch/readback, Notice
   cuts and browser lifetime. Freeze, source-only protocol/owner and UI/contract
   lenses; full native + frontend + real browser, scoped lint/Pi/wait/dist checks.
5. Update MQTT/API/operator tutorial, input guides, support matrix and progress.
   No skill/global memory/dependency/release change. Timers/feedback remain next.

Bounds: patch at most7 native properties (three groups in one field), at most4
frames/plan, max3 filter bytes, original8s deadline, one existing physical permit.
Any new waits reuse or extend the documented wait registry with an expiry action.

## Self-review evidence

Round1 traced physical Work arms, owner acceptance, readback.resolve, existing
group Read-before-completion and HTTP PreparedCommand. Round2 traced both pending
and settling cancellation, native Notice lifetimes, yes/no collision arms and
unknown-type filters. Correction: groups retain raw FF versus unavailable null;
enabled false requires NoResponse plus before/after type proofs. A read must not
overwrite input value/Event evidence, and configure retains both permanent cuts.
Design gate is one fresh-context pass before code; no new owner decision is needed.

Design gate APPROVE: no blockers, same-family/read-only review. It emphasized
settings-specific yes/no decoding, a separate settings result (not input value
State) and settlement revocation before Notice publication even without a sample.

Implementation mid-checkpoint: common-type PTY patch/readback passes without
saved identity changes. Independent literal tests initially exposed a test-peer
PTY lifetime error: disjoint field capture dropped its slave keeper. Capturing
the whole PTY inside the task fixed the test infrastructure, without a transport
change. Final focused native and browser acceptance cover the actual entry points.

## As-built acceptance

Implemented at `7bab05b`; metadata-only correction `c899f02`. All build/verify
steps above are complete. The design and both final source lenses approve
(same-family/source-only); the native lens also approves the metadata correction.
Eight focused native cases, 49 frontend cases and eleven intended RED controls
pass. Real phone/desktop browser acceptance passes 894 owned commands in 70.74 s,
including actual row-menu settings and late-read dismissal/navigation.

The full suite passed 637 functional cases and initially failed its separate wait
registry test. Correcting two annotations and registering the local wrapper made
that test and standalone validation pass; no runtime change followed the full run.
Scoped strict Clippy, Pi library/four binaries, affected control plans, formatting,
assets and whitespace pass. See the P4i2 entry in `../../v2-progress.md` for logs,
control-fixture correction and remaining scope. Physical hardware, Store and
deployment remain unverified and unchanged.
