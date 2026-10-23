# Managed push-button timing

Approved mission: common switches with configuration from the protected bridge
UI. The Part301 codec and independent emulator profile are complete. This T2
slice makes those four settings usable through the existing event-settings
operation and dialog. No user decision is outstanding.

## Shape and evidence

Extend `configure_input.settings` with an optional `button_timers` patch and
`read_input_settings` values with optional button timing evidence. Reuse the
same physical request, authority, original8s budget, admission/settlement Notice
cuts and actual readback. A new parallel command/physical owner would duplicate
the existing lifecycle; a saved settings shadow would replace hardware truth.
The UI keeps one Event settings entry and adds a compact timing fieldset only
for type1 buttons. Other common input settings remain available independently.

Checked code: `service/input/settings.rs` already brackets probes with type,
validates all common settings before writing and returns separately boxed
diagnostics. Its `Patch::plans`, `probe` and `execute` are the extension points.
The browser freezes row/root/epoch, revokes synchronously/on layout cleanup and
compares actual requested fields. Reuse those guards. The literal settings peer
and owned browser flow provide actual entry-point tests.

Protocol/ranges/minimum proof follow the accepted
`2026-10-23-dali-v2-button-timing-foundation.md`; no new wire command is invented.

## API and execution

- Strict optional `settings.button_timers` object contains any nonempty subset
  of `short`, `double`, `repeat`, `stuck`, as native integer bytes. No null
  properties, unknown keys or writable minimums. Reject static invalid ranges,
  non-button targets and an empty nested patch before physical work. Timer-only
  patches are valid. Existing parameters, command classes and authority remain.
- Values add `button_timers`: null for non-buttons; otherwise raw nullable
  `short`, `short_min`, `double`, `double_min`, `repeat`, `stuck` plus its own
  complete/partial quality. Read all six before the closing type proof. Preserve
  FF and missing/null independently. Complete requires valid values/minima and
  current short/double values respecting those minima (double0 allowed).
- Keep common/base quality independent: a partial timer read does not prevent a
  confirmed enable/filter/group-only change or claim complete timer evidence.
  A timer change requires complete timing evidence before any write in a mixed
  patch. Validate every requested short/double against freshly read minima
  before sending even the first common-setting frame. No partial programming
  followed by discovering that another requested timer is invalid.
- Keep existing common plans; append at most four checked timer plans under the
  same bus owner. Every timer plan has one DTR0 and two ordinary24-bit setting
  sends, using the existing 100ms/original-deadline repeat bound. Never renew the
 8s budget. No extra retry, rollback, task, persistent config or snapshot State.
- After programming, probe again. `configured` requires common current quality
  and every requested field (including timers) matching complete actual evidence.
  Unsupported, partial or ignored writes remain unconfirmed. Existing expiry,
  stale stamp/type/authority and uncertainty handling are unchanged. No timer
  result generates events or replaces an input value.

## Browser

Add a two-column timing fieldset to the existing dialog, using native number
inputs: Short press(ms), Double press(ms), Long repeat(ms), Stuck button(s).
Use exact20ms increments for the first three and whole seconds for stuck.
Show device short/double minima, ranges and double0-disabled explanation. Do not
silently round. Edit only with complete timer evidence; otherwise explain timer
settings are unavailable while leaving common controls usable.

Retain the original independently held read values. Compute a nested patch only
for changed timers; convert UI milliseconds exactly to native ticks, reject bad
steps/ranges/minima before submitting and compare returned actual timer values.
Do not accept a declared complete quality without validating values/minima.
Reuse current dirty authority, read/save/cancel/refresh and layout cleanup.
Job replies never replace SSE inventory/current-value authority.

## Execution and verification checklist

1. Fail-first service tests: all four fields via existing typed/MQTT path, rawFF
   and double0, unchanged saved bytes/revision/generation and native values.
   A mixed enable+below-device-minimum patch must send neither change.
2. Implement child timing helper and minimal settings extensions. Update literal
   peer scripts for six button queries before final type; preserve unknown-type
   filter partial/collision tests. Literal cases must prove missing/invalid
   minima, ignored timer writes and actual repeated frame order.
3. Extend current browser scope/unit tests for unit conversion, changed-only
   patch, invalid step/minimum, partial quality, mismatched actual and existing
   frozen lifetime. Real phone/desktop flow sets/reopens/restores all four timers,
   verifies native raw settings and unchanged inventory, scroll/layout and cancel.
4. Mid-checkpoint/self-review, serial intentional faults for minimum preflight,
   missing evidence, actual timer match and client units/match, then source freeze.
   Source-only native/owner and UI/contract review lenses; full native, frontend,
   real browser, scoped lint/Pi/wait/format/asset/control-plan checks.
5. Update operator/MQTT/API and contributor guides, support matrix and progress.
   No global skill/memory/Store/dependency/publication/deployment changes.

## Self-review

Round1 traced current common complete-quality calculation, pre-write plan loop,
strict proxy deserialization and browser accepted/save/changedSettings. Round2
checked disabled-but-present buttons, non-button type refusal, mixed patch
preflight before common writes, partial timer evidence with successful common
operations and final readback minima. This keeps optional timer quality separate
and rejects the whole mixed patch before programming when timer minima fail.

Design budget: one fresh T2 pass after two self-reviews; final source-only two
lenses after mutations. Same-family reviewer limitation remains disclosed.

Design gate APPROVE, no blockers. Requested verification includes double0 with
invalid/missing minimum, a later timer failing after an earlier valid timer, and
forged complete browser evidence with common-only partial-timer usability.

Mid-checkpoint: two native cases first failed on the unsupported nested patch;
two browser-scope tests failed on missing actual-match/minimum guards. All12
focused native and54 frontend cases now pass. Literal peers observe complete
input command sequences to prove no settings were sent on whole-patch preflight
failure. Real phone/desktop acceptance passes894 commands in71.65s, including
four timer fields with exact native values and reopen/restore. Both screenshots
were inspected; scrollable dialog actions remain exercised by actual Save/Cancel.

All seven serial intentional-fault controls produce intended REDs in52.3s. The
browser no-submit test checks command count before error text, isolating preflight
from later value matching. Each production mutation was restored before freeze.

## As-built acceptance

Implemented at `334b264`; design and both final source lenses APPROVE. The full
649-case native suite passes, as do frontend54, Pi, formatting, wait validation,
assets and affected control plans. Strict Clippy exposed a larger physical Work
variant due to expanded diagnostic data. `2dea106` boxes only Request's stored
fallback report and corrects stale scope prose; the native lens approves this
follow-up. All12 affected native cases, strict Clippy and Pi pass after it, with
no wire/control/UI behavior change. Real browser894/71.65s preceded that internal
allocation-only fix. See P4i4 in `../../v2-progress.md` for logs and remaining scope.
