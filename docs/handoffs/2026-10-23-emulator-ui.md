# Handoff: configurable DALI emulator web UI

Date: 2026-10-23. Prepared for a separate implementation session.

## Request and authorization

Implement a responsive web UI for the standalone ATX I4 HAT emulator. The user
wants to open it in another tab alongside the bridge configuration UI and test
the system without physical hardware. The UI must both configure the simulated
installation and operate its devices. The user requested this handoff and
continued bridge development in the original session. This document does not
mean another session has been started or a worktree has been claimed.

The bridge architecture and continued implementation are already approved.
Inspect the code and follow the repository workflow; make routine implementation
choices without reopening settled requirements. Ask for a material new product
decision if one actually blocks progress. Do not deploy, publish, push, merge,
modify a production configuration, or contact a live HAT/Store.

## Starting point

- Source checkout: `/Users/yuval/Documents/Projects/Home/mqtt_dali-v2`.
- Branch: `feat/dali-v2`; implementation baseline `b8a8ef6`.
- Use a separate branch and worktree from that baseline or its descendant that
  contains this handoff. Verify the actual tree is clean before creating it.
  Do not work in the original session's checkout.
- Read `CLAUDE.md`, `docs/emulator.md`, `docs/v2-progress.md`, and the applicable
  user-level ways-of-working instructions. This is the bridge repository, not
  the Store repository; Store's dev-track pool is not this project's lifecycle.
- Master architecture: `docs/superpowers/specs/2026-10-21-dali-v2-design.md`.

The user subsequently specified a separate emulator process with its own web UI,
emulating the HAT over **TCP in place of UART**. The bridge retains its current
web UI and supports either physical UART or this TCP endpoint.

The emulator has an independent device model and ATX wire implementation. Today
it provides a PTY; retain that headless compatibility while adding TCP. It does not
connect to MQTT, open physical serial ports, or borrow the bridge inventory as
its simulated hardware truth. Keep that separation.

Existing entry points:

- `src/bin/dali_hat_emulator.rs`: standalone CLI and owned lifecycle.
- `src/emulator/{scenario,model,session,control,pty,wire}.rs` and sibling modules.
- `examples/emulator/*.json`: strict version-1 scenarios.
- `tests/hat_emulator.rs`: actual standalone PTY/socket tests.
- `docs/emulator.md`: supported controls, profiles and limitations.
- `web/` and `src/web/`: reference implementation of Preact, embedded assets,
  authentication and bounded server lifecycle. These belong to the bridge UI;
  avoid changing them for the emulator.

The existing private Unix socket supports snapshot, input_event, set_input_value,
set_address, bus_power, next_fault, raw, reboot and advance_time. There is no
emulator HTTP listener or graphical panel yet. Snapshot is diagnostic state,
not a scenario document that can be blindly saved and reloaded.

## Agreed product scope

Serve the new UI from the emulator process, at its own address, with an obvious
**DALI Emulator** title. Reuse the project's Preact/TypeScript approach with a
separate source/build/asset entry point. Make it usable on an iPhone and desktop.
The bridge remains a separate process/tab and connects to the emulator's TCP HAT
endpoint. Each process has its own HTTP endpoint; HTTP is not the HAT transport.

## Shared TCP transport boundary

The original bridge session owns the connector and CLI option
`--hat-tcp IP:PORT` (numeric IPv4 or bracketed IPv6 socket address). It is mutually
exclusive with an explicitly supplied `--uart`. UART remains the normal default.
TCP carries the existing ATX UART byte stream unchanged: ordinary commands,
echoes, J/N/X/Z terminals, asynchronous L frames and B/V/D records. No JSON
wrapping, length prefix, WebSocket, Telnet negotiation or extra handshake.
Writes may split or coalesce arbitrarily; parse bytes, not TCP packets.

