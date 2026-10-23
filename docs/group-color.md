# Control a DT8 hardware group's color

Open **controller/Bus1 → Groups**, select a saved group and choose **Inspect color
support**. The Color card shows representations supported by every actual member
in the saved bus roster. Select White temperature, CIE xy or RGBWAF channels,
then open **Group color values**. Changing the selection saves metadata and
refreshes State; it does not change the lights' operating mode or color.

The dialog shows actual uniform values, Mixed or Unknown. Enter only the values
you want to change. **Read current values** refreshes observations without
replacing your draft. **Apply group color** sends a native group command and
reads the fixtures back. A color-only change preserves power and brightness.
Values may differ from the request because of native rounding or fixture limits.
An incompatible member disables color controls. Stop Blink before changing color.

## MQTT commands and Desired

Use `DALI/Command/Controller` with an MQTT v5 response topic. Reads can omit
`target-generation`; supplied tokens must match. Writes require the saved group's
original token and the current inventory revision.

```json
{"command":"read_group_color","target":"Bus1.Groups.Kitchen"}
```

```json
{"command":"set_color_representation","target":"Bus1.Groups.Kitchen",
 "expected_revision":23,"color_representation":"temperature"}
```

Selection preserves the group's identity and members but retires its old binding
token. Get the new token from Inventory before controlling it. Null clears the
selection. Individual outputs' selections and tokens are independent.

```json
{"command":"set_group_color","target":"Bus1.Groups.Kitchen",
 "expected_revision":24,"values":{"ColorTemperature":4000}}
```

The `values` map uses the same [selected color fields and units](mqtt-outputs.md#colour-controls)
as individual DT8 outputs. It must contain a color value; optional PowerState and
Brightness follow existing validation and command ordering. Wrong-family, null,
unknown/read-only and invalid values reject the entire map. Existing
`set_group_output` remains power/brightness only.

Group Desired also accepts selected Color* scalars or complete maps, for example
`DALI/Desired/Controller/Bus1.Groups.Kitchen/ColorTemperature` with payload `4000`.
Desired needs the group's token; the driver captures the current revision once.
Rust callers use `read_group_color` / `set_group_color` and typed `ColorIntent`.

## Read the actual State

MQTT group State and protected live snapshots carry ColorRepresentation,
ColorAvailable, ColorQuality, ActiveColorRepresentation, ColorCapabilities and
the nine Color* scalars. Every scalar has a corresponding Color*Mixed flag:

| Evidence across actual known members | Scalar | Mixed flag |
|---|---|---|
| Complete and equal | Actual value | false |
| Complete and different | null | true |
| Unreadable, unsupported or outside the selected family | null | null |

Color availability is independent of base availability: an actual-level MASK
does not erase complete color evidence. ActiveColorRepresentationMixed similarly
distinguishes mixed active formats from unknown formats. Missing membership,
empty groups, stale captures and retired generations cannot supply current
scalar values. Explicit nulls clear prior merged State.

Capabilities contain only common native evidence: DT8 support, intersected
supported_representations, common_rgbwaf_assignments and intersected temperature
limits. Unknown evidence stays null. There is no invented group physical-channel
count or shared RGBWAF control byte. ColorQuality includes current, unconfigured,
unobserved, stale, partial, empty, unavailable, unsupported, unsupported_mode and
representation_mismatch. Current color may still be mixed.

## Native behavior and limits

The bridge reads actual membership across the complete saved bus roster, then
preflights every known hardware member before changing color or power. It uses
one native group staging/ACTIVATE sequence; it does not send individual color
writes. Partial xy/RGBWAF requests mask untouched coordinates/channels. Unknown
membership, an empty group, unsupported DT8/features/channels or unnormalized
RGBWAF mode refuses the whole write. Scratch DTR queries can still occur.

Coverage is `saved_inventory`: untracked or unaddressed members also receive the
hardware group command, and cannot be certified by this roster. Verify assignments
and discover devices before operating an existing installation.

All work shares the existing fifteen-second group budget and original physical
stamp, context and authority. Slow/large rosters may be incomplete. Fresh readback
is required after mutation. Aborted/expired operations can return unconfirmed and
leave previous evidence stale; explicitly read again before retrying. No automatic
retry or rollback is added. Base-only writes clear previous color observations;
configured group reads can refresh color. `read_group_color` inspects support even
before selection.

Command reports remain the [membership/base envelope](group-output.md); color
results are in State and the typed Observation's native color samples. HTTP job
results are not color State. Local evidence uses the independent TCP emulator,
MQTT, protected jobs and real phone/desktop Chrome. Physical validation is pending.
