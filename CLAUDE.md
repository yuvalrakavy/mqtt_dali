# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

mqtt_dali is a Rust service that bridges MQTT and DALI (Digital Addressable Lighting Interface) protocols, enabling remote control of DALI lighting buses via MQTT commands. It targets Raspberry Pi (ARMv7) deployment.

Before expanding infrastructure, establish that the problem is real, assess its
likelihood/impact and consider simpler alternatives. The owner accepts dropped
commands during extremely rare DALI bus outages; do not restart the abandoned
target-deferral effort. If progress stalls for an extended period, explain the
problem, attempted approaches and alternatives promptly so the owner can steer.

## Build Commands

The `feat/dali-v2` worktree is developing the approved migration in phases. Current
scope, verification and remaining work are recorded in `docs/v2-progress.md`.
The default `mqtt_dali` now runs the managed v2 service through BridgeKit;
`mqtt_dali_v2` is an alias. Both accept explicit `--check-config` to import v1
into separate `dali.v2.json` if absent, or validate existing v2 without MQTT/HAT
or terminal setup. See `docs/running-v2.md`. The relocated `mqtt_dali_v1` is
only a regression harness; operational v1 builds use the maintained v1 tag/branch.
For example:

```bash
cargo run --target aarch64-apple-darwin --bin mqtt_dali_v2 -- --check-config
cargo nextest run --target aarch64-apple-darwin --lib --test config_startup
```

`--legacy-config` and `--config` select distinct import/current paths. Existing
v2 always wins; deleting it forces a fresh import on the next check/start. v1
bytes are never changed. Use a local operator-owned directory on a filesystem
with hard links and directory fsync. No migration command contacts the real bus.

New migration safety controls are in `docs/v2-migration-controls.toml`. Run them
with Store's `scripts/negative-control.py --root <this-worktree> <table>`; the
runner restores each mutation. Avoid simultaneous builds or edits while it runs.

The v2 library's `atx` modules provide typed line framing, continuous monitoring
and a common raw Unix UART/PTY adapter. `Monitor::write` reports bytes accepted
only; it does not correlate HAT replies or confirm device state. `Transactions`
privately owns the monitor and serializes/correlates commands while draining idle
events. Initialize explicitly. Timeout/cancellation retains outstanding terminals;
explicit measured `resynchronize` can retire complete-write/receive uncertainty
and requires fresh initialization. The managed owner applies it on startup and
recovery (default 250 ms; `--uart-quiet-ms 1..5000`). Known partial writes still
require a later accepted Boot, including poison discovered during close/reopen.
There is no persistent UART journal; see the quiet-recovery spec and running guide.
Unverified j/t
completion is refused. Subscribe before checking `status()` so a terminal link is
not missed; any broadcast Lagged invalidates inferred state. Custom transports
must implement the explicit nonblocking `DrainRead` contract; generic Pending
does not establish emptiness. Transaction negative controls are recorded in
`docs/v2-transaction-controls.toml`.
`docs/v2-uart-controls.toml` contains the substrate safety controls. All automated
PTY tests are local and never open `/dev/serial0` or contact a real DALI bus.

The standalone `dali_hat_emulator` owns a separate strict physical scenario and
exposes a PTY plus optional mode-0600 Unix control socket in a private directory.
It uses an independent ATX codec/model, seeded addressing, configurable repeat
terminal count and faults; no broker or bridge inventory. See `docs/emulator.md`
and `examples/emulator/basic-i4.json`. `docs/support-matrix.md` distinguishes base
local coverage from P4 extensions and unverified physical behavior. Emulator
safety controls are in `docs/v2-emulator-controls.toml`.

The default build target is `armv7-unknown-linux-musleabihf` (set in `.cargo/config.toml`). To cross-compile for Pi:

```bash
cargo build --release
```

To build for the local machine (e.g., macOS for development):