The emulator session owns a configurable TCP listener (suggest
`--hat-listen 127.0.0.1:5580`, with port0 supported for tests). Bind only when
explicitly enabled; report the actual endpoint in the JSON ready record. Keep
the web listener separate. One active bridge connection owns this HAT; refuse
additional concurrent clients without changing the first connection's model.
On disconnect preserve native fixture state. Each newly accepted connection
starts a fresh HAT parser/output session and emits `B000004\n`, preserving
fixtures as a HAT reboot does. Do not replay prior connection replies or events.
The model must remain available to web controls while no bridge is connected.

Keep existing I4 one/two-terminal profile selection explicit on both sides.
The connector uses existing initialization, quiet-period handling, continuous
event processing, deadlines and reconnect ownership. A TCP write still does not
prove DALI execution. Do not redesign command correlation or bus-off recovery.
Neither listener nor connector needs MQTT. Prefer loopback by default; remote
raw TCP is an explicit trusted-network choice, separate from web authentication.

Configuration UI:

- Create, load, edit, save/export and reset scenarios; include ready-made examples.
- Add/remove buses, output fixtures, input modules and their instances.
- Select supported profiles: dimmer, DT7 relay, DT8 colour light, button,
  absolute input, occupancy sensor and light sensor; preserve unknown native
  types where the model already supports them.
- Configure device capabilities, supported colour representations and input
  value/resolution profiles. Expose existing meaningful model options rather
  than presenting unsupported capabilities as simulated.
- Set initial addresses or leave devices unaddressed. Set initial brightness,
  colour, groups, scenes and input values.
- Give every device a stable emulator ID independent of short address and
  bridge-assigned name. Clearing/reassigning an address must not lose the
  device's identity in this panel.

Runtime panel:

- Show independently observed native brightness, colour, relay contact and
  virtual level, addresses, groups/scenes and feedback indicators.
- Operate buttons (press/release), maintained inputs, occupancy and numeric
  sensors. Display live input state.
- Simulate address loss, inspect addressing changes, and add/remove devices
  so discovery and recovery workflows can be tested.
- Put HAT reboot, bus power, time controls and communication fault injection
  in an Advanced area. Label HAT reboot separately from scenario reset.

Saved scenario and live state are different:

- Runtime bridge commands and emulator physical controls update live state.
- Reset restores the saved starting scenario; ordinary commands do not rewrite
  its file. Provide an explicit **Save current state as scenario** action.
- Keep editing drafts separate until explicitly applied. Explain whether an
  installation change is live or requires reset; avoid a hidden whole-scenario
  replacement when merely adding one fixture.
- Validate a complete candidate before replacing the current model. Invalid
  imports/edits must preserve the running installation and show useful errors.
- Define and test reset while the bridge remains connected: stale UART output,
  owned commands, input events and HAT initialization must not cross the reset
  as apparently valid new device evidence. Prefer an existing coherent lifecycle
  boundary over inventing a second recovery framework.

## Realistic input controls

Today `input_event` is deliberately raw injection: it bypasses enabled state,
event filters and addressing scheme. `set_input_value` changes queried values but
emits no event. Button timers and sensor parameters exist as native settings,
but do not yet schedule natural gestures or reports.

Normal UI controls should exercise the simulated device's configured behavior,
not compose arbitrary event numbers in the browser. Add semantic stimuli in the
emulator model, using one model clock, so button timing/filter/enable/scheme and
sensor settings affect emitted traffic. Preserve the existing raw injection API
as an explicitly advanced compatibility/testing tool. Do not add a second gesture
recognizer in the bridge. Stage the implementation if needed, and label any
remaining unsupported timing/report behavior accurately.

## Security and lifecycle

The existing socket is private (0700 parent/0600 socket). Adding HTTP changes
that exposure. Default to an explicitly enabled loopback listener with proper
origin/CSRF protections; do not silently expose mutable controls to the LAN.
If remote access is supported, require explicit credentials/configuration and
document it. The bridge's approved web UI has no default password; do not
introduce a shared default here. Review reuse of existing auth/server code
without coupling the emulator to bridge services.

