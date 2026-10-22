# DALI-2 input protocol codec

This contributor guide describes `dali::input`, the pure codec used by input
integration. It constructs read-query frames and interprets native event data.
It performs no UART I/O or configuration writes. The managed service now connects
resolved input frames to [live MQTT State/events](mqtt-inputs.md), separately from
this pure codec. Hardware discovery, configuration and browser controls remain
in progress. Saved named modules/instances and durable
metadata rename are implemented separately in the
[inventory contract](inventory-contract.md).

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
conversion when one exists. Discovery supplies the resolution needed for later
full-value reads and interpretation.

## Read-query frames

`Address::query(DeviceQuery)` uses the device-wide FE instance selector.
Queries include Status, Version and NumberOfInstances. For short address 0,
NumberOfInstances is `01FE35`; at address 63 it is `7FFE35`.

`Address::query_instance(instance, InstanceQuery)` addresses one instance.
Queries include Type, Resolution, Error, Status, Enabled, EventScheme, InputValue,
InputValueLatch and the three event-filter bytes. InputValue at short address 63,
instance 31 is `7F1F8C`. Wrap these in the existing ATX Bits24/`l` command; repeated
16-bit `t` commands are a different operation.

Building a query is not a device response or proof of presence. No response and
collisions must remain unavailable evidence in the executing service.

The independent literal tests cover source schemes, native action values,
command/event distinction, contextual type resolution, unknown data, raw sensor
fields and device/instance query bytes. Protocol cross-checks use
[python-dali's Part 103 definitions](https://github.com/sde1000/python-dali/blob/4b21c18e79d02d2a97f91d135f0eee450e8c2fb9/dali/device/general.py)
and its adjacent pushbutton/occupancy/light definitions. The
[DALI Alliance input overview](https://www.dali-alliance.org/dali/sensors.html)
describes the common input families. Physical I4/switch validation remains pending.