```bash
cargo build --target aarch64-apple-darwin   # Apple Silicon
cargo build --target x86_64-apple-darwin    # Intel Mac
```

The remaining driver/process notes in this section describe the legacy regression
harness. v2 uses the portable raw Unix UART/PTY adapter and shared BridgeKit runtime.

The legacy DALI hardware driver (`dali_atx`, via `rppal`) is Linux-only, so off the Pi the harness runs
with `--emulation` only; check the Pi build with `cargo check` (the default target). The HAT's
serial protocol (`hat_line`) sits behind a `ByteSource` trait, so its loops are tested on the Mac
against fake byte sources. Tests run on the Mac against the emulator and an in-process fake broker
on 127.0.0.1 (`tests/process.rs` runs the built binary against it):

```bash
cargo nextest run --target aarch64-apple-darwin
```

Every wait carries a `// WAIT: <row>` tag, on its own line above the statement, naming a row of
`docs/wait-registry.md`, which `tests/wait_registry.rs` checks (Store's no-hang spec §13.3, §14).
The MQTT event loop is polled by `Pump`, a task that waits on nothing else; the session never polls
(§14.3). The session runs on a thread of its own (`mqtt::spawn`), since its DALI calls are
synchronous; SIGTERM or SIGINT stops it, and `main` waits for it at most `SHUTDOWN_BOUND`
(`shutdown.rs`). The stop handlers are installed first thing, before logging, the configuration
and the hardware; the logging start and then start-up each run on the blocking pool, raced with a
stop; the configuration file is written off the session, under `SAVE_BOUND`, to a temporary file
renamed over the old one, and a change saved while a write is stuck is queued and written when it
ends; and `main` ends the runtime with `shutdown_timeout`, since dropping it would wait without
limit for a blocking thread stuck in the kernel. The bus protocol's retry loops are bounded in
sends and in time (`dali_manager::BUS_LIMITS`). `docs/no-hang-3b-controls.toml` holds the negative
controls for every guard. One test seam lives in the binary: in debug builds only,
`MQTT_DALI_TEST_LOGGING_GATE` names a file the logging start reads before tracing-init's init, so
`tests/process.rs` can hold that start on a FIFO; inert unless set, absent from a release build.

Nothing on the start, stop or session path prints to stdout or stderr directly: it logs through
tracing, whose sinks drop what they cannot write, since a supervisor's pipe that stopped draining
would hold a print for ever. A line needed where logging may not be up goes through `note`, a
thread of its own. The exemptions are interactive or come before anything can wait: rustop's
`--help` and argument errors, the runtime's own start failure in `main`, and the interactive
configuration (`--setup`, a missing config file, an emulator config without buses), which runs
inside the raced start-up.

Linting and formatting (via trunk):

```bash
cargo clippy
cargo fmt
```

Run with emulation mode (no DALI hardware needed):

```bash
cargo run --bin mqtt_dali_v1 -- --emulation <mqtt_broker_address>
```

Run in setup mode (interactive TUI for configuration):

```bash
cargo run --bin mqtt_dali_v1 -- --setup <mqtt_broker_address>
```

## Legacy regression harness reference

Optional HTTP uses `src/web` and the existing managed owner; see
`docs/web-access.md`. Disabled by default; explicit password/origin, cookie-only
sessions, CSRF and live stream expiry/revocation. Strip CLI credentials before
shared Debug Settings. Clean stop includes actual accepted-IO drop, not just
Axum's outer task. `docs/v2-web-controls.toml` tests these guards. Embedded Preact
source/lock/dist live in `web/`; see `docs/browser-development.md`. Runtime and
Cargo compilation need no Node. Configuration/commissioning command jobs reuse the strict
proxy parser and typed service APIs with original context/deadline and no MQTT
completion. Bound job futures inside the Server task, never detached; results
are process-local, with private free-text reasons sanitized. See
`docs/v2-web-job-controls.toml`. HTTP-only scalar apply/read uses the existing
intent parser, budgets and latest-snapshot acceptance. SSE shares exact
Notice::current receive fencing, bounded loss reporting and three-source rotation;
job watch notifications occur after dropping the book lock. Snapshots replace
the complete state array, with unknown/stale values preserved as such. See
`docs/v2-web-observation-controls.toml`. Browser views unwrap `inventory.Inventory`
and use SSE alone as view authority; HTTP job replies never replace newer views.
Retain original dirty item authority and stable ledger Origin keys across renames.
Rebuild and byte-check committed assets; run the opt-in real-browser test for UI
changes and `docs/v2-browser-controls.toml` for browser authority/route guards.

Managed v2 group membership uses `dali::groups`, `service::groups` and concrete
`physical::Work::Groups`; see `docs/groups.md`. Ordinary assignment saves desired
membership before programming only one bit. Adoption saves reviewed hardware
membership without settings writes; repair explicitly enforces all saved bits.
Keep original stamp/permit/completion through save/program. Complete FF/FF is
mask FFFF; incomplete halves never make a full mask. `read_groups` is Read-class
and must dispatch before requiring CommandCompletion. `create_group` is ordinary
controller metadata. Behavioral controls: `docs/v2-group-controls.toml`.

The old runtime/CLI, modules and MQTT topics described below belong to
`src/legacy_main.rs` / `mqtt_dali_v1`. They are not the canonical v2 interface.
For v2 see `src/daemon.rs`, `src/service.rs`, `src/service/owner.rs`,
`docs/service-development.md` and `docs/inventory-contract.md`. BridgeKit owns
the process, signals, logging, MQTT session and watchdog. The service owns
configuration and optional UART health independently of broker availability.

Base individual output properties use `docs/mqtt-outputs.md`: strict PowerState /
normalized Brightness Desired values, actual readback and Available/Quality State.
The driver and LinkContext-owned snapshot helper publish physical values only on
the SDK worker. Revalidate the latest service snapshot after the final await and
report actual post-write values before success, even unchanged clamped values.
Only the SDK owns publication coalescing; a helper-local skip cache can hide a
physical return after a newer driver report. Aggregate reads retain cursor progress
before each await, skip disabled buses and reject absent UART before admission.
The regression controls are `docs/v2-mqtt-output-controls.toml`.

`dali::scenes` provides checked slots, Off/MASK levels, repeated Store/Remove,
scene report plans and nonactivating colour staging. See
`docs/dali-scenes-contract.md`. Store with masked temporary colour type preserves
the old stored colour; Remove clears both kinds of participation. QueryActualLevel
overwrites scene reporting. Keep each staging/report sequence under one bus
permit. The independent emulator models sixteen colour slots and auto/manual
recall. Named-slot metadata now uses the existing durable inventory/ledger and
shared MQTT/HTTP jobs. Creation requires controller authority; rename/describe/
removal require the original scene authority. Description preserves its token;
removal/recreation replaces its lifetime. Metadata removal never clears hardware.
Scene history remains after removal: do not erase it for an older reader.
Stored reads and native bus/group recall use `service::scenes` under the same
physical owner, original context/deadline and final stamp acceptance. Discover
capabilities before selecting stored reports; stored data never becomes actual
State. Recall says sent, invalidates all cached actual values on its bus at
admission and settlement, even
after failed/cancelled possible sends, and refuses continuing Blink claims there.
Private desired rows use `config::scene_settings` by continuing output Origin;
strip them centrally in `Model::state_bytes`, including HTTP/SSE readers. Never
put contents into ledger projections. `service::scene_settings` owns bounded
Set/Adopt/Repair/Capture and separate Probe/Program work with save-before-write.
Retain original scene token, root revision, fixture Origin and physical stamp.
Copy the confirmed cleared-slot MASK report before staging and Store; editing
never activates. A no-op desired Set still programs, and repair rechecks review
after the handoff. Capture uses actual native family/assigned channels, not the
configured public component. The protected Scenes screen composes the row APIs
for reviewed capture/copy/clear and verification.
Controls: `docs/v2-scene-controls.toml`, `docs/v2-scene-identity-controls.toml` and
`docs/v2-scene-operation-controls.toml`, `docs/v2-scene-settings-controls.toml`.
The protected Scenes screen composes the accepted row APIs; operator guide:
`docs/scenes-browser.md`. `read_scene_settings` is a pure Ready snapshot read,
with optional MQTT token and mandatory HTTP token, independent of UART/bus status.
It has a 1 MiB HTTP result budget; ordinary jobs retain 32 KiB. Job list/SSE
summaries omit results; the waiter fetches per-id results without replacing SSE
view authority. Keep exact output/token coverage, raw scene MASKs, original
Scene/fixture/root/connection authority and frozen candidates. Metadata dialogs
retain their original Scene identity. Release completed operation snapshots.
UI and reader controls: `docs/v2-scenes-ui-controls.toml`. Rebuild assets after
the real browser creation control, then check-dist. No Store/PanelScript change.

Full reassignment uses `service::reassignment` and the existing atomic removal
edits. Save/activate before clear; preserve original stamp, context, completion
and permit. Post-save checks use the own committed revision, not the retired
controller token. An empty saved space still clears unsaved native gear.
`reset_address_space` clears one explicit space; the browser composes existing
assignment/discovery/current reads per space with frozen roots/epoch/lifetime.
Do not reuse Controller.scan (both spaces), latest-root injection, or demand a
65th search after 64verified assignments. See `docs/reassignment.md` and
`docs/v2-reassignment-controls.md`. Lost-address recovery uses `service::restoration`:
identified recovery probes, then atomically removes the candidate and readdresses
the original saved identity to its new address. Preserve module/child Origins,
move output desired group references, and keep private scene rows by Origin.
Free the old reservation; never send a second native address change. Accept the
own new mapping token/root while completing under the original request token.
The sole-unaddressed shortcut may reuse the old address without a config edit.
Its unchanged token/stamp requires permanently revocable queued Notice flags.
Keep both cuts and MQTT/SSE checks. Browser recovery accepts already scanned
rows, freezes both identities, stops Blink first and refreshes the resulting
address. See `docs/address-recovery.md` and `docs/v2-restoration-controls.md`.

Individual `clear_short_address` uses restoration's distinct Clear action/plan,
the addressed codec and the same stamp, permanent Notice cuts and final authority.
Presence -> DTR0 FF/addressed repeated write -> absence; preserve the saved row
and old reservation without a config edit. Clear is mutating and needs the known
output repeat profile. UI confirms frozen name/address, stops matching Blink
first and does not rediscover automatically. `docs/v2-address-clear-controls.toml`
checks native scope/proofs/profile and frontend authority/Stop/outcome guards.
Clear dismissal revokes its scope synchronously; layout-effect cleanup handles
unmount. Passive effects can allow an awaited Stop reply to resume first. The
real browser test pauses timers/animation frames while exercising actual Cancel,
Close and navigation; directly toggling a unit-test scope cannot test that wiring.

`dali::colour` provides pure typed DT8 Tc/xy/RGBWAF values and complete frame plans;
see `docs/dali-colour-contract.md`. `ServiceHandle::read_colour`/`apply_colour`
reuse whole-plan DTR/enable ownership, captured deadlines and final acceptance.
Colour quality is independent of base quality; incidental colour failures cannot
fail successful base writes. Writes preflight known gear features, representation,
normalized RGBWAF mode and physical channel assignments; no implicit mode change.
Both manual and automatic activation are accepted. Combined operations preflight
before either mutation, send off before colour or colour before positive base,
and require both actual readbacks. Ordering never promises completed fades or
rollback after partial writes. Controls are in `docs/v2-colour-service-controls.toml`
and `docs/v2-combined-colour-controls.toml`.
Never share the codec with emulator semantics,
treat a high byte FF as full-word MASK, clear unrequested WAF channels, or infer
RGB support from DT8 alone. MQTT scalar colour uses the configured family only,
with independent ColorAvailable/ColorQuality and actual supported-format State;
see `docs/mqtt-outputs.md` and `docs/v2-mqtt-colour-controls.toml`.
Outputs can persist optional `color_representation` (`temperature`, `xy`, `rgbwaf`)
through `MetadataEdit::OutputColorRepresentation` / `set_color_representation`.
This is component metadata, not hardware configuration or capability evidence.
Changes preserve Origin and rotate the binding token, including combined renames;
consumers retire old contract intent. See `docs/inventory-contract.md` and
`docs/v2-colour-selection-controls.toml`. First reads discover configured colour;
read/base-write budgets are 6 s, colour/combined writes 8 s, plain outputs 2 s,
and All retains its original 15 s ceiling. Partial xy sends numeric/MASK setters
for both coordinates. Explicit null colour properties clear stale merged State.
The independent emulator's optional strict colour profile covers selected DT8
staging/activation/current values and limits; see `docs/emulator.md`. Staged MASK
overwrites earlier staged numeric values; ACTIVATE clears temporaries. Standard
16-bit commands consume DT8 selection; special DTR/24-bit input traffic preserve
it. Physical/alternate-mode/reset/settings/scene behavior remains unverified or
unsupported as explicitly listed. Never use emulator agreement as certification.

`dali::input` decodes standard Part 103 source schemes and native Part 301–304
data, and builds individual read-query frames; see `docs/dali-input-contract.md`.
The codec is pure. Saved Inputs/Instances use the existing named identity ledger
and durable metadata rename path; see `docs/inventory-contract.md`. Module
replacement retires every child lifetime; ordinary rename preserves them. Replay
validates hierarchy and continuing parent origins at every batch, and the
independent consumer verifies this without producer replay helpers. Input
configuration remains subsequent work. Addressed
discovery is documented in `docs/discovery.md`: one short address per bounded
operation, captured probe evidence, durable additions only, preserved existing
lifetimes and original command completion. Output/input address spaces are
independent. Input expansion mints only new child Origins. `physical::Work` owns
the probe through settlement; the service retains its permit through save.
No fixture availability is inferred from a probe after disk awaits. Use
`docs/v2-discovery-controls.toml` for behavioral controls. Live observed State
and native events now use `docs/mqtt-inputs.md`: nullable unknown properties,
separate bounded event notices, original source generations/absent traces and
receive-only per-route cutover. Sync routes before draining frames after metadata
activation. State comes from latest snapshots after awaits; notices carry native
actions rather than historical State. Preserve the existing bounded SDK event
queue across broker outages; never reconstruct actions from retained State.
`read_input` and MQTT instance/aggregate refresh query exact 1..255-bit values.
`Evidence` separates Event/Query; input reads share the existing physical owner,
permit and fixed settlement through `physical::Work`. Newer native observations
win over latched queries; receive boundaries reset sequence tracking. Query
availability requires complete supported/type-matched values. Explicit nulls
clear old merged data. Preserve full bytes/hex, strip low padding and never
truncate to u64, treat FF as MASK or fabricate actions/units. See
`docs/v2-input-readback-controls.toml`; physical input validation remains pending.

One unaddressed output or input module can be assigned per `assign_next_output` /
`assign_next_input` request; see
`docs/commissioning.md`. Search/save/program are concrete owned phases retaining
the same permit and captured search stamp. A fresh default-named reservation
commits before PROGRAM and is not removed on physical failure/cancel. Confirmed
reservation failures return structured reserved_but_unconfirmed; disk/CAS
uncertainty retains existing freeze/recover and never auto-programs. No fake
fixture State or UART journal. `--uart-repeat-terminals unverified|1|2` exposes
the existing policy, default unchanged; unknown profiles refuse the sequence
before starting output assignment. Inputs use two ordinary 24-bit `l` exchanges
under one 100 ms pair deadline, 200 ms randomisation wait and raw short addresses.
They reserve empty `InputN` modules before PROGRAM; separate addressed discovery
adds real instances under the continuing identity. No second save/CAS is hidden
inside assignment. The independent emulator and literal peers test both counts;
neither proves physical HAT behavior. Fault controls are in
`docs/v2-commissioning-controls.toml` and `docs/v2-input-assignment-controls.toml`.

`configure_bus` / `MetadataEdit::ConfigureBus` creates or updates a bus through the
same durable owner. See `docs/bus-setup.md`. Preserve saved children/Origins;
enable-only edits keep child tokens, while rename changes bindings. Sync the
committed enabled set into HAT polling and input routes before replying. Track
actual enabled membership separately from the internal bus-0 health fallback.
Disable retires values/listeners; Notice's private receive stamp plus the event
helper's latest-snapshot check prevents pre-disable actions after re-enable.
Ordinary asynchronous bus health remains best effort; no new UART recovery or
health freshness framework. Controls: `docs/v2-bus-setup-controls.toml`.

`remove_from_list` / `MetadataEdit::RemoveOutput` / `RemoveInput` exposes existing
durable removal. See `docs/remove-from-list.md`. Require current selected item
authority, not the controller token; preserve the committing command reply after
target retirement. Module removal includes children; output removal clears saved
group references. Hardware addresses/levels/groups/events remain unchanged.
Rediscovery creates fresh lifetimes, never resurrecting old intent. Controls:
`docs/v2-removal-controls.toml`. Store logical-equipment handling remains P8 work.

Output identification uses the service-owned Manager in `service/identification.rs`;
see `docs/identification.md`. Never hold a permit across dwell or refresh public
intent after rename. Private restoration requires continuing Origin/location and
the captured physical stamp. Public writes to the active Origin refuse both at
submission and owner admission. Metadata waits fairly only while identifying,
within its original deadline. Restore reports actual readback, not captured intent
or completed fade. Ordinary power may activate staged DT8 colour; no explicit
colour/settings writes. Parent drop owns teardown; no crash/shutdown restoration
guarantee or journal. Controls: `docs/v2-identification-controls.toml`.

## Architecture (legacy)

### Module Dependency Flow

```
legacy_main.rs → mqtt.rs → dali_manager.rs → dali_atx.rs (hardware)
                                     → dali_emulator.rs (testing)
```

### Key Modules

- **legacy_main.rs** — Legacy harness entry point, CLI parsing (`rustop`), logging init, controller instantiation
- **mqtt.rs** (`MqttDali`) — MQTT client lifecycle, command dispatch, topic pub/sub, reconnection logic
- **dali_manager.rs** (`DaliManager`, `DaliBusIterator`) — Core DALI protocol: bus discovery (24-bit binary search), group management, address programming, brightness control
- **dali_atx.rs** (`DaliAtx`) — UART communication to DALI HAT hardware via `/dev/serial0`
- **dali_emulator.rs** (`DaliControllerEmulator`) — Software simulation of DALI buses/lights for development without hardware
- **dali_commands.rs** — IEC62386 DALI command constants
- **command_payload.rs** — MQTT command/response serde structures (`DaliCommand` tagged enum)
- **config_payload.rs** — JSON configuration structures (`DaliConfig`, `BusConfig`, `Channel`, `Group`)
- **setup.rs** — Interactive TUI for creating/editing configurations

### Hardware Abstraction

The `DaliController` trait (`dali_manager.rs`) abstracts hardware access with two implementations:
- `DaliAtx` — real UART/serial hardware
- `DaliControllerEmulator` — full protocol simulation

### MQTT Topics

- `DALI/Controllers/{name}/Command` — receives JSON commands (subscribed)
- `DALI/Config/{name}` — publishes full configuration
- `DALI/Status/{name}` — publishes status ("OK" or error)
- `DALI/Active/{name}` — availability via Last Will Testament
- `DALI/Reply/{command}/{name}/Bus_{n}/Address_{n}` — query responses

### Error Handling

Uses `error-stack` for contextual error chains with `thiserror`-derived `CommandError` enum.

### Configuration

JSON files (default: `dali.json`) with hierarchy: `DaliConfig` → `BusConfig[]` → `Channel[]` + `Group[]`. Config is created interactively if the file doesn't exist.

## Deployment (legacy)

v2 deployment/release remains pending; use `docs/running-v2.md` for isolated development.

Cross-compiled binary is deployed to Raspberry Pi via `install_to_pi` (Fish script) and installed as a systemd service (`dali.service`). The `install_on_pi.sh` script handles on-device setup.

## Key Dependencies

- **tokio** — async runtime
- **rumqttc** — MQTT client v5 (`rumqttc::v5`) (default features disabled)
- **rppal** — Raspberry Pi GPIO/UART access
- **error-stack** / **thiserror** — error handling
- **tracing** + **tracing-init** (git dep, `otel` + default features) — structured logging
- **serde** / **serde_json** — serialization

## Logging (legacy harness)

Canonical v2 uses BridgeKit logging and `RUST_LOG`; see `docs/running-v2.md`.
The legacy flags and default filter below apply to the regression harness.

Log levels and `kind` fields follow the fleet policy at
`~/Documents/Projects/Store/docs/guides/logging-policy.md`:

- **ERROR** — a code change is warranted; fails any test baseline.
- **WARN** — unexpected-but-handled; zero per hour on an idle healthy system;
  every WARN carries `kind = "<family>"`.
- **INFO** — lifecycle/state-transitions/designed degradations needing no action.
- **DEBUG/TRACE** — developer detail; free.

Every ERROR and WARN must carry a structured `kind` field. Common families for
this bridge: a broker outage is one episode — INFO `connection_lost` on its first
failed attempt, DEBUG retries, one WARN `external_failure` past 30 s (`attempts`,
`down_for_ms`), INFO `external_recovered` once a connection has held for 15 s;
`command_rejected` (a DALI command failed: its error also goes to the status
topic), `config_save_failed`, `signal_handler_unavailable`, `decode_error`
(undecodable MQTT payload), `protocol_mismatch` (unexpected packet or emulator
unsupported command).

The bridge participates in **distributed traces** via MQTT v5 `traceparent` user
properties (`tracing_init::traceparent`): inbound command packets read the
`traceparent` property and re-parent the handling span before entering it;
outbound publishes stamp the property when a trace is active. The
`tracing_init::traceparent::current()` / `set_remote_parent()` helpers are
gated behind the `otel` feature (already enabled).

tracing-init is always initialised. `logging.toml` (searched upward from the working directory,
which the systemd units set to the binary's home) or the `LOG_DESTINATION` env var chooses the
destinations — the committed `logging.toml` adds GELF and OpenTelemetry to logmon — and `--console`
and `--log` add the console and a file (`logs/dali.<date>.log`); `LOG_DESTINATION` applies only
when neither flag is given. The default filter is
`warn,mqtt_dali=info` (`--filter`). A destination that cannot start is skipped
(`on_destination_error` is pinned to skip in code), and logging that cannot start at all never
stops or panics the bridge. tracing-init bounds the rest: starting the file and GELF destinations
takes at most 5 s (then the destination is skipped, WARN `log_destination_skipped`), console, file
and GELF writes never block (a full queue drops lines, WARN `log_lines_dropped`), and dropping the
guard takes at most about 4 s.
