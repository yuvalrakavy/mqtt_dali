# Fixture settings

On a bus's Devices page, open an output's actions and choose **Fixture settings**.
The protected dialog reads the fixture on entry. It shows minimum/maximum
brightness, power-on and bus-failure behavior, basic fade time and dimming rate.
Fixture diagnostics shows actual brightness, active status flags and the raw
reported version byte. Opening, Cancel and Refresh send no settings writes.

Brightness percentages round-trip the native control code, rather than measured
luminous flux. Physical minimum is read-only. Unavailable fields stay unknown;
readable families remain usable. A readable packed fade with reserved rate0
allows selecting a valid rate to repair it. Fade time0 uses other supported fade
settings and does not guarantee an instant transition. Stop Blink before saving
settings to that identifying fixture; reads and unrelated fixtures remain usable.

Save submits only changed fields under the original row identity and revision.
The bridge freshly checks the whole patch, including physical/current limits,
before any setting is sent. It orders crossed limit changes to avoid unintended
clamping. Limit changes can alter a dimmer that is already on; an off ordinary
dimmer stays off. DT7 virtual-limit changes can operate the relay contact.
Requested fields must match actual readback before the dialog closes.
An uncertain or failed save clears actionable values until explicit Refresh;
it never silently resends the patch. Names, bindings, groups and saved bridge
configuration are unchanged. No automatic reset or fixture-mode change is sent.

## Fixture status

Each output row shows a compact red warning for a reported lamp or gear failure,
or amber for a limit, missing-address or power-failure condition. A neutral question
icon means status is unknown, unavailable or stale. A current reading with none
of these conditions has no icon. Lamp-on, fade-running and reset flags remain
informational. Input rows do not show fixture fault indicators.

Select the icon to see all eight flags, the native status byte and its freshness.
**Read current values** refreshes through the normal output read. The open dialog
follows current observations, including when a fault clears or a bus disconnects.
A clear reading means no fault was reported; it does not measure illumination.
Use [MQTT status fields](mqtt-outputs.md) for automation without the web UI.

## MQTT, HTTP and typed service

Generic MQTT consumers use `read_output_settings` with an individual output target
and no parameters, or `configure_output` with the original target-generation
property, current expected revision and a strict changed-field patch:

```json
{"command":"configure_output","target":"Bus1.Outputs.Kitchen",
 "expected_revision":7,"settings":{"minimum_level":5,"power_on_level":255,
 "system_failure_level":0,"fade_time":1,"fade_rate":7}}
```

Publish using the ordinary BridgeKit command envelope and MQTT v5 reply/trace
properties described in [MQTT outputs](mqtt-outputs.md). HTTP jobs use the same
command names and `params` payload through the existing authenticated API.
Embedders call `ServiceHandle::read_output_settings` / `configure_output` with
an original RequestContext and absolute deadline. Total operation ceiling is8s,
clipped to the caller's deadline. Writes need the configured verified HAT repeat
profile. These operations are explicit and are not part of scalar All polling.

Patch fields are `minimum_level`, `maximum_level`, `power_on_level`,
`system_failure_level`, `fade_time` and `fade_rate`, all native integer codes.
Null, unknown/read-only fields, fractional values, empty patches, bad ranges and
retired authority reject. Startup/failure0..255 remain stored native settings;
they need not equal current min/max. Limit edits require fresh valid physical
minimum/current limits and a valid final min/max pair. Missing unrequested
diagnostics do not prevent a supported patch.

Reports retain original target/generation/revision/bus/address, before/after
settings, raw correlated readings and decoded status flags. Read outcomes are
`current`, `partial` or `unavailable`; write outcomes include `configured`,
`rejected` and `unconfirmed`. HTTP success alone is not fixture confirmation.
Only `configured` means every requested field matched fresh readback. Silence,
both collision directions/codes, actual MASK and reserved fade rate stay explicit.
`configuration_changed:false` refers to bridge inventory; hardware settings may
have changed. After possible writes, inspect hardware before retrying uncertainty.

There is no settings State cache. Configuring conservatively retires bus-local
base/colour/group actuals until their normal fresh read; settings reads preserve
those caches and native input values/Notices. Original authority, physical epoch,
deadline and the shared permit remain under the existing owner through settlement.

## DT7 relays

A freshly identified DT7 fixture adds **Relay thresholds** to the same dialog.
Four native virtual-level thresholds have separate Disabled toggles; disabled255
is not a brightness percentage. Fixed thresholds and unavailable capabilities
remain read-only. Actual contact On/Off comes from actual0/254, independently of
raw threshold diagnostics. Relay min/max and startup numeric fields describe a
virtual control level. Changing virtual limits can operate the contact.

