# Push-button timing foundation

Approved mission: common DALI switches, comprehensively supported without needing
physical equipment for development. T2 because this adds a public protocol API.
Start with Part 301's short/double/repeat/stuck timing, whose command map and
readable device minima are independently documented. Sensor timers and feedback
remain separate slices; no guessed operation or timer behavior is introduced.

## Shape and evidence

Extend the pure input codec and independent physical emulator. The alternative,
recognizing gestures in the MQTT bridge, duplicates hardware logic and disagrees
with native events. A bridge-owned configuration shadow would also obscure actual
hardware settings. Reuse existing addressed plans, DTR0 and the bus repeat owner;
the service and existing Event settings dialog will consume this in a later slice.
No new UART worker, wait, MQTT command, scenario schema or browser is required here.

Current code checked: `dali/input/settings.rs` owns `Step { frame, repeat }`;
`emulator/model.rs::transmit_input` owns adjacent equal-frame repetition within
100ms and both input/output interruption; `emulator/input.rs` keeps per-instance
settings independent of optional value profiles. Reboot/address changes preserve
fixture state. No timer state exists in these enumerated handlers.

Primary implementation reference:
[python-dali Part 301](https://github.com/sde1000/python-dali/blob/master/dali/device/pushbutton.py).
Hardware ranges and units:
[Beckhoff button variables](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/5908386187.html),
[SetShortTimer](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/5643146123.html).
The variable table and explanatory prose differ on the universal short minimum;
do not hard-code 25 as a universal requirement. Read the device minimum instead.

| Timer | Set | Query | Minimum query | Raw range | Unit |
|---|---|---|---|---|---|
| Short | 00 | 0A | 0B | 10..255, also device minimum | 20ms |
| Double | 01 | 0C | 0D | 0 (disabled), otherwise device minimum..100 | 20ms |
| Repeat | 02 | 0E | none | 5..100 | 20ms |
| Stuck | 03 | 0F | none | 5..255 | 1s |

## Contract

- Add `dali::input::button` with checked `Timer`, `Value` and errors. `Timer`
  supplies current and optional minimum query frames. `Value::new(timer, raw)`
  checks static ranges (double nonzero starts at 10). `check_minimum(raw)` checks
  both the reported minimum's legal range and the requested value; double 0
  remains valid regardless of the valid reported minimum. Asking for a minimum
  check on repeat/stuck returns an error, not an invented capability.
- `Value::duration()` uses exact native units (double 0 returns zero).
  `Value::plan(address, instance)` returns exactly DTR0 then the addressed setting
  with repeat true. It does not execute, establish type/capability, prove actual
  hardware application or silently clamp. Later managed writes must probe type,
  read short/double minima and apply `check_minimum`, then verify actual readback.
- Independent emulator per-button `timers` stores short25/min25, double0/min10,
  repeat8 and stuck20. These are explicit emulator profile choices, not universal
  hardware defaults. Non-button instances have no timer profile and answer none
  to button-specific queries. A button needs no optional measurement value profile.
- The model accepts in-range settings respecting its profile minima, ignoring
  unsupported/invalid writes. That invalid-write policy is a deliberately limited
  test profile, not a claim about every hardware device's clamping behavior.
  FF is a valid short/stuck value. Double0 disables double-press timing. Timer
  reads/writes preserve enable/filter/groups, values/latches, other gear and buses.
- Extend the existing repeat predicate to instance00..03, with no new latch.
  Device-wide FE commands retain their current handling. Equal pairs must still
  be adjacent within100ms; DTR writes happen before the pair. Unsupported types
  remain inert even if they receive that pair. FD/FF retain existing addressing.
- Preserve timer state across HAT reboot and readdress. Raw injected events still
  bypass settings. No actual gesture scheduler, sensor timers, feedback, reset,
  persistence claim, live HAT/Store, memory, dependency or release changes.

## Execution and testability

1. Literal independent emulator tests first, prove RED on the missing model.
2. Implement pure codec plus literal query/plan/range/minimum/unit tests and
   independent profile, reuse bus repeat owner. Mid-checkpoint and self-review.
3. Model tests: all four writes and six queries, FF and double0, range/minimum
   refusals, unsupported types, address63/instance31, sibling/bus isolation,
   100/101ms and input/output interruption, latches and addressing retention.
   Literal UART transcript verifies transport and reboot persistence.
4. Intentional faults: omit plan repeat, wrong unit, apply single write, mistake
   double0, accept below reported minimum, mutate instance0. Each must fail its
   named independent assertion. Mutation controls run serially before source freeze.
5. Two source-only final lenses (protocol/codec and independent model/test reach).
   Full native once, scoped strict Clippy/Pi, changed-file format, wait validation,
   affected control plans and whitespace. UI untouched, so no browser rerun.
6. Update codec/emulator guides, support matrix, CLAUDE and progress with evidence.

Budget: one T2 design pass after two bounded self-reviews; two final source lenses.
The current source-only same-family review limitation remains disclosed.

## Self-review

Round1 inspected all settings dispatch arms, repeat selection, FD/FF routing,
value-latch location and HAT reboot's retained Model. Round2 traced opcode overlap
with device-wide FE, non-button instances and sibling state; also compared the
documented static limits against device-specific minimum queries. The API keeps
those two validations distinct and does not claim transport success is readback.

Design gate APPROVE, no blockers (source-only, same-family). It confirmed current
code seams and asked verification to retain invalid minima even for double zero,
minima above profile defaults, repeat only on the final step and non-button
instances both with and without measurement profiles. These are covered below.

Mid-checkpoint: four literal model cases failed first on the absent timer model.
The codec/model/UART focused suite now passes all seven cases. Self-review traced
the final formatted diff through the FE branch, per-instance profile dispatch,
repeat owner and untouched value-latch paths. All six serial intentional-fault
controls produce intended REDs in 85s; each production mutation was restored.
No runtime design change or new ownership/wait mechanism was needed.
