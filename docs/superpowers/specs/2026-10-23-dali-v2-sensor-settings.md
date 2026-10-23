# Common sensor event settings

Approved mission: comprehensive common DALI inputs, configured from the protected
bridge UI with hardware truth. Button timing is complete. This T2 slice covers
the nine standard parameter bindings for absolute inputs, occupancy and light
sensors. No user decision is outstanding; no physical hardware is required.

## Shape and checked evidence

Use checked pure plans, an independent physical profile and an optional nested
patch in the existing input-settings operation/dialog. Separate commands/owners
would duplicate authority, deadlines and Notice cuts; a saved settings shadow
would obscure what the device actually reports. Preserve the current static
common settings and completed button-timing path.

Read `dali/input/button.rs`, emulator input dispatch/repeat owner and managed
`settings.rs`/`settings/button.rs` before design. The same extension points fit:
independent instance profiles before measurement lookup, repeated ordinary24-bit
plans, type-bracketed probes and complete whole-patch preflight before any writes.
The queued fallback report is already boxed. Native and browser tests own private
PTY/broker/server resources; no live HAT or Store actions are needed.

Primary wire reference: [Phoenix Contact PLCnext manual, pages149–152](https://s3.eu-central-1.amazonaws.com/plcnext-store-prod/doc/SE_DAL2_Dali2-v2_0_0-20210324153545.pdf)
(local PDF fetched, all four table pages rendered and inspected), cross-checked for
Parts303/304 against [python-dali occupancy](https://github.com/sde1000/python-dali/blob/master/dali/device/occupancy.py)
and [light](https://github.com/sde1000/python-dali/blob/master/dali/device/light.py).
Ranges/units: Beckhoff [absolute](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/8021122955.html),
[occupancy](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/5908388107.html)
and [light](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/5911718027.html).
IEC62386-303's published text (9.5.4) specifies FF for an unsupported hold timer,
ignored FF hold writes and raw0 corresponding to a one-second hold.

| Type | Parameter | Set | Query | Native byte range/unit |
|---|---|---|---|---|
| Absolute2 | report | 10 | 1E | 0..255, seconds;0 disables |
| Absolute2 | deadtime | 11 | 1D | 0..255,50ms;0 disables |
| Occupancy3 | hold | 21 | 2D | 0..254,10s;0 means1s; queryFF unsupported |
| Occupancy3 | report | 22 | 2E | 0..255, seconds;0 disables |
| Occupancy3 | deadtime | 23 | 2C | 0..255,50ms;0 disables |
| Light4 | report | 30 | 3E | 0..255, seconds;0 disables |
| Light4 | hysteresis | 31 | 3F | 0..25 percent;0 disables |
| Light4 | deadtime | 32 | 3D | 0..255,50ms;0 disables |
| Light4 | hysteresis_min | 33 | 3C | 0..255, sensor input units |

## Codec and independent model

- `dali::input::sensor` exposes `Parameter`, checked `Value::new(kind,param,raw)`,
  type-specific query frames and two-step DTR0/final-repeat plans using existing
  Step. Reject wrong families/unknown types and invalid ranges. A plan is not a
  capability proof. Duration conversion is exact; hold0 is1s, other timers use
  their native multipliers, hysteresis parameters have no duration.
- Independent emulator profile per type2/3/4, without production codec imports.
  Explicit profile defaults: absolute report0/deadtime2; occupancy hold90/report20/
  deadtime2; light report30/deadtime30/hysteresis5/minimum0. These are emulator
  choices, not universal resets, calibrated values or universal hold support.
  The occupancy profile models supported hold; literal service peers also test
  unsupported FF. Invalid writes are ignored in this limited model.
- Extend the existing repeat predicate without another latch. Keep old button
  and common-setting predicates individually intact where practical. Sensor writes
  need adjacent equal frames within100ms; input/output interruption still breaks
  pairs. FE/device-wide commands and other types stay separate. Preserve values,
  latches, siblings/buses and settings through readdress/HAT reboot. No scenario
  schema change or natural stimulus scheduling.

## Managed API and evidence

- Add optional strict `settings.sensor_settings` containing a nonempty subset
  of report/deadtime/hold/hysteresis/hysteresis_min as native integer bytes.
  Null, unknown keys, invalid ranges, wrong-family properties and non-sensor
  targets reject before physical work. No new command/topic/owner or config save.
- Values add optional `sensor_settings` (null for other types), all five nullable
  raw fields plus `hold_supported:bool|null` and independent complete/partial
  quality. Inapplicable fields are null. Probe only applicable queries before the
  closing type proof. For occupancy, holdFF is known unsupported (support=false),
  hold0..254 supported; null is unknown, never unsupported or zero. Other numeric
  FF bytes remain data. Complete accepts unsupported hold with known report/deadtime.
- Keep common and optional quality independent. A sensor patch needs complete
  valid sensor evidence; requested hold additionally requires actual support.
  Preflight the whole patch before any common/button/sensor setting write, then
  append checked plans under the original bus permit and8s deadline. No retry,
  rollback, effective-interval inference or implicit filter changes.
- Read actual settings afterward. Success requires complete matching requested
  parameters and the existing type/authority/stamp/deadline acceptance. Preserve
  raw unsupported/missing distinctions; never report desired values as actual.
  Settings queries do not replace current input values or create input events.

## Protected dialog

Reuse the current frozen Event settings scope, dirty-only patch/save/readback,
synchronous dismissal and layout cleanup. Add compact applicable sensor fields:
Report interval(s), Deadtime(ms), Hold time(s), Hysteresis(%), Minimum hysteresis
(sensor units). Report0/deadtime0/hysteresis0 are explained as disabled. Deadtime
uses exact50ms steps. Hold accepts1s or10s increments up to2540s; do not round.
An unsupported hold is an explanation instead of an editable input; report and
deadtime remain usable. Partial sensor evidence leaves common settings usable.
No lux/calibration claim. Validate claimed complete data, family, raw ranges,
units/support and returned requested values before success.

## Implementation and verification

1. Literal model tests first (prove RED), pure codec literal maps/range/unit cases,
   independent profile and repeat extension. Cover all nine bindings, zero/FF,
   wrong families,100/101ms/interruption, instance31/address63, FD/FF scope,
   untouched latches/settings and UART/reboot retention.
2. Fail-first managed patch/readback; extend the existing operation/helper and
   literal scripts only for matching types. Real service/literal cases cover all
   families, strict invalid patches, unsupported/missing hold, mixed no-write
   preflight, ignored writes, unchanged saved bytes/identity/native values and
   current authority/deadline. Mid-checkpoint and diff/comment self-review.
3. Frontend unit cases for exact units, hold1s special case, dirty-only patches,
   family/range/support validation, malformed complete data, actual match and
   common-only operation with partial optional data. Real phone/desktop flows
   edit/reopen/restore each family and verify native raw settings/retained inventory.
4. Serial controls: wrong family/opcode, hold0 conversion, single model write,
   unsupported-hold preflight, missing evidence, ignored actual, client units and
   matching. Maintain affected old anchors and rerun any corrected control.
5. Freeze; two source-only native/model and UI/contract lenses. Full native once,
   focused frontend/real browser, strict scoped Clippy/Pi/wait/format/asset/affected
   control-plan checks. Update operator/API, codec/emulator/browser/service guides,
   support matrix, CLAUDE and progress with observed evidence.

No Store, memory/skill, dependency/publication/deployment, sensor calibration,
newer sensitivity/range capabilities, feedback, catch/cancel control actions or
natural emulator stimuli in this slice. Those remain explicit later work.
Budget one T2 design pass after two self-reviews, two final source-only lenses.

## Self-review

Round1 checked each primary opcode/unit table and current per-instance/repeat,
probe, preflight and actual-matching seams. Round2 traced FE overlap (light set30
versus device Status), optional holdFF versus numeric report/deadtimeFF, hold0's
special duration, partial optional evidence, whole mixed-patch ordering and
unchanged value-latch ownership. One correction: unsupported hold is complete
capability evidence and must not disable report/deadtime edits; missing hold
remains partial. Same-family review limitation remains disclosed.

## Design gate and implementation checkpoint

The fresh-context combined implementer/reviewer design lens APPROVE, source-only
on GPT6Astra/high. Its checks are implemented: preserve raw unsupported holdFF,
keep FE/status30 outside the sensor repeat predicate, use valid family-specific
literal filter masks, and validate hold's discontinuous display range explicitly.
No implementation delegation or cross-family review was available/claimed.

Literal model tests first failed on absent parameter queries, then passed with
the independent profile. Managed tests first failed on the unsupported nested
patch; frontend tests first failed on missing actual-match/support guards. The
22 focused native cases and61 frontend cases now pass. Diff review confirmed
the existing owner, boxed fallback report, original deadline, type bracketing,
whole-patch preflight and module-wide Notice cuts remain in use. No config write
or natural event scheduler was added.

Actual phone/desktop edit/reopen/restore flows pass for all three sensor families,
including hold1s and numericFF. The first full browser attempt passed the new
flows but found a reassignment assertion still expecting four children after the
fixture gained occupancy as its fifth. Both literal counts now expect five and
the sensor flow asserts all three families are present. The corrected full run
passes921 owned commands in75.76s. All six sensor screenshots were inspected;
the dialog scrolls to its visible actions on phone. Artifacts:
`/tmp/dali-browser-fe073b2c6413c27988b46db167abb41b`; log
`/tmp/dali-sensor-settings-browser2.log`.

All11 serial intentional-fault controls produce their declared RED in109.2s,
including wrong opcode, hold conversion, repeat omission, unsupported/missing
preflight evidence, FF preservation, ignored-write outcome, exact UI units and
preflight/actual-match submission guards. Source was restored after every row.
The measured ledger is `docs/v2-sensor-settings-controls.ledger.json`.
