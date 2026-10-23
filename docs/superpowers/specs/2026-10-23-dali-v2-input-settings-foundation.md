# Input event settings foundation

Approved mission: comprehensive common DALI-2 input support. This T2 slice builds
the protocol and independent physical model before exposing hardware writes in
the managed service and browser. Existing discovery never changes input settings.
The next slice must retain the physical owner, original authority/deadline and
permanent Notice revocation; this slice adds no MQTT command or UI.

## Shape and evidence

Use pure checked per-instance plans in `dali::input::settings`, and independent
literal opcode handling in the emulator. Reusing the production codec in the
emulator would make agreement circular. Writing configuration implicitly during
discovery would change installed equipment without an explicit operator action.
A persisted bridge shadow is unnecessary for settings that hardware can report.

Current `dali/input.rs` exposes queries for enabled/scheme/filter, but no write
plans. `emulator/input.rs::State::query` handles type and values only;
`model.rs::transmit_input` already owns the 100ms repeated-pair latch. Extend
that latch to instance setting opcodes rather than adding another repeat owner.
These claims were checked in code before design.

Protocol references: [python-dali Part 103 commands](https://github.com/sde1000/python-dali/blob/master/dali/device/general.py),
[Siemens common-instance filter/priority tables](https://cache.industry.siemens.com/dl/files/151/109771151/att_1120833/v3/109771151_ConfigDaliModule_DOC_v11_en.pdf),
[Beckhoff event priorities](https://infosys.beckhoff.com/content/1033/kl6821/9837733003.html).
Part 103 setting opcodes 61..68 send twice; DTR0/1/2 are C130/31/32.
Event schemes 0..4, priorities 2..5, groups 0..31 or FF (unassigned).
Filters use up to 24 bits; common type masks are button FF, absolute 01,
occupancy 1F, light 01. Unknown types have no invented filter semantics.

## Contract and scope

- Checked `EventScheme`, `EventPriority`, `EventFilter` and `GroupSlot` types.
  Filter construction preserves all 24 raw bits; a separate common-type check
  refuses unsupported/reserved bits, and unknown types refuse that check.
- `Setting` represents enabled, priority, scheme, filter or one of three group
  slots (optional validated Group). `plan(address, instance, setting)` returns
  bounded `Step { frame, repeat }` values. DTR steps are single; only the final
  addressed setting is repeated. The caller owns the complete plan under one
  bus permit. No plan performs I/O or proves successful application.
- Extend InstanceQuery with Priority and the three groups. Preserve existing
  query/event wire values. Inputs/outputs remain independent address spaces.
- Emulator keeps per-instance enabled/priority/scheme/filter/three-group state,
  readable even without a value profile. Initialize explicit *emulator defaults*
  (enabled, priority4, scheme2, common filter mask or 0, unassigned groups).
  Do not claim these are universal hardware reset defaults.
- Settings survive emulator HAT reboot and address changes, as other fixture
  state does. No scenario schema change is needed for this first model slice.
  Snapshot exposes settings for diagnostics; it is not saved bridge inventory.
- A setting requires two consecutive equal frames within100ms, with no
  intervening input/output frame. DTR writes are broadcast scratch-register
  changes; only the addressed existing instance changes its settings. FD and FF
  retain their existing native broadcast meanings. Unsupported settings/invalid
  DTR values cannot create affirmative backward replies. Known type filter writes
  retain only supported bits; unknown types retain all 24 raw bits.
- QueryEnabled returns FF for true, no backward reply for false; missing devices
  also have no reply. Later service reads must bracket presence/type before
  interpreting that response. FF filter bytes are valid data, not MASK.
- Emulator does not schedule natural gestures or generate events automatically.
  The existing raw `input_event` fault/test injection deliberately bypasses
  configured filters and enable state. Semantic stimulus support is later work.
  Timers, feedback, device-wide settings and persistent-variable save are outside
  this slice. No memory/Store/live-bus/dependency changes.

## Execution and verification

1. Add literal emulator behavior tests first; run RED on existing model.
2. Add checked codec plans/literal boundary tests; implement independent model.
3. Targeted codec/model and existing input/addressing tests; full native once,
   scoped lint/Pi/format/wait checks. No browser change means no browser rerun.
4. Intentional faults: omit final repeat, truncate filter high bits, accept a
   late pair, allow reserved priority, mutate sibling instance. Each must fail
   the named literal test. Then freeze and use source-only protocol/model lenses.
5. Update contributor codec guide, emulator guide/support matrix/progress and
   CLAUDE resource guidance. No reusable skill change is warranted.

Tests use `cargo nextest --lib` driving `Model::transmit_input` with literal
frames, queries and snapshots, and codec plans against independent literal frame
arrays. Include instances0/31, addresses0/63, all settings, mixed buses/devices,
unknown types, enabled false, filter FF/high bits, repeated-pair 100/101ms,
interrupted input/output traffic, and preservation of values/latches/address.
No emulator-only result is hardware certification.

## Design self-review

Round1 checked current codec, independent model, scenario validation and both
existing repeat interruption paths. Round2 checked sibling instance/module/bus
selection, invalid DTR arms, FF versus absent semantics and unknown filter widths.
Correction before gate: keep raw24 filter validation separate from common-type
validation so diagnostic data is not truncated; explicitly label model defaults
and raw event injection. Bounds: at most4 plan steps,32 setting records/module,
64 modules/bus,4 buses. No waits/tasks or unbounded queues are introduced.
One T2 design gate carrying implementer and reviewer lenses precedes code.

Design gate: APPROVE, no blockers. Source-only, same-family review; no independent
model-family coverage. It specifically checked FD/FF and sibling-instance
selection, invalid DTR refusal, unknown filter width and retained value latches.
