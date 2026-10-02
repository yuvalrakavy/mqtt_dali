# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

mqtt_dali is a Rust service that bridges MQTT and DALI (Digital Addressable Lighting Interface) protocols, enabling remote control of DALI lighting buses via MQTT commands. It targets Raspberry Pi (ARMv7) deployment.

## Build Commands

The default build target is `armv7-unknown-linux-musleabihf` (set in `.cargo/config.toml`). To cross-compile for Pi:

```bash
cargo build --release
```

To build for the local machine (e.g., macOS for development):

```bash
cargo build --target aarch64-apple-darwin   # Apple Silicon
cargo build --target x86_64-apple-darwin    # Intel Mac
```

The DALI hardware driver (`dali_atx`, via `rppal`) is Linux-only, so off the Pi the bridge runs
with `--emulation` only; check the Pi build with `cargo check` (the default target). The HAT's
serial protocol (`hat_line`) sits behind a `ByteSource` trait, so its loops are tested on the Mac
against fake byte sources. Tests run on the Mac against the emulator and an in-process fake broker
on 127.0.0.1 (`tests/process.rs` runs the built binary against it):

```bash
cargo test --target aarch64-apple-darwin
```

Every wait carries a `// WAIT: <row>` tag, on its own line above the statement, naming a row of
`docs/wait-registry.md`, which `tests/wait_registry.rs` checks (Store's no-hang spec §13.3, §14).
The MQTT event loop is polled by `Pump`, a task that waits on nothing else; the session never polls
(§14.3). The session runs on a thread of its own (`mqtt::spawn`), since its DALI calls are
synchronous; SIGTERM or SIGINT stops it, and `main` waits for it at most `SHUTDOWN_BOUND`
(`shutdown.rs`). The stop handlers are installed first thing, before logging, the configuration
and the hardware; start-up runs on the blocking pool, raced with a stop; the configuration file is
written off the session, under `SAVE_BOUND`; and `main` ends the runtime with `shutdown_timeout`,
since dropping it would wait without limit for a blocking thread stuck in the kernel. The bus
protocol's retry loops are bounded in sends and in time (`dali_manager::BUS_LIMITS`).
`docs/no-hang-3b-controls.toml` holds the negative controls for every guard.

Linting and formatting (via trunk):

```bash
cargo clippy
cargo fmt
```

Run with emulation mode (no DALI hardware needed):

```bash
cargo run -- --emulation <mqtt_broker_address>
```

Run in setup mode (interactive TUI for configuration):

```bash
cargo run -- --setup <mqtt_broker_address>
```

## Architecture

### Module Dependency Flow

```
main.rs → mqtt.rs → dali_manager.rs → dali_atx.rs (hardware)
                                     → dali_emulator.rs (testing)
```

### Key Modules

- **main.rs** — Entry point, CLI parsing (`rustop`), logging init, controller instantiation
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

## Deployment

Cross-compiled binary is deployed to Raspberry Pi via `install_to_pi` (Fish script) and installed as a systemd service (`dali.service`). The `install_on_pi.sh` script handles on-device setup.

## Key Dependencies

- **tokio** — async runtime
- **rumqttc** — MQTT client v5 (`rumqttc::v5`) (default features disabled)
- **rppal** — Raspberry Pi GPIO/UART access
- **error-stack** / **thiserror** — error handling
- **tracing** + **tracing-init** (git dep, `otel` + default features) — structured logging
- **serde** / **serde_json** — serialization

## Logging

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
stops or panics the bridge.
