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
target on Linux. The JSON ready record gives the PTY slave path. P3 connects the
v2 daemon to it; currently `mqtt_dali_v2` only checks configuration. Scenario IDs
are private fixture identifiers, not bridge names. Unknown fields/ranges fail.
Runtime address/group/scene changes are in memory; the scenario is not rewritten.

Controls use one newline-terminated JSON request per private Unix socket
connection. A reply contains `result` or `error`, then closes. For example, open
`nc -U /tmp/my-dali-emulator/control.sock`, enter one request, and reconnect for
the next. Available requests include:

```json
{"op":"snapshot"}
{"op":"input_event","bus":0,"frame":132097}
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

132097 is 0x020401, a 24-bit event; command-bit frames are rejected. P2 injects
wire events; P4 adds semantic inputs and discovery. One next-response fault may
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
immediately. Fade timing, timed UP/DOWN, DT8, input-device commands, emergency and
memory/data models are P4 extensions. Unsupported operations never fabricate a
positive device reply. N proves HAT transmission, not device support.

SIGINT/SIGTERM stops under bounded waits, including with unread output. Logging
uses tracing-init's normal configuration. All tests use private PTYs/sockets and
no live broker. Run `cargo nextest run --target <host-target>`; safety controls
are in `v2-emulator-controls.toml`. P2 does not certify DALI or physical firmware.