Use bounded operations and owned tasks under the emulator lifecycle. Extend the
wait registry for actual new waits. Do not accept arbitrary filesystem paths
from browser requests: prefer validated upload/download and explicit operator
CLI paths. Preserve existing scenario files, socket clients, CLI behavior and
headless tests. Runtime/build should not require Node; commit embedded assets.

## File ownership and parallel coordination

The parallel session owns the emulator implementation: `src/emulator/**`,
`src/bin/dali_hat_emulator.rs`, emulator-specific test files/examples/docs and
new emulator web directories. The original bridge session will avoid changing
those paths while this work is active.

The original session retains `src/atx/**`, `src/daemon.rs`, `src/service/**`,
`src/dali/**`, bridge configuration and inventory, `src/web/**` and `web/**`.
Its immediate next slice is the TCP HAT connector above, followed by remaining
bridge capabilities. No emulator-model change is needed for the connector.
If the emulator wants to use a common TCP DrainRead adapter from `src/atx`,
coordinate that small transport API; sharing IO infrastructure is acceptable,
sharing bridge protocol codecs/model interpretation is not.

Coordinate before editing shared files: `Cargo.toml`, `Cargo.lock`, `src/lib.rs`,
`docs/wait-registry.md`, `CLAUDE.md`, `docs/v2-progress.md`, support matrices and
the master spec. Prefer emulator-specific documentation and a final integration
note. Retain independent wire/model code; do not import bridge codecs into it.
Do not include another session's edits in a commit. Integrate reviewed commits
later, one at a time, with targeted compatibility checks.

Use a separate target directory from the active bridge session. Its current
warm target is `/Users/yuval/Documents/Projects/Home/mqtt_dali/target`; coordinate
before using/copying it and never run cleanup against another session's target.
The shared SDK commit `153d38051f5197b060fa4281efe6136cffee7419` is currently
local/unpublished; do not change/publish it to solve an unrelated build issue.

## Implementation and verification expectations

Prepare a concrete staged design grounded in the existing scenario/runtime model.
Implement the backend/control lifecycle and scenario round trips before wiring
their UI. Scope commits so the headless emulator remains useful between stages.

Acceptance should include:

1. Start headless as before; existing socket and PTY regressions still pass.
2. Build an installation in the UI, export/import it, and verify independent
   native snapshots and strict scenario validation. Failed edits preserve state.
3. Connect the real bridge through TCP, discover/assign/rename fixtures in
   the bridge tab, and see changes in the emulator tab without inventory sharing.
4. Send light/group/scene/colour/relay commands through the bridge and verify
   native state in the emulator; show actual contact separately from virtual level.
5. Operate normal inputs in the emulator and verify bridge State/events; test
   native timing, filters, disabled inputs and supported addressing schemes.
6. Exercise live add/remove, address loss, save-current and reset semantics.
7. Perform a real functional browser pass on phone and desktop, including menus,
   forms, validation, persistence, scrolling and cancellation/disconnection.
8. Test TCP fragmentation/coalescing, input events interleaved with replies,
   one-client ownership, disconnect/reconnect with retained fixtures, and no
   prior-session output replay. Existing PTY tests must remain compatible.
9. Check asset reproducibility, applicable Rust/TypeScript checks, Pi build,
   bounded shutdown and HTTP access protections. Use fail-first regressions and
   meaningful fault controls for new correctness boundaries.

Existing baseline evidence is **741 native tests**, Node89 and a separately run
1177-command browser workflow; these are historical reference, not tests run by
this handoff. Do not rerun broad suites after every small edit. Physical HAT
compatibility remains unverified even when the emulator tests pass.

Return a final handoff with branch/worktree, exact commit IDs, changed public
controls/formats, compatibility decisions, checks actually run and remaining
limitations. Include exact integration edits needed in shared files.
