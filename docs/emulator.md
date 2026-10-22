# ATX HAT emulator

The standalone emulator owns a physical scenario independent of bridge inventory.
It creates a local PTY; it never connects to MQTT or opens a real UART.

```sh
cargo build --target aarch64-apple-darwin --bin dali_hat_emulator
mkdir -m 700 /tmp/my-dali-emulator
target/aarch64-apple-darwin/debug/dali_hat_emulator \
  --scenario examples/emulator/basic-i4.json \
  --control-socket /tmp/my-dali-emulator/control.sock
```

Use the actual Cargo target directory if `CARGO_TARGET_DIR` is set, and your host
target on Linux. The JSON ready record gives the PTY slave path. Supply it as the
v2 daemon's `--uart` argument; see [running v2](running-v2.md). Scenario IDs
are private fixture identifiers, not bridge names. Unknown fields/ranges fail.
Runtime address/group/scene changes are in memory; the scenario is not rewritten.

Controls use one newline-terminated JSON request per private Unix socket
connection. A reply contains `result` or `error`, then closes. For example, open
`nc -U /tmp/my-dali-emulator/control.sock`, enter one request, and reconnect for
the next. Available requests include:

```json
{"op":"snapshot"}
{"op":"input_event","bus":0,"frame":32769}
{"op":"set_address","id":"new-fixture","short_address":null}
{"op":"bus_power","bus":1,"powered":false}
{"op":"next_fault","effect":{"kind":"delay","millis":250}}
{"op":"next_fault","effect":{"kind":"receive_collision"}}
{"op":"next_fault","effect":{"kind":"transmit_collision"}}
{"op":"next_fault","effect":{"kind":"truncate","length":1}}
{"op":"next_fault","effect":{"kind":"drop"}}
{"op":"raw","bytes":[74]}
{"op":"reboot"}
{"op":"advance_time","millis":900000}
```

32769 is 0x008001, a device/instance button press at input address 0, instance 0;
command-bit frames are rejected. With matching saved input metadata, the bridge
now publishes [native input State/events](mqtt-inputs.md). Discovery and semantic
emulator controls remain in development. One next-response fault may
be armed at once. It affects the next J/N/X/Z terminal, excluding echoes, status
and input events; in the two-terminal profile it affects the first terminal.

`"clock":"frozen"` selects deterministic model time. Otherwise monotonic elapsed
time advances it. `advance_time` moves either clock forward. UART, fault-delay and
shutdown timers continue when model time is frozen. Randomization is seeded.
HAT reboot preserves fixture state and clears HAT parser/output/fault schedules,
delimiting interrupted output before B.

The socket directory must belong to this user with mode 0700; the socket is mode
0600. Existing paths are never replaced. After an unclean exit remove your stale
socket explicitly. Cleanup is best effort, only for this process's socket inode.
No HTTP/TCP endpoint exists. Requests are limited to 16 KiB and 16 concurrent
clients; idle clients close after 1 s. Timeout does not undo an applied control.

`V000004`/`B000004` identify an emulated I4 profile, not actual firmware. Choose
`"repeat":"one_terminal"` (default) or `"two_terminals"` and use the matching
explicit dispatcher policy. Physical repeat cardinality remains unverified.

Base coverage includes addressing, DTRs, direct level/MASK, off/steps/max/min,
groups/scenes and selected queries/configuration. Levels currently change
immediately. The DT8 colour profile below adds selected colour operations.
Fade timing, timed UP/DOWN, input-device commands, emergency and memory/data
models remain P4 extensions. Unsupported operations never fabricate a
positive device reply. N proves HAT transmission, not device support.

## Colour fixtures

Use `examples/emulator/colour-i4.json` in place of the basic scenario to obtain a
DT8 fixture at short address 3 on bus 0, plus a button input. The existing basic
scenario and scenario version 1 are unchanged. A colour profile is optional and
requires device_type 8. It explicitly lists supported xy, temperature and/or
RGBWAF representations and a supported initial `active` type. Missing
representations are unsupported; RGB support is never inferred from DT8 alone.

The example starts at 250 mirek (4000 K), with configured limits 200..400 and
physical limits 153..500 mirek. Its six RGBWAF values start at [1,2,3,4,5,6],
xy at [4660,9029], and master DALI level at 100. Initial numeric values must be
valid: Tc 1..65534 with ordered physical/configured limits surrounding its value;
xy 0..65534; RGBWAF 1..6 entries, each 0..254. Unknown profile fields fail.

Temporary xy/Tc/RGB/WAF commands stage raw values. ACTIVATE applies them and
clears the temporary values/type to MASK. Writing MASK replaces an earlier staged
numeric value; at activation it leaves the actual channel unchanged. RGB/WAF
triples accumulate together. Selecting another representation clears the previous
family's pending values. Tc zero is ignored. Tc MASK does not change actual Tc
or make it active. Tc requests outside configured limits clamp at activation and
set the Tc range bit. Colour writes leave master brightness unchanged.

The DT8 enable selection is consumed by the next standard 16-bit command, even
if addressed to another fixture or awaiting its configuration repeat. Special
DTR commands and 24-bit input events preserve it. The profile models the
broadcast selection once per bus because every modeled fixture hears the same
special command. The real bridge still plans ENABLE immediately before every
extended command. Emulator and bridge have separate command/value implementations.

Supported queries include colour gear/type features, active type/range status,
current xy/Tc/RGBWAF values, Tc limits, control and raw extended version 1.
Word values return their MSB and place the LSB in DTR0. RGBWAF byte queries leave
DTR0 alone. Inactive/unsupported representations return MASK, without inventing
a converted colour. RGBWAF control is the fixed normalized profile value 0x80;
gear features report manual activation and no automatic calibration.

Private `snapshot` retains `colour` as the initial physical profile and exposes
`colour_state.actual`, `colour_state.temporary` and the range flag as live raw
model state. Profiles without colour omit this new state. Address/group changes
do not replace fixture identity. The existing base RESET path does not establish
DT8 reset/settings support.

This model does not simulate photometry, xy gamut, fades, colour scenes, alternate
RGBWAF modes/channel assignments, primary-N control, auto activation, or DT8
reset/settings. Those remain explicit extensions. It is not a certification
oracle or evidence of a particular firmware's behavior. Selected transitions were
cross-checked with an independently authored
[gear implementation](https://github.com/cad435/OpenDALI_EVG/blob/c1f73d0261c79ae1def6c4a615dcb49f7dc335dd/Firmware/src/dali/protocol/dali_dt8.c)
and its [dispatch](https://github.com/cad435/OpenDALI_EVG/blob/c1f73d0261c79ae1def6c4a615dcb49f7dc335dd/Firmware/src/dali/protocol/dali_protocol.c),
alongside the protocol references in [the colour codec guide](dali-colour-contract.md).
The source was read, not copied. Focused sensitivity controls are in
`v2-colour-emulator-controls.toml`.

SIGINT/SIGTERM stops under bounded waits, including with unread output. Logging
uses tracing-init's normal configuration. All tests use private PTYs/sockets and
no live broker. Run `cargo nextest run --target <host-target>`; safety controls
are in `v2-emulator-controls.toml`. P2 does not certify DALI or physical firmware.
