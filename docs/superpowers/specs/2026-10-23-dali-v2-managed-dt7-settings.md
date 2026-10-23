# Managed DT7 relay settings

Tier T2: additive MQTT/HTTP settings contract and relay-aware limit preflight.
Continue the approved comprehensive bridge/UI architecture. Budget two bounded
self-review rounds, one combined source design gate, inline implementation,
serial measured faults, frozen sequential final readers and applicable checks.

## Grounding and chosen shape

Foundation f3b2e30 supplies checked Part208 plans and an independent model.
Current common settings has one owner/permit/context/revision/deadline, whole-patch
preflight, matching actual readback, Blink guards and bus-local cache cuts.
Its `physical <= minimum` rule rejects adjustable DT7 virtual limits. Settings
reports and the protected dialog expose no relay capability/thresholds. These
claims are confirmed in `service/output/settings{,/execute}.rs` and
`web/src/output-settings{,-ui}.ts*`. Discovery type hints are transient and cannot
authorize writes. Saved output identity has no hardware-type field.

Options: separate relay worker/API/dialog (duplicates ownership and interactions);
persist a relay classification and change all output components (premature contract
change from a transient discovery hint); extend the existing explicit settings
report/patch/dialog with freshly proved optional relay capabilities. Choose the
last. No new command names, config format, poller, physical owner or public scalar
component change. Capability metadata is available to generic MQTT consumers in
the explicit settings reply; retained relay-specific component metadata is later
work. Do not claim it exists after this slice.

UI alternatives are a separate Relay settings dialog or an inline Relay section
in Fixture settings. Choose the inline section, using existing native-field rows,
two-column desktop/one-column phone flow, Save/Refresh/Cancel and diagnostics.
No new navigation or device-row real estate. Existing screenshots/controls establish
the interaction; inspect real phone/desktop renders after adding the conditional
section. This is an extension of the already approved form, not a redesign.

## Additive explicit contract

Keep `read_output_settings` and `configure_output {expected_revision,settings}`.
Add optional strict `settings.switching` with optional u8 `upward_on`, `upward_off`,
`downward_on`, `downward_off`. Upward1..255, downward0..255;255 means disabled.
Reject null/unknown/fractional/empty nested patches before admission. Existing
common fields and strict outer parser remain unchanged. A mixed common/relay
patch is one preflight, not partially applied if any requested family is invalid.

Settings Values gains nullable `switching_support` and optional `switching`.
Support true requires fresh positive Type7 evidence, false a valid ordered type
prefix excluding7, null unavailable/malformed evidence. A positive proof yields
relay values with nullable raw features/status/gear-type/extended-version,
nullable four thresholds, decoded adjustable-threshold/physical-selection booleans
and independent quality. Preserve raw unknown bits, collision direction/code,
invalid upward0, and disabled255. Raw switch status does not prove contact state.
An unavailable relay family does not degrade independent readable common fields.

Probe ordinary Status/Version/Actual/PhysicalMinimum/limits/startup/failure/fade,
then fresh DeviceType. For multi-type255, query NextDeviceType at most nine times,
requiring a strictly ascending prefix below254; stop at7 (positive), a value>7
or254 after at least two valid prefix types (negative), or malformed/missing
evidence (unknown). An immediate terminator or one-type tail after255 is unknown.
Query each Part208
diagnostic with its checked enable/query plan only after positive Type7. Closing
ordinary Status brackets the entire read. Include bounded raw type-prefix and
Part208 readings in the existing readings map. No group/broadcast queries.

## Whole-plan authority and acceptance

Use existing Work::OutputSettings end to end without another await/task/permit.
Existing eight-second ceiling, original caller deadline, physical epoch, target
generation and revision checks remain. No refreshed authority after awaits.
The common fields keep their existing per-request evidence requirements.

For limit edits, use virtual floor1 only with fresh positive Type7 plus readable
adjustable-threshold support=true; otherwise retain physical-minimum rules.
Unknown capability must not relax the rule. Require valid current min/max and
physical minimum and the final ordered range; preserve crossed-limit ordering.
Fixed relays remain254/254. Lowering/raising virtual limits may operate the contact;
no unconditional promise that changing limits retains an off contact. Actual
State still comes from QueryActual0/254, never a virtual setting.

Threshold edits require fresh Type7, adjustable=true and readable requested
thresholds. Reuse checked Part208 setter plans under the same DTR ownership and
verified HAT repeat policy. Execute common plans then relay plans deterministically.
Do not insert a second enable between repeated threshold frames. Re-probe and
confirm every requested value against actual evidence, including positive Type7
and support for relay fields. A sent setter/HAT terminal is not confirmation.
Preserve partial-family usability, explicit uncertainty/refresh, Blink guards,
bus-local actual/colour/group cuts and unaffected input Notices/values.

## Protected controls and tests

The existing dialog shows a Relay section only for positive Type7: four thresholds
as native-code numeric inputs plus Disabled toggles, capability/type/raw diagnostics,
and separate actual contact On/Off/unknown. Label limits as virtual control levels
for a relay and explain that thresholds control its contact. Fixed/unreadable
thresholds remain read-only; raw invalid values remain diagnostic. Typed numeric
strings update on input and survive toggle/select rerenders, including invalid
intermediate input. Disabled255 is not rendered as a brightness percentage.
Extend frozen scope's raw-evidence validation to type/relay readings and verify
all requested nested fields. Preserve cancellation and explicit refresh after loss.

Fail-first: existing typed configure call with a deserialized relay patch first
fails behaviorally on unknown field; a literal adjustable relay limit edit also
fails existing physical-floor preflight before production changes. Then test:

