# Output control over MQTT

This guide is for MQTT consumers and bridge operators using the development v2
bridge. Any MQTT v5 consumer can use it. A Store server is not required. Configure
the controller and saved outputs first; [discovery](discovery.md) and
[commissioning](commissioning.md) APIs are available through the protected browser
interface as well. See [running v2](running-v2.md) and
[the inventory contract](inventory-contract.md).

For controller `Controller`, bus `Bus1` and output `Light1`, the target is
`Bus1.Outputs.Light1`. Read its current target-generation token from Inventory.
Every write requires that exact token in the MQTT v5 `target-generation` user
property. A rename retires the old target/token. Do not copy a controller token
onto a fixture request.

| Topic | JSON payload | Effect |
|---|---|---|
| `DALI/Desired/Controller/Bus1.Outputs.Light1/PowerState` | `true` or `false` | Recall configured maximum or turn off |
| `DALI/Desired/Controller/Bus1.Outputs.Light1/Brightness` | A number from `0` to `1` | Set DALI control-code position, normalized by 254 |
| `DALI/Desired/Controller/Bus1.Outputs.Light1` | `{"PowerState":true,"Brightness":0.5}` | Validate and apply the complete intent |

Brightness is the DALI control level, not measured luminous flux. Zero means off.
Power true alone recalls maximum; it does not restore an application's remembered
brightness. Nulls, unknown properties, wrong JSON types, out-of-range levels and
contradictory pairs are rejected before any output command is sent. Native
[group power/level commands](group-output.md) and [membership operations](groups.md)
are available. Groups support base scalars and selected DT8 color fields;
see [hardware group color](group-color.md).

BridgeKit coalesces pending Desired values by property. The driver validates and
applies the complete effective map it receives. One rejected map can produce an
Error for each property. Use an explicit refresh/readback to sequence actions;
an Error count alone does not establish completion of a publication.

## Fixture settings

Explicit `read_identity` and bounded `read_memory` commands return captured
read-only reports for individual fixtures and input modules. Exact serials,
version-gated bank0 layouts and raw bytes remain separate from scalar State.
See [device information](device-information.md) for strict parameters, quality
and examples.

`read_luminaire_data` takes no parameters and reads Part251 OEM identity, ratings
and descriptive text from individual saved outputs. Its typed captured fields
include units and unknown/unavailable/invalid status, with exact identifiers as
strings. See [luminaire data](luminaire-data.md) for the supported format and
report. It is a Read command, separate from scalar State.

Common individual fixture settings use explicit `read_output_settings` and
`configure_output` commands, with original target-generation and expected revision
for writes. They return readback reports, separate from scalar Desired/State and
All refresh. See [fixture settings](gear-settings.md) for fields, partial evidence,
physical preflight, special values and unconfirmed outcomes.

## Hardware groups

Saved groups use the same PowerState/Brightness Desired properties and strict
complete-map validation as individual outputs, with the group's own original
target-generation. For example, publish `0.5` at
`DALI/Desired/Controller/Bus1.Groups.Kitchen/Brightness`, or
`{"PowerState":false}` at `DALI/Desired/Controller/Bus1.Groups.Kitchen`.
No inventory revision parameter is needed in Desired. The driver captures one
current revision and rejects a racing change without retokening or retrying.
Selected color fields are also accepted after group representation selection.
Wrong-family and read-only/unknown properties reject the whole group map.
The group's selection is independent of each output's selection; see
[group color State and native preflight](group-color.md).

The bridge sends one native group command. Actual hardware membership determines
which fixtures receive it, including untracked or unaddressed members; saved
checkboxes are not a fanout list. Refresh uses the ordinary command envelope
`{"command":"refresh","target":"Bus1.Groups.Kitchen"}`. A supplied generation
must match; omitted generation binds the current mapping once.

State at `DALI/State/Controller/Bus1.Groups.Kitchen` summarizes individual-address
membership and actual level/status across the complete saved bus roster:

```json
{"Available":true,"Quality":"current","Coverage":"saved_inventory",
 "PowerState":true,"Brightness":null,"PowerMixed":false,"BrightnessMixed":true,
 "KnownMembers":2,"MembershipMatches":true}
```

This group is uniformly On but its actual brightness values differ. No average
is invented. Complete mixed evidence has Available=true; each mixed scalar is
null with its own mixed flag true. Consumers check the property as well as
availability. Coverage is the saved inventory, not a census of every physical
member. MembershipMatches compares actual membership to saved intent; group
control never repairs it. KnownMembers is a lower bound for partial membership.

