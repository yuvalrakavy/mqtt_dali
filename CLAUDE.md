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
- **rumqttc** — MQTT client (default features disabled)
- **rppal** — Raspberry Pi GPIO/UART access
- **error-stack** / **thiserror** — error handling
- **tracing** + **tracing-init** (git dep) — structured logging
- **serde** / **serde_json** — serialization
