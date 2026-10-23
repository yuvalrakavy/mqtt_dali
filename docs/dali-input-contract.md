# DALI-2 input protocol codec

This contributor guide describes `dali::input`, the pure codec used by input
integration. It constructs query and configuration plans and interprets native
event data. It performs no UART I/O. The managed service now connects
resolved input frames to [live MQTT State/events](mqtt-inputs.md), separately from
this pure codec. Managed discovery, saved named modules/instances and durable
metadata rename use the [inventory contract](inventory-contract.md).
Enable, priority, event scheme, filter and instance-group configuration are
available through the managed service and protected browser controls; see
[input event settings](input-event-settings.md). The button timing codec/model
below is implemented; managed timer controls, sensor timers, feedback and natural
emulator stimuli remain pending.

Input short addresses are 0..63, independently of output addresses. Instances
and input groups are 0..31. Use the validated `Address`, `Instance` and `Group`
types instead of the control-gear address types.

## Native event sources

Pass a 24-bit DALI frame to `decode`. Commands have bit 16 set and return None;
they must never create input actions. Wider values and reserved source schemes
return an error. Events retain their original frame and complete 10-bit data.

| Source scheme | Information available | Example frame |
|---|---|---|
| Device/type | Short address and instance type | `020401` |
| Device/instance | Short address and instance number | `028801` |
| Instance/type | Instance number and instance type | `828801` |
| Device group/type | Device group and instance type | `860401` |
| Instance group/type | Instance group and instance type | `C60401` |

These examples encode button-press information 1. They illustrate wire sources,
not uniquely named endpoints. Several instances in one module can share a type;
instance/type contains no device address; groups can represent several sources.
The service must resolve discovered metadata before routing an event.

`Event::payload(None)` uses an explicit type when present. Device/instance needs
its discovered type and otherwise returns Unresolved. Supply that type through
`payload(Some(kind))`; a type conflicting with an explicit wire type returns
TypeMismatch. Unknown types and unknown event data remain Unknown with raw data.

## Supported native data

| Part | Interpretation |
|---|---|
| 301 buttons | Released=0, Pressed=1, ShortPress=2, DoublePress=5, LongPressStart=9, LongPressRepeat=11, LongPressStop=12, Free=14, Stuck=15 |
| 302 absolute inputs | Raw 10-bit position field, including maintained/multiposition switch inputs |
| 303 occupancy | Movement, occupied, repeated-event and movement/presence sensor flags; reserved bits stay Unknown |
| 304 light | Raw 10-bit relative light field |

The codec preserves native short/double/long actions. It does not run another
gesture recognizer or infer a physical held/released state from action-only
events. Raw absolute/light values do not establish calibration, physical units
or availability. Relative light is not universally lux; use a verified device
conversion when one exists. Full-value reads query resolution independently.

## Read-query frames

`Address::query(DeviceQuery)` uses the device-wide FE instance selector.
Queries include Status, Version and NumberOfInstances. For short address 0,
NumberOfInstances is `01FE35`; at address 63 it is `7FFE35`.

`Address::query_instance(instance, InstanceQuery)` addresses one instance.
Queries include Type, Resolution, Error, Status, EventPriority, Enabled, the three
instance groups, EventScheme, InputValue,
InputValueLatch and the three event-filter bytes. InputValue at short address 63,
instance 31 is `7F1F8C`. Wrap these in the existing ATX Bits24/`l` command; repeated
16-bit `t` commands are a different operation.

Building a query is not a device response or proof of presence. No response and
collisions must remain unavailable evidence in the executing service.

## Event-setting plans

`dali::input::settings` builds per-instance enable/disable, event-priority,
event-scheme, event-filter and instance-group plans. `EventPriority` accepts
2..5 (2 highest); `EventScheme` accepts the five native schemes 0..4.
`GroupSlot` selects Primary, Group1 or Group2; `None` writes unassigned FF,
distinct from group 31. These are native input groups, independent of output
groups 0..15. Choosing a scheme does not prove that the bridge can resolve a
particular event uniquely; see the source ambiguity rules above.

`EventFilter` preserves all 24 bits, including FF bytes. Use `validate_for`
before a semantic write to a known common input: Part 301 mask FF (released,
pressed, short, double, long start/repeat/stop, stuck/free); Part 302 mask 01;
Part 303 mask 1F (occupied, vacant, repeat, movement, no movement); Part 304 mask 01.
Unknown types retain raw diagnostic data but have no invented common semantics.

