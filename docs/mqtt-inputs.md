# Live DALI inputs over MQTT

This guide is for operators and MQTT consumers of the v2 development bridge.
Named input instances now publish observed State and native events through the
existing managed UART service. Addressed discovery and explicit native event
configuration, including Part301 button timing with device minima and actual
readback, are implemented locally; see [event settings](input-event-settings.md).
This boundary uses saved module/instance metadata. No
physical I4 or switch certification is claimed by local emulator tests.

## Configure the physical hierarchy

Each bus may have an `inputs` array in its v2 configuration, independently of its
outputs. For example, an input module at input short address 0 with a Part 301
button at instance 0 is represented as:

```json
{
  "short_address": 0,
  "name": "WallPanel",
  "instances": [
    { "number": 0, "name": "Door", "instance_type": 1 }
  ]
}
```

Its target is `Bus1.Inputs.WallPanel.Instances.Door` when the bus is named Bus1.
Module names and instance names follow the
[inventory identity rules](inventory-contract.md). Use the bridge's metadata
rename command for live changes; do not edit the file while the bridge owns it.
An output may independently use short address 0 on the same bus.

Common instance types are 1 button, 2 absolute input, 3 occupancy and 4 relative
light. Preserve other native numeric types as unknown. Device/instance event
sources identify the saved endpoint directly. Device/type sources resolve only
when exactly one saved instance in that module matches. Ambiguous type sources,
instance/type sources without a device address and group sources without
implemented membership evidence do not become guessed named actions. Configure
scheme 2 (device/instance) to identify each saved instance directly. Other native
schemes can serve other bus consumers; source/group routing beyond the rules
above remains pending.

## Current state

For controller `Controller`, observed button State is published at:

```text
DALI/State/Controller/Bus1.Inputs.WallPanel.Instances.Door
```

For example, a native press produces:

```json
{"Available":true,"Quality":"current","Status":true,"EventInformation":1}
```

The same MQTT v5 `target-generation` used by the complete inventory accompanies
the observation. Saved metadata never establishes a pressed/released value or
device availability. Unknown values are explicit JSON null, including fields
that previously held a known value; omission would leave stale merged State.

| Instance | Observed properties |
|---|---|
| Button | Status=true for native Pressed, false for Released, null for action-only or unknown data |
| Absolute input | PositionRaw, the native 10-bit event position |
| Occupancy | Occupied and Movement booleans |
| Relative light | LightLevelRaw, the native 10-bit event measurement |

Every instance also has Available, Quality and EventInformation. Supported
current values use Available=true/Quality=current. A recognized button action
without a current physical value uses true/partial and Status=null. Unknown
native data uses false/unsupported and null type-specific fields. Unobserved or
invalidated UART/bus evidence uses false/unobserved or false/stale and nulls.
Unavailable does not mean off or released. A quiet sensor is not declared absent
merely because no new event occurs.

Raw native position is not automatically a Boolean maintained-switch value.
Relative light is not universally lux. Verified device-specific unit conversions
remain separate work.

## Query current values

An instance `refresh` queries physical type, resolution and the complete input
value, followed by its independent indicator feedback. It works before the first
event and when event delivery is disabled.
Send this Command with an MQTT v5 response topic to `DALI/Command/Controller`:

```json
{"command":"refresh","target":"Bus1.Inputs.WallPanel.Instances.Door"}
```

A pressed one-bit button then has State including:

```json
{"Available":true,"Quality":"current","Status":true,"ValueSource":"query","ValueResolution":1,"InputValueRaw":"01","EventInformation":null}
```

InputValueRaw is an exact uppercase hexadecimal integer, big-endian and padded
to ceil(ValueResolution/8) bytes. Repeated low padding bits are removed: one-bit
FF becomes `01`; ten-bit ABC0 becomes `02AF`. Resolution can be 1..255 bits.
Hex avoids JSON number precision loss. Missing/colliding replies or incomplete
latches never become a fabricated zero or partly accumulated measurement.

| Queried type | Additional current properties |
|---|---|
| Button, resolution 1 | Status Boolean |
| Absolute input | Position normalized 0..1; InputState Boolean only at resolution 1 |
| Occupancy, resolution 2 | Occupied and Movement Booleans |
| Relative light | Exact raw value and resolution, without invented lux |