Empty groups have Available=false/Quality=empty; missing/MASK/collided evidence
is partial or unavailable. Invalidated evidence is stale, and an unsampled mapping
is unobserved. Scalar values and mixed flags are explicitly null whenever no
complete aggregate is usable, clearing older merged State. Stale/unobserved
counts and comparisons are also null. Empty/missing never means Off.

Scalar writes publish accepted actual State before completion. Ignored/clamped
writes still report actual values. Partial/empty/unconfirmed post-write evidence
returns an error after publishing its truthful quality; it does not confirm an
attained level. Native read/Set commands retain their richer per-fixture reports
and sent/observed/unconfirmed outcomes; see [group operations](group-output.md).

Individual, colour, Blink, scene, group-membership and output-address operations
invalidate cached group evidence on their bus before possible mutation. A native
group Set refreshes only its own group; siblings need fresh reads. Inventory
revision changes retire captured roster evidence. Inputs and other buses remain
independent. Group observations survive ordinary read traffic and do not save
configuration. Polling uses the existing fair cursor and total15s ceiling, with
each group clipped to15s and the remaining total; there is no extra poller.

## Colour controls

Select one public colour family with the
[`set_color_representation` metadata command](service-development.md#mqtt-metadata-commands),
then obtain the output's new token from Inventory. The setting is independent of
hardware capabilities; physical writes check support, mode and assignments before
mutation. A format change preserves fixture identity and retires old requests.
Consumers own cleanup of their old retained properties and bindings.

| Selected `color_representation` | Desired property | Value |
|---|---|---|
| `temperature` | `ColorTemperature` | Kelvin; rounded to a representable mirek code |
| `xy` | `ColorX`, `ColorY` | Each coordinate from 0 to 65534/65536 |
| `rgbwaf` | `ColorRed`, `ColorGreen`, `ColorBlue`, `ColorWhite`, `ColorAmber`, `ColorFree` | Each independent channel from 0 to 1 |

Use the same topic shape as Brightness, for example
`DALI/Desired/Controller/Bus1.Outputs.Light1/ColorTemperature` with payload `2700`,
or send a whole target map such as
`{"PowerState":true,"Brightness":0.5,"ColorTemperature":2700}`. Every write needs
the current output token. Only the configured family is writable. Wrong-family,
mixed-family, read-only, null, invalid-type and out-of-range fields reject the
whole request, including any base power/brightness values. BridgeKit may coalesce
scalar requests for one target; a combined invalid batch is refused in full.
Observe its error before testing a later corrected request.

Colour alone leaves power and master brightness unchanged. Partial xy explicitly
masks the other coordinate; partial RGBWAF masks other channels, including WAF.
Zero is a valid channel value. RGB/RGBW fixtures use the assigned subset of the
RGBWAF family; DT8 alone does not promise six channels or RGB support. Normalized
RGBWAF mode and confirmed physical colour assignments are required. The bridge
does not change persistent gear settings or convert between colour spaces.

Combined maps complete capability preflight before either mutation. OFF precedes
the colour plan; positive power/brightness follows it. This orders commands,
without promising that configured fades finish before the next command. A partial
write can have physical effects and returns uncertainty; refresh before deciding
what to send next. There is no automatic retry or rollback.

Use `DALI/Command/Controller` with
`{"target":"Bus1.Outputs.Light1","command":"refresh"}` to request readback.
Supply a v5 response topic for its reply. A refresh may omit target-generation;
an explicitly supplied generation must match. The optional `traceparent` user
property follows the original request through the service and physical readback.
Headerless requests and unsolicited bus invalidations do not inherit old causes.

## Understanding observed State

`DALI/State/Controller/Bus1.Outputs.Light1` contains observed values:

```json
{"Available":true,"Quality":"current","PowerState":true,"Brightness":0.5}
```

Writes query actual level and status before reporting success. A fixture can clamp
the requested brightness or still be fading; State reports its sampled result.
The HAT's ordinary `N` response alone never establishes the requested brightness.
Level and status are sequential samples, not an atomic physical snapshot.

| Quality | Current control values available? |
|---|---|
| `current` | Yes, when Available is true |
| `partial` | No; some evidence is missing or the fixture answered MASK |
| `unavailable` | No usable query evidence |
| `stale` | Earlier evidence was invalidated by bus, UART or mapping changes |
| `unobserved` | No sample for this saved mapping yet |

Gate usable State by `Available`, and by the controller's BridgeKit Active/Status.
BridgeKit merges properties: after an outage, old PowerState/Brightness may remain
in retained State as last-known values with Available=false. They are not a new
off reading. A missing fixture stays in Inventory and does not prevent controlling
another healthy fixture. Available is not a certification that a fixture is
fault-free. Saved inventory alone never proves physical presence.

Individual output State also includes the fixture's reported status:

| Read-only property | Meaning |
|---|---|
| `FixtureStatusRaw` | Native QUERY STATUS byte, 0..255, or null |
| `FixtureStatusQuality` | `current`, `unobserved`, `unavailable` or `stale` |
| `GearFailure`, `LampFailure` | Driver-reported gear or lamp fault |
| `LampOn`, `LimitError`, `FadeRunning`, `ResetState` | Reported operating flags |
| `MissingShortAddress`, `PowerFailure` | Reported address or power condition |

All eight flags are booleans only when FixtureStatusQuality is current; otherwise
raw and flags are explicitly null, clearing previous merged values. A valid
status byte is independently current even when brightness is missing or MASK.
Status FF means all eight bits are set. These properties are rejected in Desired.
A reported fault does not make a responding fixture unavailable or change its
brightness. Check controller availability as well as FixtureStatusQuality before
acting on retained data. Status is sampled on ordinary output reads/writes,
refresh and existing polling; no extra polling loop is introduced. No aggregate
group lamp-failure flag is inferred. See [fixture status](gear-settings.md) for
the compact row indicator and [emulator controls](emulator-web.md) for simulation.

Colour State additionally reports `ColorRepresentation` (the saved selection),
`ActiveColorRepresentation` (the sampled native format), `ColorAvailable`,
`ColorQuality`, `ColorCapabilities` and the nine colour scalar properties above.
Only the selected active family has usable numbers. Other, incomplete or stale
values are explicitly null, clearing old values in BridgeKit's merged State.
Gate colour values by ColorAvailable as well as controller availability. Base
Available/Quality remain independent; an incidental colour-query failure does
not invalidate a successful observed base-only write.

ColorQuality is `current`, `unconfigured`, `unobserved`, `stale`, `unavailable`,
`unsupported`, `partial`, `representation_mismatch` or `unsupported_mode`.
An external DALI master can change the active format; the bridge reports that
evidence without changing the saved public selection or inventing conversions.
An actual sample can reflect clamping or an unfinished fade, and is never an
echo of the requested value. For example, a 2000 K request clamped to 400 mirek
reports 2500 K.

ColorCapabilities is null when no fresh matching observation exists. Otherwise
it contains nullable `dt8`, `supported_representations`, `primary_count`,
`rgbwaf_channel_count`, ordered `rgbwaf_assignments`, raw `rgbwaf_control`,
`automatic_activation` and `temperature_limits`. Unknown/malformed evidence
remains null. A confirmed non-DT8 fixture has an empty supported-format list.
Supported formats can include `primary_n`, whose control is still pending;
capability is distinct from currently writable modes. Assignment entries are
colour names or null for unknown, unassigned, MASK or failed replies. The four
limit fields are `coolest_kelvin`, `warmest_kelvin`, `physical_coolest_kelvin`
and `physical_warmest_kelvin`; each is a known positive actual limit or null.

Plain target reads/writes have a 2 s service budget. Configured or already sampled
colour mappings use 6 s for reads and base-only writes; colour/combined writes
use 8 s for preflight, mutation and fresh readback. These budgets are selected
before admission; explicit library caller deadlines are unchanged. Background
polling runs every 30 s, with a 15 s aggregate ceiling and each fixture clipped
to the remaining total and its 2 s/6 s read budget. Large/incomplete
passes report an error while keeping completed samples, and the next pass resumes
fairly from a persistent cursor. Disabled buses are not queried. Without a ready
UART, background reads reject before taking physical admission; metadata remains
accessible. Busy or expired operations can fail. A canceled frontend cannot undo
a command already sent: inspect observed state before retrying an uncertain write.

Outage behavior: a Desired request consumed while a fixture's bus
or UART is unavailable is rejected. HAT/bus recovery alone does not retry it,
including retained Desired consumed before startup readiness. A later broker
reconnect may replay retained property intent. Dropping commands received during
such rare outages is an accepted policy. Metadata and healthy fixtures remain
accessible during that outage.

These paths are tested with isolated brokers and independent PTY fixtures. Physical
I4 firmware and fixture validation remain pending; see the
[support matrix](support-matrix.md).

Explicit `read_output_settings` / `configure_output` replies also provide optional
DT7 relay capabilities and native thresholds; see [fixture settings](gear-settings.md).
Unknown support remains null. These capability replies do not change retained
output components or scalar property names. Actual PowerState/Brightness continue
to describe contact0/254; virtual limits/thresholds are configuration values.
