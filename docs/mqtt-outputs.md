# Individual output control over MQTT

This guide is for MQTT consumers and bridge operators using the development v2
bridge. Any MQTT v5 consumer can use it. A Store server is not required. Configure
the controller and saved outputs first; [discovery](discovery.md) and
[commissioning](commissioning.md) APIs are available, with the browser interface
under development. See [running v2](running-v2.md) and
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
are available; group scalar Desired and colour remain subsequent work. Individual
DT8 colour controls are described below.

BridgeKit coalesces pending Desired values by property. The driver validates and
applies the complete effective map it receives. One rejected map can produce an
Error for each property. Use an explicit refresh/readback to sequence actions;
an Error count alone does not establish completion of a publication.

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
