# MQTT DALI bridge v2

Connect an ATX DALI HAT to an MQTT broker, configure its buses from an optional
password-protected web UI, and control lights or consume DALI-2 input events.
The broker can serve any compatible consumer; a Store server is optional.

V2 uses BridgeKit MQTT and tracing-init. Its executable is **`mqtt_dali_v2`**.
The maintained legacy release uses **`mqtt_dali`** and its original MQTT interface.
The two install side by side, with separate configuration files. Only one may
own a physical HAT at a time.

## Start here

- [Install, upgrade, import v1 configuration, or roll back](docs/installation.md)
- [Run the bridge directly](docs/running-v2.md) and [enable protected web access](docs/web-access.md)
- [Try the independent HAT emulator and its own web UI](docs/emulator-web.md)
- [Configure buses](docs/bus-setup.md), [discover devices](docs/discovery.md), and
  [assign addresses](docs/commissioning.md)
- [MQTT output control](docs/mqtt-outputs.md), [inputs](docs/mqtt-inputs.md), and
  [inventory contract](docs/inventory-contract.md)
- [Supported capabilities and explicit limits](docs/support-matrix.md)
- [Build and verify a release](docs/release.md)

Common operations include dimming and DT7 switching, DT8 temperature/xy/RGBWAF,
groups and opaque hardware scenes, input buttons/switches/sensors, indicator
feedback, lamp-failure status, device information and selected emergency-light
operations. See the support matrix before relying on a particular profile.

## Release status

This branch produces a **v2 development release candidate**. Local automated
checks use independent protocol peers and emulated HATs. Physical ATX I4 timing,
repeated-command completion behavior, and fixture interoperability require
hardware acceptance. A package build does not establish DALI certification.
The [progress tracker](docs/v2-progress.md) records the evidence and remaining work.

The web UI is disabled by default and has no default password. A broker, UART
access and appropriate DALI power/hardware are deployment prerequisites. No
installer changes networking, enables a Pi UART, or programs fixtures.
