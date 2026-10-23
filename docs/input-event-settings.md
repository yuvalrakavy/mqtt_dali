# Configure a switch or sensor's events

This guide is for a v2 bridge operator or MQTT consumer. Input discovery and
event configuration are separate actions. Discovery reads and saves the hardware
hierarchy; it does not change which events a device sends.

## Use the protected web interface

1. Sign in and open **controller/Bus1 → Devices**. Discover/assign the input
   module if it is not saved yet. Operate each input and give it a meaningful name.
2. On the individual button or sensor row, choose **⋮ → Event settings**.
   The bridge reads the actual device settings. Opening the dialog changes nothing.
3. Choose whether the instance is enabled, its event priority, the source scheme,
   the events it should send and its three optional input-instance groups.
   Buttons also show short/double/long-repeat/stuck timing when complete device
   evidence is available. The first three fields use milliseconds in 20ms steps;
   stuck uses whole seconds. Device minimums are shown; double0 disables timing.
   On a phone, scroll the dialog to reach its actions.
4. Choose **Save settings**. The bridge changes only edited fields and reads them
   back. Success means the actual supported settings matched the request.
   Reopen or use **Refresh settings** to inspect them again. **Cancel** discards
   the draft. A command already sent to hardware cannot be undone by closing a dialog.

Priority 2 is highest and 5 lowest. Prefer **Device and instance** (native scheme
2), which identifies a saved instance directly. **Device and type** (1) routes
only when one saved instance in that module has the type. **Instance and type**
(0), **Device group and type** (3) and **Instance group and type** (4) are currently
diagnostic-only sources in this bridge. They can be configured for other bus
consumers but do not become guessed named actions here.

Buttons expose released, pressed, short, double, long-start/repeat/stop and
stuck/free events. Absolute inputs expose position, occupancy sensors expose
occupied/vacant/repeat/movement/no-movement, and light sensors expose relative
light changes. These are native events; the bridge does not create another
gesture recognizer. Input-instance groups are 0..31 and distinct from output
groups 0..15. Each of the three group slots may be unassigned.

Unknown instance types show their full raw filter without editable event labels.
Generic enable/priority/scheme/group settings remain available when complete
standard readback exists. Sensor timers, feedback and calibrated sensor units
remain separate capabilities. No physical HAT certification or power-cycle persistence
claim follows from local emulator/readback checks.

The saved name, identity, bindings, inventory revision and configuration file
remain unchanged. Setting changes invalidate current observations and queued
actions for the module; fresh activity or a current-value read restores usable
State. Reading event settings alone does not generate a press or replace an input
value. If a device disappears, collides, changes type, times out or returns partial
settings, the bridge cannot report a confirmed configuration. Inspect it before
retrying; there is no automatic retry or rollback.

## MQTT commands

Use the existing command envelope at `DALI/Command/Controller`, with an MQTT v5
response topic. Use the exact `target-generation` from current inventory for a
write; do not substitute the controller generation or an address. For example:

```json
{"command":"read_input_settings","target":"Bus1.Inputs.WallPanel.Instances.Door"}
```

`read_input_settings` is Read-class and accepts no parameters. It needs a saved
individual instance target. Reads may omit a generation; returned evidence still
identifies the captured input generation.

```json
{
  "command":"configure_input",
  "target":"Bus1.Inputs.WallPanel.Instances.Door",
  "expected_revision":23,
  "settings":{
    "enabled":true,
    "event_priority":3,
    "event_scheme":2,
    "event_filter":255,
    "groups":[0,null,31]
  }
}
```

`configure_input` is WithReply. `settings` is a nonempty patch: omit untouched
fields. Priority is 2..5, scheme 0..4 and groups exactly three null-or-0..31
entries. Null unassigns a group slot; null property values and unknown fields are
rejected. Common filter masks are FF for buttons, 01 for absolute/light and 1F
for occupancy. Reserved bits and semantic filter writes to unknown types are
rejected before sending. A raw filter FF is data, not MASK.

For Part301 buttons, the same patch optionally contains native timer bytes:

```json
{"button_timers":{"short":50,"double":20,"repeat":8,"stuck":20}}
```

Put that object inside `settings`, alongside common fields if needed. `short`,
`double` and `repeat` use20ms ticks (50 means1s); `stuck` uses seconds. Omit
unchanged timers. Short's static range is10..255 plus the actual device minimum;
double is0 (disabled) or the device minimum..100; repeat5..100; stuck5..255.
The bridge reads short/double minima before any setting write in a mixed patch,
then reads actual settings afterward. An invalid later timer prevents earlier
common/timer fields from being programmed. Empty nested patches, null, unknown
keys, writable minima and non-button timer patches are rejected. FF is valid
short/stuck data, not MASK. The web UI converts display units exactly and never
rounds a non-native step into a different threshold.

Authenticated HTTP jobs accept these same command names/parameters and require
their normal CSRF/session/original-target authority. The typed Rust APIs are
`ServiceHandle::read_input_settings` and `configure_input` with `input::settings::Patch`.
There is no separate raw-address API or retained Desired configuration shadow.

Reports contain operation, outcome, target/generation, revision, bus/address/
instance, reported_type, before, settings and reason. `settings.quality` is
complete/partial; absent or type-mismatched devices have no settings. Numeric
fields/required filter bytes can be null when unavailable. Groups report raw
bytes: 255 means unassigned, null means unavailable. Common filters have width 8;
unknown types preserve all 24 bits. Reports use `current`, `configured`,
`unconfirmed`, `unavailable` or `type_mismatch`. `configuration_changed:false`
means saved bridge inventory was not edited; hardware settings may have changed.

`settings.button_timers` is null for non-buttons. Buttons report nullable raw
short/short_min/double/double_min/repeat/stuck and an independent complete/partial
quality. A partial timer read leaves basic event settings usable; `current`
describes complete common settings, not a claim that every optional timer is
known. Any requested timer change requires complete valid timing evidence and
matching actual readback. A missing minimum stays null, including when requesting
double0. Unavailable timings appear read-only in the dialog.

Only complete, matching readback with current authority, physical epoch and the
original deadline can report `current`/`configured`. A write failure after possible
transmission reports `unconfirmed`; caller timeout/cancellation can also leave
hardware changed. Replies are evidence, not a replacement for the authoritative
inventory/State stream. Maximum service budget is eight seconds within the
caller's original deadline. All native steps share the physical owner; repeated
24-bit settings use two ordinary sends within a combined 100 ms window.

See [live input State/events](mqtt-inputs.md), [protocol plans](dali-input-contract.md)
and [independent emulator model](emulator.md#input-event-settings-model).