- Literal PTY bytes: exact native type/enable/setter/readback sequence, both repeat
  profiles, partial/masked/colliding/invalid evidence, whole mixed-patch refusal,
  no support inference, multitype ascending/duplicate/silent prefixes and bounds.
- Independent emulator/managed service: all four thresholds including255/0,
  virtual min below254, fixed relay rejection, unchanged contact after threshold
  programming, retained identity and truthful binary actual output afterward.
- Strict MQTT parser: nested invalid shapes/ranges/nulls and unchanged Read class.
- Node: nested changed-only patches, raw/value agreement, unsupported/unreadable
  edits refused, native0/255 and type proof, lost/mismatched result retirement.
- Real phone/desktop browser: edit number before Disabled toggle, save/reopen,
  independent model snapshots of all requested fields and contact, virtual limits,
  fixed/unknown controls, ordinary fixture workflow retained, protected access,
  Cancel/refresh/navigation/identity and no overflow. Inspect screenshots.

Measure intentional faults for positive Type7/support, virtual-floor relaxation,
whole-patch preflight, native enable/repeat, requested nested readback, UI proof/
matching/number retention. Reuse existing authority/cache/Blink controls, check
affected anchors and remeasure affected behavior only. No unknown-command fallback
in literal peers. Freeze, sequential native/contract then UI/model source reviews,
full native, strict Clippy, Pi library/four binaries, formatting, waits, Node/build/
asset bytes, whitespace, affected control plans and actual browser pass.

Update fixture/DT7/emulator/MQTT contract guides, support matrix, CLAUDE and progress
with exact capability metadata boundary. Relay virtual scene capture, timed fades,
fault profiles, retained component classification and physical validation remain
pending. No live HAT/Store, SDK, memory/skill, release, publication, push/merge or
deployment changes. Same-family source reviews are the available limitation.

Self-review1 opened current settings/probe/browser/strict-peer paths and confirmed
the physical-floor rejection and reusable owner. Self-review2 checked multi-type
prefix bounds/unknown arms, independent relay/common quality, invalid upward0,
fixed-profile limits, mixed patches, actual-versus-virtual wording, nested nulls,
partial evidence and existing control anchors. Native behavior is grounded in the
foundation's linked public IEC Part208 preview and primary command references.

Combined source design gate requested one contract correction: multi-type
termination must contain at least two ascending types. Explicit vectors are
255/[254] and255/[6,254] unknown;255/[0,6,254] false;255/[0,6,7] true without
further enumeration. Add successive virtual-limit saves and mixed patches with
the last requested threshold unreadable, plus post-write requested-read loss.
Methodology: vary zero/one/two entries around a sentinel, rather than treating
the sentinel byte alone as proof. The other design surfaces passed source review;
same-family limitation remains. No architectural revision is required.

## Delivery checklist before final source review

- [x] Strict nested patch and explicit relay capability/raw readings through the
  existing typed/MQTT/HTTP settings path; no new owner/config format/components.
- [x] Literal type-prefix, requested-family, exact-plan/readback and missing-
  physical cases; independent model, both repeat profiles, successive virtual
  limits and generic MQTT capability/strict-parser evidence.
- [x] Native fail-first on unknown nested settings and physical254 preflight;
  additional literal RED on missing physical evidence falsely marked complete,
  fixed without making the physical reading optional for limit edits.
- [x] Node validation and actual protected phone/desktop workflow with independent
  snapshots and inspected responsive screenshots. The browser harness disables
  periodic All polling; it does not certify operator/poll overlap. A real recovery
  completion barrier fixes the observed stale-enabled-button test race.
- [x] Serial current fault table and two affected common-limit controls measured
  RED; source/assets restored. An earlier blur-only browser mutation was GREEN
  because the checkbox click commits the changed field on blur. It is retained
  as historical evidence, replaced by a native-draft-loss fault detected by the
  unchanged independent snapshot assertion, not counted as a passing control.
- [x] Operator/native/MQTT/browser guides, support matrix and contributor notes
  state the explicit-reply boundary and pending retained metadata/virtual capture.
- [x] Frozen sequential source reviews and complete restored native/Pi/lint/
  format/wait/assets/Node/control-plan acceptance, recorded in progress afterward.

Methodology: sample missing neighboring diagnostics even when a positive capability
allows an exception; target the native edit path when an event-handler substitution
is equivalent on the exercised workflow. No memory or reusable skill changes.

Final native/contract review approved the code and found two old generic guide
claims that Off always remains Off. Both now explicitly apply to ordinary dimmers.
The UI review found one real state-transition defect: Disable on/off replaced an
explicit blank/out-of-range text draft with a hidden valid minimum. A real browser
RED submitted upward_on1 while the field was blank. Re-enable now preserves the
explicit invalid draft; the same DOM oracle verifies no job/native change for
blank and300, then confirms corrected values on both viewport sizes. The shared
four-field renderer covers upward/downward fields; neighboring startup-mode
controls deliberately clear displayed text when selecting a visible replacement.
Absent text still gives an initial visible native default when re-enabling.
The focused browser mode runs this same flow/login/service in16s; final acceptance
still runs the full workflow. Both affected native-draft and hidden-minimum browser
faults are RED with the unchanged snapshot/no-job assertions. Methodology: test
invalid → disabled → enabled, and distinguish absent drafts from explicit blanks.

Both final source gates APPROVE after those corrections. Frozen `e3db774` passes
the complete protected browser workflow:1177real commands,108.65s. Node89,
generated assets, Pi, protected-assets1/1, whitespace and every repository
control-table plan pass after the UI-only fix; backend734/734/Clippy/format/wait
acceptance remains applicable. All17current controls are RED,243.9s latest-row
sum. Detailed acceptance and scope limits are recorded in `docs/v2-progress.md`.
