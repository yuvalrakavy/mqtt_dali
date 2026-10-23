# Device identity and read-only memory

The bridge can read the standard product identity and bounded memory-bank ranges
from an individual saved fixture or input module. No special Store consumer is
required. These are explicit captured reports, separate from scalar MQTT State.
Reads do not rename devices, replace their bindings, program memory or change
light levels, colors, groups, scenes or input settings.

## In the bridge browser

Open the controller's bus, then **⋮ → Device information** on a fixture or input
module. The password-protected dialog reads identity immediately and shows GTIN,
serial number, firmware/hardware versions, the identity layout and read quality.
**Read identity again** takes a fresh capture. Input instances share their parent
module's product identity, so this action appears on the module row.

Expand **Read a memory bank** to choose a decimal bank number, first location and
byte count, then press **Read memory**. The result lists hexadecimal locations
and exact hexadecimal/decimal byte values. No response and receive/transmit
collisions remain separate readings. An unsupported bank may simply be silent;
silence alone does not prove why a device did not answer.

Banks are 0–255. Read 1–64 bytes at a time within locations 0–254; a range cannot
wrap. Bank 0 contains identity information. Other banks depend on the product
and supported DALI parts. **Read luminaire data** on output fixtures interprets
supported Part251 bank1 data; see [luminaire identity and ratings](luminaire-data.md).
Expand **Energy and power** for explicit, snapshotted [Part252 reads](energy-data.md)
on individual fixtures.

The report belongs to the original device and configuration revision. Close,
sign-out, navigation, rename, address changes or replacement retire a pending
dialog. A failed new read clears the previous displayed report. Refresh or
reopen after a configuration change.

## MQTT and authenticated jobs

Publish one of these JSON envelopes to `DALI/Command/Controller`, with an MQTT v5
Response Topic if a reply is needed:

```json
{"command":"read_identity","target":"Bus1.Outputs.Kitchen"}
```

```json
{"command":"read_memory","target":"Bus1.Inputs.WallPanel","bank":255,"start":0,"count":4}
```

Only individual saved output and input-module targets are accepted. Groups,
scenes and instances are refused. `read_identity` has no parameters;
`read_memory` requires all three integer parameters and rejects unknown fields.
Both are BridgeKit Read commands. An omitted `target-generation` binds the
current mapping once; a supplied token must match the selected target. Replies
preserve the request's original trace and header absence. They do not need a
mutation-completion token or an `expected_revision` parameter.

The successful BridgeKit reply's `value` contains:

```json
{
  "operation":"read_memory","outcome":"observed",
  "target":"Bus1.Inputs.WallPanel","target_generation":"<epoch>.1",
  "revision":1,"bus":0,"address":0,"space":"inputs",
  "bank":255,"start":0,"count":4,"quality":"partial",
  "version":null,"identity":null,"reason":null,"configuration_changed":false,
  "readings":[
    {"address":0,"reading":{"kind":"value","raw":255}},
    {"address":1,"reading":{"kind":"no_response"}},
    {"address":2,"reading":{"kind":"value","raw":0}},
    {"address":3,"reading":{"kind":"value","raw":127}}
  ]
}
```

Raw `FF` is numeric data here, never a global MASK. Each location has its own
reading. A collision uses `receive_collision` or `transmit_collision` with a
nullable `code`; `reading:null` means unvisited. Transport failure, expiry or
retired evidence fails the operation rather than accepting an old capture.

`complete` means every requested location answered with a byte; it does not
certify a product identity. `partial` includes holes, an unknown identity layout
or a truncated/malformed bank0 header. `unavailable` means no requested byte was
read. A silent bank0 header produces no decoded identity. Reserved header `FF`
is preserved with partial quality and an explanatory reason.

Identity reports include the raw DALI version reading and a nullable `identity`
object with `layout`, `gtin`, `serial_number`, `serial_bits`, `firmware_version`,
`hardware_version` and `last_memory_bank`. GTIN is a padded 14-digit decimal
string, without check-digit or manufacturer validation. Serial is exact-width
uppercase hexadecimal: 8 characters for legacy 32-bit identity, 16 for modern
64-bit identity. Firmware/hardware versions are separate `[major,minor]` pairs.
Missing or all-FF multi-byte identity fields decode to null; raw bytes remain.
Single-byte last-bank `FF` remains 255.

Raw version 1 selects the older bank0 layout through location `0E`; modern
encodings 8–254 select through `1A`. Unknown/reserved versions read only common
locations through `0A` and leave serial/hardware unknown. The bank's own last
location limits every identity read. No universal checksum is assumed for byte1.
Physical identity never replaces the bridge's saved name or Origin.

Authenticated `/api/jobs` uses the same command names, target/generation and
strict parameters inside the normal job envelope. Session, Origin and CSRF
requirements apply to admission and result access. See [web access](web-access.md).
Typed embedders use `ServiceHandle::read_identity` or `read_memory` with a checked
`dali::memory::Window`, original context and deadline.

## Emulator configuration

In the emulator's scenario JSON editor, add this optional field to an output
or input module, then apply the scenario:

```json
"memory": {
  "version":8,
  "banks":[
    {"bank":0,"bytes":[26,0,255,0,1,2,3,4,5,6,7,254,220,186,152,118,84,50,16,2,3,0,0,0,0,0,0]},
    {"bank":255,"bytes":[255,null,0,127]}
  ]
}
```

This produces a modern identity with serial `FEDCBA9876543210`, firmware6.7 and
hardware2.3; bank255 exercises FF, a silent location and zero. `version` defaults
to8. At most16 unique banks are allowed, with1–255 nullable bytes per bank.
Arrays are indexed by location. Headers are explicit data, permitting legacy,
unknown, truncated and malformed test cases. No identity is inferred from the
emulator ID. Existing scenarios can omit `memory` and receive no memory reply.
The output's preexisting default version reply remains8; an input without a
profile retains its prior version silence.

Reads affect only scratch DTRs. Capture, export, reboot/reset and addressing
retain the configured memory profile. Existing emulator controls and the bridge
dialog can be open in separate tabs; see [emulator browser](emulator-web.md).

## Contributor notes and limits

One existing physical owner holds each whole read under a15-second absolute
cap. It selects DTR1 once and explicitly reloads DTR0 per byte, so a missing
reply cannot shift later locations. Output and input address spaces use distinct
16/24-bit native commands. Reads need no repeat-terminal profile, do not cut
output/group/input/feedback observations and create no poller or identity cache.
Final acceptance checks the original mapping, revision, deadline and physical
stamp. DTR scratch operations are serialized with other bridge work.

Local evidence covers literal UART bytes, the independent TCP emulator, MQTT,
typed operations, strict protected jobs and real phone/desktop browsers. Physical
HAT/fixture validation remains pending. [Part251 luminaire decoding](luminaire-data.md)
reuses this owner, as do [Part252 energy snapshots](energy-data.md). Generic memory
writes, Part253 and manufacturer-specific layouts are separate capabilities.

Primary protocol references: [python-dali bank0 layouts](https://github.com/sde1000/python-dali/blob/master/dali/memory/info.py),
[Part102 commands](https://github.com/sde1000/python-dali/blob/master/dali/gear/general.py),
[Part103 commands](https://github.com/sde1000/python-dali/blob/master/dali/device/general.py).
