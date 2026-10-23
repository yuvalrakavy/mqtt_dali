# Common fixture settings protocol

`dali::gear::settings` supplies checked values and command plans for minimum and
maximum dimming levels, power-on and bus-failure levels, and basic fade time/rate.
This is a library foundation. Managed MQTT commands, authenticated HTTP jobs and
web controls for these settings are subsequent work.

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
the verified HAT repeat policy. The later managed API starts with individual
fixtures: freshly read physical/current limits, validate the whole patch, order
crossing minimum/maximum edits to avoid unintended native clamping, then compare
actual readback. Raising minimum can brighten an already-lit fixture; lowering
maximum can dim it. Off remains Off. Such writes must retire affected output,
colour and group cache evidence through the existing owner.

The independent emulator already implements these registers and setters. Literal
model regressions cover all six setters, adjacent repeats and their 100 ms
window, raw special values, retained fade nibbles, individual/group/broadcast
scope, bus isolation and Off versus lit clamping. A standalone process test also
executes typed plans through the real UART/PTY adapter and queries the model.
No timed fades, mains-cycle behavior or physical HAT certification is claimed.
Controls are in `v2-gear-settings-controls.toml` and its measured ledger.

Protocol references: [pinned python-dali command source](https://github.com/sde1000/python-dali/blob/4b21c18e79d02d2a97f91d135f0eee450e8c2fb9/dali/gear/general.py)
and [Beckhoff's variable reference](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/5908382347.html).
See [capability status](support-matrix.md) for remaining managed and specialist work.