For adjustable DT7, virtual minimum may be1 even when physical minimum is254.
Both the current and requested range use that exception only after fresh positive
Type7 and adjustable-support evidence. Other fixtures retain physical-minimum
preflight. Missing optional relay diagnostics do not block readable common fields.

The existing command accepts an optional nested patch:

```json
{"command":"configure_output","target":"Bus1.Outputs.Relay",
 "expected_revision":7,"settings":{"minimum_level":1,
 "switching":{"upward_on":50,"upward_off":255,"downward_off":0}}}
```

Upward thresholds accept1..255, downward0..255. Empty/null/unknown nested members
reject. The entire mixed patch requires fresh support and readable requested
fields before any DTR/setter frame. Threshold setters do not themselves operate
the contact; accompanying virtual-limit edits can. Actual requested readback,
including fresh relay support, is required for confirmation of threshold edits.

Values include nullable `switching_support` (true/false/unknown) and an optional
`switching` object only after positive Type7. It retains four nullable thresholds,
`features_raw`, `switch_status_raw`, `gear_type_raw`, `extended_version_raw`,
decoded nullable `adjustable_thresholds`/`physical_selection` and independent
`quality`. Raw unknown bits and invalid upward0 remain visible in `readings`;
invalid/missing values do not become defaults. Multiple-type enumeration is bounded
and ordered; an immediate or one-entry terminator after a multiple-type reply is
unknown. These are explicit settings replies, with no retained relay component
classification or automatic settings polling. See [DT7 switching](dali-switching.md).

## Checked protocol foundation

`dali::gear::settings` supplies checked values and command plans for minimum and
maximum dimming levels, power-on and bus-failure levels, and basic fade time/rate.
The managed service and protected controls above execute this checked foundation
through the existing UART owner.

| Value | Accepted native values | Meaning |
|---|---|---|
| `LimitLevel` | 1..254 | DALI control code; normalized form is code/254 |
| `PowerOnLevel` | Fixed 0..254 or `LastActive` | 255 restores the pre-power-loss level |
| `SystemFailureLevel` | Fixed 0..254 or `Unchanged` | 255 leaves the actual level unchanged |
| `FadeTime` | 0..15 | Codes 1..15 provide nominal seconds; 0 has no basic duration |
| `FadeRate` | 1..15 | Native steps per second |
| `FadeSettings` | Any packed byte | Time in the high nibble, rate in the low nibble |

Brightness values describe the DALI control scale, not measured light output.
Basic fade time 0 can defer to another supported fade mechanism; it is not proof
of instantaneous output. The packed fade byte FF is two numeric code-15 values.
Rate nibble 0 is retained in the raw byte, with no invented steps-per-second
value. These meanings differ from actual-level FF, which is unavailable MASK.

`Query::PowerOnLevel`, `SystemFailureLevel` and `FadeTimeRate` use opcodes A3,
A4 and A5. Queries require an individual `ShortAddress`; decode only the
transaction owner's correlated `BusReply`. Silence and collisions remain unknown.

`Setting::plan(destination)` returns a two-element array: ordinary broadcast DTR0
followed by the repeated addressed setter. For example, maximum 200 at short
address 3 produces A3C8 then repeated 072A. Setters are 2A maximum, 2B minimum,
2C bus failure, 2D power on, 2E fade time and 2F fade rate. Checked native
destinations include individual, group and broadcast addresses. Building a plan
does no I/O and neither saves bridge configuration nor proves fixture support.

An executor must hold the existing whole-bus permit across both steps and use
the verified HAT repeat policy. The managed API starts with individual
fixtures: freshly read physical/current limits, validate the whole patch, order
crossing minimum/maximum edits to avoid unintended native clamping, then compare
actual readback. Raising minimum can brighten an already-lit fixture; lowering
maximum can dim it. Off remains Off for ordinary dimmers; DT7 limits control a
virtual level and can operate the contact. Such writes must retire affected output,
colour and group cache evidence through the existing owner.

The independent emulator already implements these registers and setters. Literal
model regressions cover all six setters, adjacent repeats and their 100 ms
window, raw special values, retained fade nibbles, individual/group/broadcast
scope, bus isolation and Off versus lit clamping. A standalone process test also
executes typed plans through the real UART/PTY adapter and queries the model.
No timed fades, mains-cycle behavior or physical HAT certification is claimed.
Controls are in `v2-gear-settings-controls.toml` and its measured ledger.
Managed service, MQTT and browser controls are in
`v2-managed-gear-settings-controls.toml`, including an actual browser regression
for preserving numeric edits when another control changes.

Protocol references: [pinned python-dali command source](https://github.com/sde1000/python-dali/blob/4b21c18e79d02d2a97f91d135f0eee450e8c2fb9/dali/gear/general.py)
and [Beckhoff's variable reference](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/5908382347.html).
See [capability status](support-matrix.md) for remaining managed and specialist work.
