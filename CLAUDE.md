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

`dali::input` decodes standard Part 103 source schemes and native Part 301–304
data, and builds individual read-query frames; see `docs/dali-input-contract.md`.
It is a pure codec. Named input discovery, MQTT state/events and configuration
are subsequent integration work, not implied by successful decoding alone.

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
