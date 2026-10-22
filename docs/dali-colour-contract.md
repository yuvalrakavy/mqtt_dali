# DT8 colour protocol codec

`dali::colour` provides typed Part 209 values and complete 16-bit frame plans.
It performs no UART, MQTT, persistence, discovery or emulator operations. The
canonical daemon currently exposes the base output and live input MQTT paths;
the library service also supports owned colour operations described below. The
[independent HAT emulator](emulator.md#colour-fixtures) now provides selected
physical DT8 semantics for later integration tests; it does not import this codec.

## Values and independent representations

| Type | Raw domain | Conversion |
|---|---|---|
| Temperature | 1..65534 mirek | Kelvin = 1,000,000 / mirek |
| Coordinate | 0..65534 | CIE xy coordinate = raw / 65536 |
| Channel | 0..254 | Normalized channel = raw / 254 |

Constructors reject MASK and invalid/nonfinite values. Float inputs are validated
before nearest-code rounding; unrepresentable values are rejected rather than
clamped into range. Channel zero remains valid, including positive values that
round to zero. These channels are independent of master brightness and do not
inherit the base brightness codec's minimum-on step.

For example, `Temperature::from_kelvin(2700.0)` gives 370 mirek, representing
approximately 2703 Kelvin. `Coordinate::from_normalized(0.5)` gives 32768.
`Channel::from_normalized(1.0)` gives 254, never MASK.

`Intent::temperature`, `Intent::xy` and `Intent::rgbwaf` preserve the requested
representation. RGBWAF order is red, green, blue, white, amber, free colour.
Each optional channel is either a validated value or unchanged MASK; an entirely
unchanged intent is rejected. Sending RGB does not clear WAF. Plans do not
select a new RGBWAF control mode, alter power or master brightness, or convert
RGB into xy. The service discovers capability, active mode and limits before
accepting an operation. DT8 alone does not imply RGB support.

## Owned service operations

`ServiceHandle::read_colour` and `apply_colour` use the same capacity-one output
owner as base output operations. `apply_colour_with_output` additionally accepts
a validated base intent within that same owned operation. Supply the named
individual output, original
request context and absolute deadline. Writes require the current target
generation. These library APIs do not add MQTT properties or browser controls.
A whole operation owns DTRs and device-type selection through actual helper
settlement, including when its frontend is cancelled. Native input events keep
flowing through the existing UART reader. No UART recovery behavior is added.

The service reads actual level/status, discovers device type 8, and queries colour
features/status. Multiple device types are enumerated without intervening frames,
with a strictly ascending prefix and at most nine next-type queries before
finding or passing 8. Unknown replies do not establish capability. RGBWAF channel
counts are separate from physical channel roles: QUERY ASSIGNED COLOUR identifies
each supported output channel, retaining unassigned, MASK and unknown bytes.

Colour writes require a fresh successful gear-features reply and accept either
manual or automatic activation. Unknown/missing/collided features reject before
setters; persistent gear settings are never changed implicitly. RGBWAF writes
additionally require normalized control mode and a
confirmed assignment for every requested logical colour. Unsupported modes and
representations are rejected with actual base evidence retained. MQTT colour
properties and public representation selection remain subsequent work.

A combined operation validates all colour capabilities before either mutation.
It sends an off intent before the colour plan, or the colour plan (including
ACTIVATE) before a positive base intent. Explicit OFF is immediate; zero DAPC
retains configured fading. This orders commands without promising darkness or
completion of a colour/level fade. Every successful combined operation requires
fresh usable base and requested-representation readback.

No DALI write is promised atomic or rolled back. An interrupted operation returns
`Uncertain`; some commands may already have taken effect. With automatic activation,
staged colour from an interrupted operation can also become visible at a later
power command. The bridge does not add retries, cleanup commands or gear-setting
workarounds. Refresh actual state before deciding what to send next.

Successful writes include new actual base/status/colour reads. The active
representation must match the requested type, with usable actual values; intended
values are never substituted. A clamped Tc can succeed and retains the range bit
and supported physical/configured limits. A different complete active type is
reported as actual evidence but returns `Uncertain`. A current sample during a
fade does not promise that the fade has finished or that a requested value was
reached exactly. Primary-N is reported but cannot be controlled by these APIs.

`output::Observation::colour` contains separate colour quality, DT8 presence,
raw device-type replies/prefix, gear/type/status/control replies, physical channel
assignments and a map of typed actual values. `raw_values` retains each value
plan's replies in order, including its leading actual-level query, high/byte reply
and (for a successful word high) low byte. No missing byte becomes zero. The
`active()` and `value()` accessors suppress stale evidence; raw fields remain for
diagnostics under their recorded quality. The outer observation carries the one
original context and UART/bus stamp for the entire sample.

Once colour has been sampled at a target, subsequent base reads/writes include
fresh colour samples instead of copying old fields under a new trace. A normal
missing/collided incidental colour reply degrades colour independently and does
not fail a successful observed on/off or brightness write. Transport uncertainty
still uses existing whole-output invalidation. Rename/address reuse retires the
old sample and token, and epoch loss marks colour stale with its base observation.
The MQTT proxy selects a 6 s per-target budget before admission for known colour
mappings, retaining 2 s for plain mappings and the unchanged 15 s aggregate cap.
This accounts for the extra correlated exchanges; it is not a hardware timing
guarantee or a renewed per-frame deadline. Explicit service deadlines are unchanged.

All evidence currently comes from private PTYs, an independent literal ATX peer
and the independent HAT emulator. Physical firmware/fixture verification remains
outstanding. See the [support matrix](support-matrix.md) for unfinished modes.

## Integrating a plan

Resolve and validate the original mapping generation, obtain the existing
exclusive transaction permit for the whole plan, and then execute every frame
in order within one operation deadline. DTR registers and device-type selection
are shared by the bus, so a permit acquired separately for each frame is
insufficient. This codec does not create that ownership or deadline itself.

Writes accept validated short/group/broadcast destinations. Word loads put the
low byte in DTR0 and high byte in DTR1. Channel triples use DTR0/1/2. Every
extended command, including final ACTIVATE, has its own immediately preceding
ENABLE DEVICE TYPE 8. These are ordinary 16-bit sends, not ATX repeat commands.

For short address 3, 370 mirek produces the literal sequence:

```text
A372 C301 C108 07E7 C108 07E2
```

Use the existing ATX encoder and transaction correlation; transport completion
does not confirm a physical colour. Preserve device clamping/range errors in
actual readback. A failed/partial execution must not be published as the intended
colour. Multi-controller DTR interleaving and hardware timing remain physical
validation concerns.

## Queries, MASK and status

Queries accept short addresses only, avoiding group reply collisions. `Step`
distinguishes Send from Query. Byte query plans cover gear/type features, colour
status, RGBWAF control and extended version. `ValueQuery` selects x/y/Tc,
individual RGBWAF channels/control and four Tc limits. Its `width` describes the
result, rather than the size of every wire reply.

Value plans first query actual level to update colour reporting, then load the
selector, enable DT8 and query the value. A word value uses that response as MSB
and an additional QUERY CONTENT DTR0 as LSB. The emulator also updates DTR1 to
the MSB, as in its independently authored gear reference. A byte value has no extra low-byte
query. Stop on a failed query. Never use a monitored frame, missing reply or
collision as a successful byte.

`byte_reply` preserves raw bytes, including bitmap/version 255. Apply `ByteValue`
only to byte colour values where 255 means MASK. `word_reply` preserves failures
from either byte, assembles in MSB/LSB order, and recognizes only full 65535 as
MASK: `FF00` is valid raw 65280. A known raw word still needs the appropriate
typed constructor; for example zero is not a valid temperature.

Features retain the raw byte, xy/Tc flags and primary-N/RGBWAF counts. Reserved
count 7 makes `counts_valid` false and must not become seven advertised channels.
Status retains range/calibration flags; `active` returns a representation only
when exactly one type bit is set. Ambiguous/no type bits remain unknown.
Primary-N capability can be reported, but its control is outside this slice.

## Evidence and references

Tests use literal independent commands, conversion boundaries and separate
byte/word MASK cases. Fault-injection rows are in
`v2-colour-codec-controls.toml`. This is local protocol evidence; no physical
DT8 HAT/fixture validation or colour MQTT compatibility is claimed.

Protocol facts were cross-checked with pinned
[python-dali Part 209](https://github.com/sde1000/python-dali/blob/4b21c18e79d02d2a97f91d135f0eee450e8c2fb9/dali/gear/colour.py)
and [Part 102](https://github.com/sde1000/python-dali/blob/4b21c18e79d02d2a97f91d135f0eee450e8c2fb9/dali/gear/general.py).
Units/ranges are also documented by
[BACMOVE](https://docs.bacmove.com/docs/dalion/en/web-user-interface-dali-commissioning/),
while [Siemens](https://cache.industry.siemens.com/dl/files/111/109807111/att_1099312/v1/109807111_DALI_RGBWAF_en.pdf)
documents RGBWAF channel MASK. These sources were read, not copied into the codec;
the emulator must retain an independent implementation.
