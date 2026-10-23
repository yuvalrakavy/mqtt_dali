# Diagnostic and maintenance captures

The bridge reads standard Part253 data from an individual saved fixture. Driver
and light-source banks provide operating hours, start/failure counts, voltage,
current and temperature. The maintenance bank provides manufacturer ratings.
These are optional device capabilities; a missing reply does not establish why
data is unavailable. Existing ordinary fixture status flags remain separate.

## Browser workflow

Open **⋮ → Device information** and expand **Diagnostics and maintenance**.
Choose **Driver**, **Light source** or **Luminaire maintenance**, then press
**Read diagnostic data**. Changing the selector does not read the fixture.
Driver/light-source results confirm a captured snapshot was released. Static
maintenance ratings have no snapshot claim. Each field has a label, unit and
value or an explicit status; boolean failure conditions show **Yes** or **No**.

Temperatures are Celsius, including negative values. Time counters are seconds;
rated life is hours. Counters and fractional measurements retain exact decimal
values. **Raw diagnostic values** and **Captured bank … bytes** expose the
underlying evidence without shifting missing bytes.

**Read protected**, **Unknown**, **Temporarily unavailable**, **Unavailable** and
**Invalid data** have distinct meanings. Protected fields are not automatically
unlocked. Unknown format versions show the raw prefix without guessed fields.
An invalid or missing failure value never becomes **No**.

A new or failed read clears the previous result. Closing, sign-out, navigation
or a configuration change retires a pending dialog. The selected bank and open
controls remain available for explicit retry. Input modules/instances and groups
do not offer this action. The bridge does not poll or retry these captures.

## MQTT and API

Publish a BridgeKit Read command to `DALI/Command/Controller` with an MQTT v5
Response Topic, for example:

```json
{"command":"read_diagnostics","target":"Bus1.Outputs.Kitchen","bank":205}
```

The only parameter is integer `bank`, limited to205,206,207. Protected
`/api/jobs` uses the same command with `{"bank":205}` as `params`; typed callers
use `ServiceHandle::read_diagnostics` and `dali::diagnostics::Bank`. Original
generation, revision, trace and deadline rules apply. No mutation completion or
`expected_revision` parameter is required.

| Bank | Contents | Maximum captured bytes | Snapshot |
|---|---|---|---|
|205|Driver diagnostics,19 fields|29|Verified and released|
|206|Light-source diagnostics,17 fields|33|Verified and released|
|207|Luminaire maintenance ratings,3 fields|8|Static; no latch|

Banks205/206 require `--uart-repeat-terminals 1` or `2`, matching the HAT. An
unverified repeat profile refuses these actions before native traffic. Bank207,
identity, luminaire and raw-memory reads remain usable under that policy.
Each bank is an independent capture; several reads are not one atomic snapshot.

The common memory report has operation `read_diagnostics`, start0, the selected
bank, exact raw readings, quality and `configuration_changed:false`. Envelope
`identity`/`version` are null. Optional `diagnostics` contains bank `version`,
`format_status`, `fields` and snapshot evidence. An example field is:

```json
"temperature":{"status":"value","raw":35,"value":"-25","unit":"°C"}
```

`value` is an exact decimal string for measurements/counts, a JSON boolean for
failure conditions, and null for every nonvalue status. `raw` retains the
unsigned counter/code if all constituent bytes were read. Units are `s`,
`count`, `Vrms`, `Hz`, `%`, `V`, `A`, `°C`, `h`, or null for booleans/power factor.
Temperature uses unsigned raw minus60; raw128 means68°C. Percentage/power-factor
raw values above100 are invalid. Boolean raw2 is invalid, not true.

All fields support temporary-unavailable TMASK (allFF endingFE). Diagnostic
allFF means `read_protected` only where the standard defines MASK; otherwise
it is invalid. Maintenance allFF means `unknown`. Unreadable/colliding bytes
mean `unavailable`. The bridge does not replace these with zero or false.

Unknown/unreadable versions leave `fields` empty and `snapshot:null`. Supported
banks205/206 require the same AA/AA/FF evidence as [energy data](energy-data.md).
Bank207 always has `snapshot:null`. Complete quality means every requested byte
responded and applicable snapshot proof succeeded, including validly received
MASK/TMASK codes. A truncated header, holes or unknown layout gives partial;
a silent header gives unavailable. Other report families omit `diagnostics`.

## Native behavior and emulator

The existing memory owner retains one permit and the original15s deadline,
revision, generation and physical stamp. A supported dynamic bank first proves
an observedFF baseline, then freshAA, bounded payload, stableAA and releasedFF.
Only temporary latch location2 receives AA/FF. Cleanup uses that same link and
lifetime, including failed evidence. Expiry/teardown cannot guarantee release
beyond the original lifetime; the next explicit read establishes a fresh latch.
Static bank207 sends no latch writes.

No counter reset,55 unlock, manufacturer programming, new connection, persistent
journal, cache or poller is added. Reads preserve saved names/bindings and native
levels, colors, settings, groups and scenes. Captured historical failure counts
do not overwrite ordinary retained fixture status.

Load [the diagnostic scenario](../examples/emulator/diagnostics-i4.json) in the
separate emulator. The JSON editor configures raw bytes. Explicit `latch:true`
is accepted on output banks205 up to29 bytes and206 up to33 bytes, alongside
energy banks202–204 up to16 bytes. Static207 and input modules cannot latch.
Runtime frozen data and write-enable stay separate from installation profiles;
scenario capture/export saves configured data and capability. Values remain
configured observations; aging, thermal dynamics and accumulating counters are
not simulated.

Local verification and physical validation status are tracked in
[the progress ledger](v2-progress.md). Physical HAT/fixture validation is pending.

Primary definitions: [diagnostics](https://github.com/sde1000/python-dali/blob/master/dali/memory/diagnostics.py),
[maintenance](https://github.com/sde1000/python-dali/blob/master/dali/memory/maintenance.py),
[memory values/latches](https://github.com/sde1000/python-dali/blob/master/dali/memory/location.py).