Position is approximate for wide values; InputValueRaw stays exact. PositionRaw
and LightLevelRaw retain their existing ten-bit event meaning and are null for
queries. ValueSource is query/event/null. A native event clears old query-only
values and resolution metadata; absolute event State remains raw until refreshed.
A newer native observation wins over an in-flight query's older latched value.
Polling never produces a button action.

Query Available requires a complete supported/type-matched current value.
Metadata-only replies use false/partial, no type reply false/unavailable, and a
changed physical type false/type_mismatch. Unknown types and incompatible button/
occupancy resolutions use false/unsupported, preserving any exact raw diagnostic
value. Stale evidence clears query fields with explicit nulls. Query State keeps
the original request trace; newer unsolicited native evidence has no trace.

Aggregate refresh includes enabled input instances alongside outputs under the
existing fair cursor and 15 s total ceiling. Each input gets at most 4 s without
renewing deadlines per byte. Both operation kinds share one UART owner/permit;
caller cancellation does not release active physical work early. Project the
latest same-generation snapshot after the final await. Module target refresh
reads indicator feedback. Instance target refresh has one twelve-second ceiling:
four for the value and eight for feedback. Aggregate refresh adds feedback work
only for previously sampled scopes within its existing fifteen-second ceiling.

Input measurements remain read-only. Modules and instances accept boolean
`FeedbackActive` Desired values and publish separate FeedbackActive/Available/
Supported/CapabilityRaw/Quality fields. See [indicator bindings](input-feedback.md#scalar-indicator-bindings)
for examples, unknown/stale semantics and aggregate State trace behavior.

## Native actions

Events use the controller's non-retained event lane:

```text
DALI/Event/Controller
```

A native press for the example instance produces:

```json
{
  "kind": "dali_input",
  "target": "Bus1.Inputs.WallPanel.Instances.Door",
  "instance_type": 1,
  "frame": 32769,
  "information": 1,
  "action": "pressed"
}
```

Actions are `released`, `pressed`, `short_press`, `double_press`,
`long_press_start`, `long_press_repeat`, `long_press_stop`, `free` and `stuck`.
One resolved native frame produces one event. There is no second gesture
recognizer: consumers should select the native action they need, rather than
treating both Pressed and ShortPress as the same click. Source State can coalesce
to the latest value; the action queue preserves successive press/release events.

Sensor events also contain `position_raw`, `light_level_raw`, or
`occupied`/`movement`/`repeated`/`sensor`, where sensor is `presence` or `movement`.
Unknown data retains frame, information and instance_type without an action.
Unsolicited events and State explicitly have no traceparent; they do not inherit
the trace of an unrelated light command running at the same time.

The service notice broadcast holds 64 events, with a separate maximum 128-notice
HAT batch. Overflow/lag reports `kind=input_event_loss`; lost actions are not
reconstructed from State. BridgeKit retains its existing bounded delivery policy:
events still queued during a broker outage may arrive after reconnect. Events
are not retained or persisted as an action history. Reconnection and State
refresh do not synthesize actions.

## Names, receive boundaries and local verification

New/changed mappings first obtain a receive-only boundary from the existing UART
reader. Queued frames and a line begun before that boundary cannot acquire the
new mapping token. Unchanged mappings keep their boundary. Mapping activation
can therefore discard actions while the new route starts listening. Metadata
saving and healthy output commands do not wait for input readiness. The owned
snapshot's input_listeners set shows established routes, not physical presence.
Inventory uncertainty suppresses action publication.

The [ATX emulator](emulator.md) already accepts raw input-event controls. With
the example saved module, `{"op":"input_event","bus":0,"frame":32769}` injects
the press above; frame 32768 releases it. The emulator physical scenario and saved
bridge inventory are separate. Configure compatible addresses/types when using
them together. These are private local controls, not MQTT commands to the bridge.

Local regression tests use independent literal UART replies as well as the
existing PTY emulator and an isolated FakeBroker. They cover actions during a
pending light command, explicit unknown state, raw sensors, native metadata,
partial-line rename cutover and input changes during aggregate reads. They never
contact the live bus or production MQTT broker. See
[the support matrix](support-matrix.md) for pending capabilities.
