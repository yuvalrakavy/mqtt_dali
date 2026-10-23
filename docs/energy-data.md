# Captured energy and power

The bridge can read Part252 energy/power data from an individual saved output.
These are explicit captures, separate from scalar MQTT State and the static
ratings in [luminaire data](luminaire-data.md). A generic MQTT consumer can use
them; no Store server is required. Reads preserve names, bindings, fixture
settings, levels, colors, groups and scenes.

## Browser workflow

Open **⋮ → Device information** on a fixture and expand **Energy and power**.
Choose **Active input**, **Apparent input** or **Load-side active**, then press
**Read energy data**. Changing the selection does not read the fixture. The
result shows captured energy and power with units and confirms that the snapshot
was released. **Counter details** shows the raw mantissa and decimal scale;
**Captured bank … bytes** shows exact readings.

Large and fractional values retain their exact decimal form. A genuine zero
stays zero. **Temporarily unavailable**, **Unavailable** and **Invalid data**
remain separate. Unknown bank versions show the version and raw prefix without
interpreted values. Silence alone does not prove that a device lacks the bank.
Input modules and instances do not offer this action.

Closing, navigating away, sign-out or a configuration change retires a pending
dialog. A failed/new read clears the old capture. The selected bank and expanded
controls remain available for an explicit retry; the bridge does not retry a
capture automatically.

## MQTT and API

Publish to `DALI/Command/Controller` with an MQTT v5 Response Topic:

```json
{"command":"read_energy_data","target":"Bus1.Outputs.Kitchen","bank":202}
```

The only parameter is integer `bank`, limited to202,203,204. Groups, scenes,
input modules and instances are refused. This is a BridgeKit Read command;
there is no mutation-completion requirement or `expected_revision` parameter.
Original target-generation, revision, trace and deadline rules apply. Protected
`/api/jobs` uses the same command with `{ "bank":202 }` as `params`. Typed callers
use `ServiceHandle::read_energy_data` with `dali::energy::Bank`.

Snapshot reads require a configured, known HAT repeat-terminal profile:
`--uart-repeat-terminals 1` or `2`, matching that HAT's behavior. An unverified
profile refuses before sending native commands. Identity, luminaire and raw
memory reads remain usable under the unverified profile.

| Bank | Energy | Power |
|---|---|---|
|202, active input|`Wh`|`W`|
|203, apparent input|`VAh`|`VA`|
|204, load-side active|`Wh`|`W`|

Each bank is a separate snapshot. Sequential reads of several banks are not one
atomic measurement. The response uses the common memory-report envelope with
operation `read_energy_data`, selected bank, start0 and at most16 raw locations.
`version`/`identity` in that envelope are null. Its optional `energy` member has
bank `version`, `format_status`, two `fields` and snapshot evidence, for example:

```json
{
  "version":1,"format_status":"supported",
  "fields":{
    "energy":{"status":"value","raw":281474976710653,"scale":6,
              "value":"281474976710653000000","unit":"VAh"},
    "power":{"status":"value","raw":4294967293,"scale":-6,
             "value":"4294.967293","unit":"VA"}
  },
  "snapshot":{
    "start":{"kind":"value","raw":170},
    "end":{"kind":"value","raw":170},
    "release":{"kind":"value","raw":255}
  }
}
```

Scaled `value` is a decimal **string**, including integer results, to avoid
floating-point rounding. `raw` is the exact six-byte energy or four-byte power
mantissa; `scale` is a signed decimal exponent. Trailing fractional zeros are
normalized; the original scale remains available. Only status `value` has a
nonnull scaled value. `temporarily_unavailable` is the defined TMASK;
`unavailable` means required bytes could not be read; `invalid` means an invalid
scale or mantissa. Diagnostic raw/scale values can remain present on these fields.

Energy fields support TMASK, not a general MASK: allFF endingFE is temporarily
unavailable, whereas an allFF mantissa is invalid. ScaleFF is valid−1, andFA is
valid−6; scales outside−6..6 are invalid. Missing/colliding bytes retain their
locations and cannot become zero or shift subsequent fields.

Unknown/unreadable versions leave `fields` empty and `snapshot:null`.
`complete` means every requested byte answered and snapshot evidence was confirmed;
individual fields can still contain TMASK/invalid data. Holes, a truncated bank or
unknown version give `partial`. A silent header gives `unavailable`; reserved
headerFF gives `partial` with a reason. Identity/raw/luminaire reports omit `energy`.

## Snapshot protocol and emulator

The existing memory owner holds one permit and the original15s maximum deadline.
Header and version1 proof bound all reads. The bridge establishes an observedFF
baseline, writes temporary latchAA, verifies it, reads the bounded payload, checks
that the latch stayedAA, releases withFF and verifies release. It reloads DTR0
per location and re-enables writes before release because memory reads can clear
write enable. Failed snapshot/release evidence does not become accepted values.

Only the temporary snapshot control byte is written; there is no generic memory
programming or55 unlock. Cleanup uses the same link, stamp and deadline, including
failed latch evidence. Expiry/owner teardown cannot guarantee release beyond that
lifetime. A later explicit capture establishes a freshFF→AA transition, so a
previous frozen snapshot cannot satisfy a new read. No journal, power-cycle
requirement, extra connection, retry, poller or retained energy cache is added.

Load [the energy scenario](../examples/emulator/energy-i4.json) in the separate
emulator browser. Its three banks exercise fractional active readings, large
apparent counters and TMASK/zero load-side readings. In the scenario JSON editor,
add `"latch":true` to a standard energy bank to opt into snapshot behavior.
Latching is limited to output banks202–204 with at most16 bytes. Other banks and
input profiles remain read-only. Existing scenarios can omit this flag.

Configured raw bytes remain installation data. The emulator's runtime latch and
write-enable state are separate; location2 reportsAA/FF according to that latch.
Capture/export preserves the profile and capability, without saving a frozen
runtime copy. Scenario reset starts fresh scratch state; HAT transport reboot
retains native device state under the existing emulator rules. Values stay at
their configured levels until edited; energy-consumption simulation is not implied.

Local verification covers independent literal/model behavior, both HAT terminal
profiles, generic MQTT, protected jobs and real phone/desktop browsers. Physical
HAT/fixture validation remains pending. Part253 diagnostics and maintenance are
tracked separately.

Primary sources: [energy definitions](https://github.com/sde1000/python-dali/blob/master/dali/memory/energy.py),
[memory values/latches](https://github.com/sde1000/python-dali/blob/master/dali/memory/location.py),
[Part102 commands](https://github.com/sde1000/python-dali/blob/master/dali/gear/general.py).
