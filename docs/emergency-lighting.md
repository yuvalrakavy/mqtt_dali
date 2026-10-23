# DT1 emergency lighting

Individual saved fixtures support explicit native status captures, settings and
function/duration test requests through the bridge's protected web UI, MQTT
commands and typed service API. Open **Actions → Emergency lighting**. Refresh
status explicitly to inspect a running test; the bridge does not hold the bus
while a battery test runs.

`read_emergency` is a Read command with no parameters. Writes use the original
target-generation and `expected_revision`, as other fixture commands do:

```json
{"command":"start_emergency_test","target":"Bus1.Outputs.Exit","expected_revision":7,"test":"function"}
```

`test` is `function` or `duration`. `stop_emergency_test` takes only
`expected_revision` and cancels pending/running tests without clearing earlier
valid-result flags. A successful request returns `outcome:"request_sent"` and
fresh native status. This is a request acknowledgement, not a newly correlated
test result. Subsequent reads expose native pending/running and last valid result
flags. A duration test uses the battery for the fixture's native test period.

## Settings and evidence

`configure_emergency` takes `expected_revision` and a nonempty `settings` patch.
Unknown fields, nulls, fractional values and invalid unit multiples are rejected.

| Setting | Supported public value |
|---|---|
| `emergency_level` | Native 1–254, within fresh manufacturer limits; adjustable-level support required |
| `prolong_seconds` | 0–7650, multiples of 30 seconds |
| `test_execution_timeout_minutes` | 15, or whole days 1–255 expressed in minutes |
| `function_test_interval_days` | 0–255; 0 disables automatic function tests |
| `duration_test_interval_weeks` | 0–97; 0 disables automatic duration tests |
| `function_delay_minutes` / `duration_delay_minutes` | Initial delay 0–983010, multiples of 15 minutes |

Automatic settings require fresh automatic-test support. Every positive interval
must include its corresponding initial delay; delay-only or disabled-interval/
delay pairs are invalid. The UI includes the displayed delay when saving an
interval, and the displayed interval when changing its first delay. Native shared
delay staging is consumed separately for each schedule. A countdown may already
advance or trigger a test during readback; `unconfirmed` never claims matching
settings and never causes automatic replay. The UI retains unsaved edits while
requiring a fresh read before another write.

Reports retain original target/generation/revision, support (`true`, `false` or
`null`), optional before/observed values, native readings and
`configuration_changed:false`. Settings change the fixture, not its saved name,
address or bindings. Each captured field has raw evidence, status, value, unit
and `at_least`. Coverage quality does not certify battery health.

Battery raw 0–254 is linear charge; 255 is unknown. Mode/features/failures/status
are full bitmaps, including meaningful 255. The last duration result and rated
duration use 120-second units, emergency runtime hours and total runtime
four-hour units. Counter 255 is a saturated lower bound, not MASK. Named status
flags omit the ambiguous battery flag at bit 3; its raw value remains available.
The UI displays a last duration result only when the native result-valid flag is
set. Missing/collided or invalid evidence remains explicit.

Writes require a known HAT repeat policy before any native prefix. Fresh fixture
presence, DT1 support, features and requested settings precede mutation. Reads
work with the unverified policy. All work uses the current physical owner,
original authority, permit and 15-second operation cap. Lost replies require an
explicit status read; no retry, poller or per-test journal is added.

Normal output reads/readback also retain nullable read-only
`SwitchingSupported` and `EmergencySupported` MQTT State properties. Type-only
sampling does not select a color representation. A proved relay has a power
toggle and virtual scene-level labels. Stale evidence clears capabilities.

## Emulator and local verification

Select **Emergency I4** in the emulator's Starting scenario, or add a **DT1
emergency** fixture. The profile editor configures native settings, battery/fault
flags, readiness and test durations/outcomes. Live **Emergency simulation**
controls change charge, failures, readiness/outcomes and advance the existing
clock. Setting readiness false makes tests pending. Pending timeout sets the
corresponding failure flag while preserving pending; a later ready transition
can still start the test. Stop cancels it. Native zero encodes a 15-minute timeout.

Profiles are optional and strict. Function/duration simulation durations are
1–86400 seconds. Active/pending timer flags are runtime state. Capture preserves
native configuration and completed observations, excluding active timer overlays
and repeat/enable staging. HAT reboot preserves native fixture state; scenario
reset restores the saved starting profile. Large clock jumps coalesce at most
one automatic occurrence per test kind. Battery/charger physics and certification
are not modeled. See `examples/emulator/emergency-i4.json`.

Focused native tests cover literal UART, independent TCP, both known repeat
policies, unverified-write refusal, native units/bitmaps/saturation, paired
schedules, timeout boundaries, Stop and original prepared authority. The linked
Chrome workflow covers phone/desktop forms and the separate emulator tab.
Physical DT1 fixture validation remains pending. REST, INHIBIT, factory/counter/
result resets and specialist manufacturer functions are outside this common
operation surface.

Primary protocol references: [python-dali Part202 definitions](https://github.com/sde1000/python-dali/blob/master/dali/gear/emergency.py),
[Beckhoff emergency variables](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/12783946635.html),
[Beckhoff queries](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/12783980939.html).
The duration interval limit follows the 0–97 table; conflicting broader prose is
not inferred as support. Total runtime 255 uses Beckhoff's 1020-hour lower bound.
