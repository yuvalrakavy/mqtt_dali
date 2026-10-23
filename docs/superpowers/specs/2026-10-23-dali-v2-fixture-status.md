# Fixture status in MQTT, compact fault UI and emulator lamp failures

Status: implementing the explicitly approved feature (23 October2026). TierT2:
additive MQTT state fields and optional emulator scenario/control fields. Existing
output owner, physical transactions, polling, HTTP snapshots and Preact UI remain.
No v1 topics, Store schema, UART recovery or new polling process.

## Approved behavior and contracts

Every individual output State adds read-only `FixtureStatusRaw` (0..255 or null),
`FixtureStatusQuality` (`unobserved`, `current`, `unavailable`, `stale`) and nullable
booleans `GearFailure`, `LampFailure`, `LampOn`, `LimitError`, `FadeRunning`,
`ResetState`, `MissingShortAddress`, `PowerFailure`. Decode the existing Part102
Status methods; FF is a valid status byte with all bits set, never MASK. Only
matching-generation observations under existing lifecycle/quality fences qualify.
A valid status byte is independently current even if actual-level readback is
missing/MASK. Missing/no-response/colliding status is unavailable; no observation
is unobserved; lifecycle-invalid or explicitly stale observations are stale.
All raw/boolean fields are explicit null whenever status is not current, clearing
prior merged MQTT values. No fault implies communication unavailability: a lamp
failure may coexist with Available=true and unchanged PowerState/Brightness.
Status bits are read-only; ordinary strict Desired validation continues rejecting
them. No new aggregate group status or separate event/publisher/cache/trace cause.

Reuse proxy/state.rs projections and the existing output Observation/context.
QUERY STATUS already accompanies output reads and writes; this adds no UART
traffic or timer. Generic MQTT consumers observe changes on normal reads/refresh/
periodic polling. No light-output level or physical illumination is inferred from
a fault bit; these are the driver's reported flags.

Output rows put one compact icon next to the inline name: red for GearFailure or
LampFailure, amber for LimitError/MissingShortAddress/PowerFailure. LampOn,
FadeRunning and ResetState are informational and do not make a fault icon.
Current all-clear rows need no icon; unobserved/unavailable/stale status has a
neutral unknown icon. Accessible button/title names the fixture and reported
faults. Clicking opens the existing modal with status quality, decoded flags,
raw byte and explicit Read current values. The modal follows current SSE State;
job replies never replace it. No new write operation or draft/cancellation layer.
Keep the existing table columns and name editing behavior on phone and desktop.
Input modules/instances do not acquire invented lamp-failure flags.

The independent emulator adds optional scenario output `lamp_failure: bool`,
default false and omitted when false. Old version1 files keep their behavior.
Control API adds strict `{op:"set_lamp_failure",id,failed:bool}` using the stable
native output ID; unknown/input IDs fail without changes. The model's QUERY
STATUS ORs literal bit0x02 with its existing computed lamp-on bit. Its native
level, color, groups, scenes, address and other devices remain unchanged. It does
not import the bridge codec and does not publish MQTT or contact the bridge.
This is reported fault simulation, not an optical/electrical physics model.
HAT reboot preserves the physical fault; Capture/export includes it; Reset uses
the saved scenario fault. Add one live checkbox and an initial-profile checkbox
in the existing fixture editor, with strict boolean validation. No new transport
fault/retry framework, route or process. The bridge sees it on its next read.

Chosen approach is an extension of the current projection/model/UI path.
Rejected alternatives: a separate retained fault stream/cache introduces duplicate
ownership; reading all fixture settings on a second poll is unnecessary; setting
brightness to zero on lamp failure would falsify independently reported values.
No remaining user decision; UI implementation choices follow the existing modal.

## Grounding, tests and completion

| Claim | Reached from | Current evidence |
|---|---|---|
| Status is already read | output::execute -> read_base | src/service/output.rs |
| All eight bits already decode | Query::Status.decode | src/dali/gear.rs Status |
| State currently omits flags | Watcher/accepted -> projections | src/service/proxy/state.rs |
| settings command exposes flags | read_output_settings -> settings::execute | src/service/output/settings/execute.rs |
| native status currently contains lamp-on only | HAT transmit -> model match0x90 | src/emulator/model.rs |
| model capture preserves output config | CaptureScenario -> Model::scenario | src/emulator/model.rs |
| both UIs follow snapshots | protected SSE / independent emulator polling | web/src/devices.tsx; emulator-web/src/app.tsx |

Fail-first native status projection and literal-model query tests. Cover true/false
fault clearing; status independent from level evidence/Available; FF all bits;
missing/collided reads; stale lifecycle and retired generations yielding nulls;
read-only Desired refusal and original MQTT context. Managed real-emulator/TCP
acceptance proves selected-fixture toggle -> native bytes -> retained MQTT State,
clear/recovery and unchanged saved config/native levels/other outputs. Scenario
old/default/faulted roundtrips, capture/reboot/reset and strict ID/boolean errors.

Real Chrome at390x844 and1440px: healthy/no icon, simulated failure/compact icon,
readable status modal, all flags/raw, explicit refresh, clearing, name draft and
layout preservation, both emulator checkboxes. Use existing private TCP emulator
and broker fixture with periodic All polling disabled; explicitly read for the
operator flow. Root demo is a separate process and remains running after refresh.
No real HAT/home MQTT/Store configuration is used.

Use named RED controls for null/staleness, status-vs-level independence, raw FF,
model fault bit and selected-device isolation as appropriate; initial fail-first
cases suffice where they hit the exact mechanism. Run focused tests, frontend
build/byte checks, then one full native/browser pass, scoped Clippy, Pi, wait and
affected control-plan checks. Keep mutations/builds/readers serialized. One fresh
design verifier then final source reviewer under ways-of-working; same-family
models are available. Add docs/mqtt-outputs, gear-settings, emulator-web/emulator,
support matrix, CLAUDE/progress and this spec's acceptance evidence. No memory or
skill change; Store/PanelScript skills do not apply. Physical HAT verification
remains pending. Skills: ways-of-working, memory-recall, staged-design-review.

Self-review: separate status quality from level and Color quality; retain existing
generation/lifecycle fencing and ContextualState cause. Explicit nulls are needed
because SDK merges State maps. Scenario field defaults/omission preserve old
captures; independent model fault must survive its ordinary capture/reboot path.
No unresolved architecture gap and no extra infrastructure required.

## Design verification

Fresh read-only GPT-6.1 Sol/high design verification: APPROVE. Root adopted the
existing generic Modal/current SSE observation path rather than settings job-result
diagnostics; generation/lifecycle fences and optional native-output persistence
were verified against current code. Same-family review only; no cross-family
review capability is available in this host. No remaining user decision.

Initial corrected fail-first projection run failed on the intended assertion:
LampFailure was absent rather than true. Earlier Hardware::Absent test typo was
corrected before recording that RED; its compile error is not behavioral evidence.
Focused native projection/model/session/TCP-MQTT acceptance: 8/8 passed. Frontend
unit suite: 91/91 passed; both TypeScript builds passed. Final acceptance below.

Mid-checkpoint: the complete linked Chrome acceptance passed in146.415s at
390x844 and1440x1024, including new native-fault set/clear, SSE draft retention,
compact modal/all flags and stale bus recovery. Four scoped deliberate defects
returned their named REDs in58.3s and restored source. Affected existing control
plans have zero rotted rows. Two earlier browser setup failures were corrected:
target authority must come from live Inventory, and snapshot-driven checkbox
tests must click then await confirmed native state rather than require an
immediate optimistic checked state. No production workaround was introduced.
