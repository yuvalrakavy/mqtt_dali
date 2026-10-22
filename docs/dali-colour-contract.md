# DT8 colour protocol codec

`dali::colour` provides typed Part 209 values and complete 16-bit frame plans.
It performs no UART, MQTT, persistence, discovery or emulator operations. The
canonical daemon currently still exposes the base output and live input paths;
colour service execution and capability discovery are subsequent work.

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
RGB into xy. The later executor must discover capability, active mode and limits
before accepting an operation. DT8 alone does not imply RGB support.

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
and an additional QUERY CONTENT DTR0 as LSB. A byte value has no extra low-byte
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