`plan(address, instance, setting)` returns at most four `Step` values. Keep all
steps under one bus permit. DTR0/1/2 are single C130/31/32 writes, least
significant filter byte first; the final setting uses the checked short address
and instance and must be sent twice within 100 ms. **A repeated 24-bit step means
two ordinary ATX `l` sends**; the ATX `t`/repeat flag only accepts 16-bit frames.
Transmission does not prove application: the managed caller must check current
authority, type/presence and actual settings afterward. QueryEnabled answers FF
for true and silence for false, so an absent device must be ruled out separately.

This slice supplies pure plans and an [independent emulator model](emulator.md#input-event-settings-model).
Managed MQTT/HTTP commands and browser controls now use these plans; see
[event settings](input-event-settings.md). Existing discovery does not configure
inputs. Managed timers and feedback are separate work.

## Push-button timing plans

`dali::input::button` supplies Part 301 `Timer::{Short, Double, Repeat, Stuck}`,
checked `Value`, current/minimum query frames and two-step write plans. Only
short/double have native minimum queries. Static byte-range checks are separate
from `Value::check_minimum`: an owner must first establish a button instance,
query its actual minimum, validate it and verify actual settings after writing.
A plan performs none of that I/O. Double zero disables timing, but an invalid or
missing minimum reply does not become a valid minimum. FF is valid short/stuck
data. `duration()` uses 20ms units except stuck's one-second units.

For example, the [independent emulator's button profile](emulator.md#push-button-timing-profile)
at input short address 0/instance 0 answers `J19` to minimum query `l01000B`
(25 * 20ms = 500ms). To choose a one-second short threshold after that proof:

```text
lC13032
l010000
l010000
l01000A
```

DTR0 receives 50, then the short setting is repeated as ordinary 24-bit frames
within100ms. Current query returns `J32` in this model. Keep the complete plan
under one bus permit. A HAT `N` terminal confirms transmission, not application.
This foundation has no managed MQTT/HTTP timer command or browser field yet;
it does not recognize gestures or change settings during discovery.

The command map and units follow [python-dali Part 301](https://github.com/sde1000/python-dali/blob/master/dali/device/pushbutton.py).
[Beckhoff's variable table](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/5908386187.html)
defines static ranges, while [SetShortTimer](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/5643146123.html)
directs callers to query the device minimum. Do not substitute the emulator's
chosen minimum for hardware evidence.

## Exact full-resolution values

`Resolution::new` accepts 1..255 bits and rejects zero. `InputValue::decode`
requires exactly ceil(bits/8) bytes, MSB first, from INPUT VALUE followed by INPUT
VALUE LATCH. Strip unused repeated low bits after accumulating all bytes. Do not
discard FF as MASK or truncate to u64. The type keeps canonical big-endian bytes
and fixed-width uppercase hex; `as_u64` returns None on overflow and `normalized`
is an approximate fraction. Examples: one-bit FF -> 01, two-bit 55/AA/FF ->
01/02/03, ten-bit ABC0 -> 02AF, 255-bit all-FF -> 7F followed by 31 FF bytes.
The managed service uses Query evidence and never invents an Event for polling.

The [general variables](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/6113524875.html)
define 1..255-bit resolution; the PLC [value query](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/5634238347.html)
documents value/latch but separately limits its own LWORD result to 64 bits.
The pinned python-dali sequence supplies the low-padding cross-check.
[Button](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/5908386187.html)
and [occupancy](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/5908388107.html)
variables define their one-bit and two-bit resolutions. Status/configuration,
calibration and feedback are separate operations.

The independent literal tests cover source schemes, native action values,
command/event distinction, contextual type resolution, unknown data, raw sensor
fields and device/instance query bytes. Protocol cross-checks use
[python-dali's Part 103 definitions](https://github.com/sde1000/python-dali/blob/4b21c18e79d02d2a97f91d135f0eee450e8c2fb9/dali/device/general.py)
and its adjacent pushbutton/occupancy/light definitions. The
[DALI Alliance input overview](https://www.dali-alliance.org/dali/sensors.html)
describes the common input families. Physical I4/switch validation remains pending.
