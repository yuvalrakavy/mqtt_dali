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
{"op":"set_input_value","id":"panel","instance":0,"bytes":[255]}
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
now publishes [native input State/events](mqtt-inputs.md). Input value profiles
are described below. Managed input configuration remains in development; the
native setting model is described below. One next-response fault may
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
Fade timing, timed UP/DOWN, input timers/feedback, emergency and memory/data
models remain P4 extensions. Unsupported operations never fabricate a
positive device reply. N proves HAT transmission, not device support.

## Input current-value profiles

An input module may have an empty `instances` list, representing module metadata
without an input instance. Status/count/type queries support bridge addressed
discovery independently of saved bridge inventory. Nonempty lists contain at
most 32 types in 0..31; explicit value profiles still require an existing instance.
The complete multi-type output enumeration regression uses a separate literal
UART peer, rather than assuming emulator agreement proves protocol correctness.

Unaddressed input modules also support independent raw Part 103 commissioning:
INITIALISE selection/15-minute expiry, repeated pairs within 100 ms, RANDOMISE
readiness after 200 ms, inclusive search, Compare, raw short-address Program /
Verify / Query, Withdraw and Terminate. Optional `random_address` must fit 24 bits;
missing values use a separate seeded input generator, preserving existing output
draws. RANDOMISE replaces participating modules' random addresses. Input and
output repeat latches are separate, and 24-bit input traffic preserves output DT8
selection. Identical backward bytes still aggregate to one value; independent
literal-peer tests supply exact-collision refusal evidence. Physical certification,
input timers and feedback are outside this model.

Standard FD broadcast-unaddressed input queries answer only modules without a
short address, independently of the bridge codec. Sole-candidate recovery uses
them for topology checks before programming. Identified recovery performs only
short-address Status/count/type queries and changes bridge inventory, leaving
the native fixture at its newly assigned address. The operator workflow is in
[address recovery](address-recovery.md).

Keep numeric instance types; add optional value profiles to a module:

```json
{"id":"panel","short_address":0,"instances":[1,2,3,4],"values":[
  {"instance":0,"resolution":1,"bytes":[255]},
  {"instance":1,"resolution":10,"bytes":[171,192]},
  {"instance":2,"resolution":2,"bytes":[170]},
  {"instance":3,"resolution":16,"bytes":[18,52]}
]}
```

Bytes are wire responses including low padding. Profiles require 1..255-bit
resolution, exactly ceil(bits/8) bytes and a unique existing instance number.
Without a profile the model answers type/count but invents no value/resolution.
Existing scenarios remain valid. Independent 24-bit commands answer individual
type, resolution, module status/count and value/latch queries. INPUT VALUE
captures all bytes; subsequent latch queries retain them even if `set_input_value`
changes live data. A new INPUT VALUE restarts the latch. Input/output address
spaces are independent; conflicting duplicate replies collide. `input_event`
changes no query value, and `set_input_value` emits no native event. Tests select
both explicitly. Value/latch state is independent of the settings below.

## Input event settings model

Every existing instance has native enable, event priority, addressing scheme,
24-bit filter and three instance-group settings, even without a value profile.
The diagnostic snapshot includes a `settings` array by instance number plus
the module's DTR0/1/2 scratch values. Scenario version 1 is unchanged.

At emulator startup, instances use enabled=true, priority 4, scheme 2
(device/instance), all common filter bits for types 1..4 or zero for unknown
types, and three FF/unassigned groups. These are explicit model defaults, not
claims about every manufacturer's reset defaults. Address changes and HAT
reboot preserve these fixture settings and value latches.

For example, send ordinary 24-bit `l` frames on bus 0:

```text
lC13002
l010061
l010061
l010084
```

This stages priority 2 in the input DTR0, sets it on short address 0/instance 0
using two adjacent setting frames, then queries it (`J02`). Each single setting
transmission replies `N`, which proves transmission rather than application.
The native pair must be within 100 ms; a late pair or intervening input/output
frame does not set anything. The ATX 16-bit `t` command is unsuitable here.

The independent model implements instance setting opcodes 61..68 and setting
queries 84/86/88..8B/90..92. Invalid priority/scheme/group DTR values leave settings
unchanged. Known types retain only their defined filter bits (FF/01/1F/01),
while unknown types preserve all 24 raw bits without invented semantics. An FF
filter byte is data. Disabled instances still answer their type/settings;
QueryEnabled returns no backward byte when disabled. Missing instances likewise
return no byte, so managed callers need a separate presence/type proof.
FD reaches only unaddressed modules and FF all modules on the selected bus;
ordinary settings affect only the selected existing instance.

