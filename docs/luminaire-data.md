# Luminaire identity and ratings

Part251 data describes the complete luminaire: its manufacturer's identifiers,
electrical ratings, light output and descriptive text. Driver identity remains
available separately under [Device information](device-information.md). These
are explicit captured reads; they do not become scalar MQTT State or change
saved names, descriptions, bindings or fixture settings.

## Read from the browser

On an individual fixture, open **⋮ → Device information**, then press **Read
luminaire data**. The protected dialog shows each field with its unit and an
explicit **Unknown**, **Unavailable** or **Invalid data** indication where needed.
Expand **Captured bank 1 bytes** to inspect the evidence. **Read identity again**
returns to driver identity; either read replaces the previous capture.

An unknown format displays its format number and raw prefix without guessing
field meanings. A device without this bank can remain silent. A successful read
does not prove that every manufacturer field is populated. The action is offered
only on fixture rows, not input modules or their instances.

## MQTT and typed API

Send this Read command to `DALI/Command/Controller`, with an MQTT v5 Response
Topic to receive the report:

```json
{"command":"read_luminaire_data","target":"Bus1.Outputs.Kitchen"}
```

It accepts no parameters. Individual saved outputs are required; groups, scenes,
input modules and instances are refused. The common original-generation,
revision, trace and deadline rules from [device information](device-information.md)
apply. Authenticated `/api/jobs` uses the same command and strict parameters.
Typed callers use `ServiceHandle::read_luminaire_data(target, context, deadline)`.

The reply's `value` has the ordinary memory-report fields, with operation
`read_luminaire_data`, bank1, start0, null `version`/`identity`, and a `luminaire`
object. For a supported format, that object's `fields` contains all fourteen
keys below. For example:

```json
{
  "format_id":3,
  "format_status":"supported",
  "fields":{
    "luminaire_id":{"status":"value","value":"FEDCBA9876543210","unit":null},
    "input_power_nominal":{"status":"value","value":42,"unit":"W"},
    "cri":{"status":"unavailable","value":null,"unit":null},
    "cct":{"status":"value","value":"part_209","unit":null}
  }
}
```

This example shows four of the fourteen fields. Status `value` alone has a
nonnull value. `unknown` is the field's defined unknown value/range;
`unavailable` means required bytes were not readable; `invalid` means malformed
encoding or a reserved value. Raw readings preserve exact locations, FF, silence
and collision directions/codes. Identity and raw-memory reports omit `luminaire`.

| Field key | Value when available |
|---|---|
| `manufacturer_gtin` | Padded14-digit decimal string; no check-digit validation |
| `luminaire_id` | Exact16-character uppercase hexadecimal string, including leading zeros |
| `manufacture_year` | Two-digit year0–99; no century assumed |
| `manufacture_week` | Week1–53 |
| `input_power_nominal` | Integer watts (`W`) |
| `input_power_minimum_dim` | Integer watts at minimum level (`W`) |
| `mains_voltage_minimum` | Minimum AC voltage rating90–480 (`V`) |
| `mains_voltage_maximum` | Maximum AC voltage rating90–480 (`V`) |
| `light_output_nominal` | Integer lumens (`lm`) |
| `cri` | Color rendering index0–100 |
| `cct` | Temperature0–17000 (`K`), or `part_209` with null unit |
| `light_distribution` | `not_specified`, `type_i`, `type_ii`, `type_iii`, `type_iv`, `type_v` |
| `oem_color` | Up to24 ASCII characters describing the product finish |
| `identification` | Up to60 ASCII characters |

The Part209 CCT indication is not65534K and does not prove the fixture supports
DT8 control. OEM color text describes the product, not the emitted-light color.
Text ends at the first NUL; empty text means unspecified. Missing bytes before
that terminator make the field unavailable. Non-ASCII text, including allFF, is
invalid; text does not use a numeric MASK. DistributionFF is unknown, whereas
reserved codes6–254 are invalid. Numeric allFF fields are unknown, never maxima.

## Format bounds and quality

The bridge reads bank1's header and prefix through location0x12. Only content
format0x0003 selects the supported Part251 layout and permits extension through
0x77, under the same physical owner. The header's last location bounds both
stages. Unrecognized or unreadable format IDs leave `fields` empty and preserve
only the prefix; the bridge does not guess a layout.

The internal capture is at most120 bytes. Public `read_memory` still accepts at
most64 bytes. `complete` means all requested bytes answered; field-level unknowns
can remain. Holes, an unknown format or truncated supported layout give `partial`.
A silent header gives `unavailable`; reserved headerFF gives `partial` with a
reason. Captures retain the original mapping/revision/physical stamp and a15s
absolute cap. Each location explicitly reloads DTR0. No memory writes, unlock,
latch, retries, polling or retained data cache are added.

## Emulator and limits

Load [the luminaire scenario](../examples/emulator/luminaire-i4.json) in the
separate emulator browser, then discover its output from the bridge. Its optional
raw `memory.banks` bank1 array contains format3, exact IDs,42W,1000lm,3000K and
descriptive text. Use the scenario JSON editor to replace bytes with null for
silence, FF for unknown numeric data, or an unfamiliar format ID. Capture/export,
reboot and reset preserve these profiles under their existing rules.

Local checks cover literal UART, an independent TCP model, generic MQTT, strict
jobs and real phone/desktop browsers. Physical HAT/fixture validation remains
pending. Parts252/253 energy and diagnostic data, memory programming and
manufacturer-specific layouts are separate capabilities.

Primary reference: [python-dali Part251 definitions](https://github.com/sde1000/python-dali/blob/master/dali/memory/oem.py)
and [memory value decoding](https://github.com/sde1000/python-dali/blob/master/dali/memory/location.py).