`input_event` remains raw UART test injection and deliberately bypasses enabled
state, filters and scheme. `set_input_value` does not generate events. Natural
stimulus/gesture scheduling, event timers, feedback and hardware persistence
validation are later work. Literal model and UART/reboot tests cover this
foundation; it is not yet a managed MQTT/HTTP/browser configuration feature.

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
An optional `"auto_activation": true` in the colour profile enables automatic
activation; omitted/false keeps the earlier manual behavior and serialized shape.

Temporary xy/Tc/RGB/WAF commands stage raw values. ACTIVATE applies them and
clears the temporary values/type to MASK. Writing MASK replaces an earlier staged
numeric value; at activation it leaves the actual channel unchanged. RGB/WAF
triples accumulate together. Selecting another representation clears the previous
family's pending values. Tc zero is ignored. Tc MASK preserves its numeric value;
activation still selects Tc as the active family. Tc requests outside configured
limits clamp at activation and
set the Tc range bit. Colour writes leave master brightness unchanged.

The DT8 enable selection is consumed by the next standard 16-bit command, even
if addressed to another fixture or awaiting its configuration repeat. Special
DTR commands and 24-bit input events preserve it. The profile models the
broadcast selection once per bus because every modeled fixture hears the same
special command. The real bridge still plans ENABLE immediately before every
extended command. Emulator and bridge have separate command/value implementations.

Supported queries include colour gear/type features, active type/range status,
current xy/Tc/RGBWAF values, Tc limits, control and raw extended version 1.
Word values return their MSB, place it in DTR1 and place the LSB in DTR0. RGBWAF
byte queries preserve both registers. Inactive/unsupported representations return MASK, without inventing
a converted colour. RGBWAF control starts at 0x80 and follows temporary
control/ACTIVATE and native scene recall; xy/Tc activation unlinks channels.
Actual control masks links to unsupported channels while preserving mode bits.
Gear features report the profile's automatic-activation bit and no automatic calibration.

With automatic activation enabled, the supported immediate base commands (OFF,
DAPC, recall min/max and immediate steps) also apply staged colour and clear its
temporary values. DAPC MASK activates colour without changing master brightness.
Explicit ACTIVATE works with either setting. This exposes the normal partial
effects of interrupted staging; it does not simulate a transactional rollback.
Levels/fades are still instantaneous in the model, so it proves command order,
not physical fade completion or absence of visible transitions.

Private `snapshot` retains `colour` as the initial physical profile and exposes
`colour_state.actual`, `colour_state.actual_control`, `colour_state.temporary`,
`colour_state.scenes`, `colour_state.report` and the range flag as live raw
model state. Profiles without colour omit this new state. Address/group changes
do not replace fixture identity. Base RESET clears all sixteen colour scenes
alongside level scenes; broader DT8 reset/settings support remains pending.

Native colour scenes support Store/Remove/Recall, manual/automatic activation,
masked participation, report queries and CopyReportToTemporary. Scene colours
start masked and are populated through native commands; see
[scene semantics](dali-scenes-contract.md).

This model does not simulate photometry, xy gamut, fades, RGBWAF control-mode or
linking effects on arc power, channel assignment changes, primary-N control, or DT8
reset/settings. Those remain explicit extensions. It is not a certification
oracle or evidence of a particular firmware's behavior. Selected transitions were
cross-checked with an independently authored
[gear implementation](https://github.com/cad435/OpenDALI_EVG/blob/c1f73d0261c79ae1def6c4a615dcb49f7dc335dd/Firmware/src/dali/protocol/dali_dt8.c)
and its [dispatch](https://github.com/cad435/OpenDALI_EVG/blob/c1f73d0261c79ae1def6c4a615dcb49f7dc335dd/Firmware/src/dali/protocol/dali_protocol.c),
alongside the protocol references in [the colour codec guide](dali-colour-contract.md).
The source was read, not copied. Focused sensitivity controls are in
`v2-colour-emulator-controls.toml`, `v2-combined-colour-controls.toml` and
`v2-scene-controls.toml`.

SIGINT/SIGTERM stops under bounded waits, including with unread output. Logging
uses tracing-init's normal configuration. All tests use private PTYs/sockets and
no live broker. Run `cargo nextest run --target <host-target>`; safety controls
are in `v2-emulator-controls.toml`. P2 does not certify DALI or physical firmware.
