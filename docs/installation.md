# Install and operate v2

These instructions are for a Raspberry Pi operator with a running MQTT broker
and an ATX HAT. Use a v2 archive for your operating system architecture. ARMv7
musl is the 32-bit Pi target; a 64-bit OS needs an AArch64 Linux build. The
installer checks architecture before changing the installation. Python 3.9 or
later, systemd, `useradd`, and the `dialout` group are required for installation.

V1 retains `mqtt_dali`. V2 installs `mqtt_dali_v2` and a separate service, config
and state directory. The existing `raspberry-install*.sh` scripts belong to v1;
they are not the v2 installer.

## Verify and install a package

Obtain the archive and its `.sha256` file from the same trusted release source.
Substitute the actual archive filename below:

```sh
sha256sum -c mqtt_dali_v2-<release>.tar.gz.sha256
tar -xzf mqtt_dali_v2-<release>.tar.gz
cd mqtt_dali_v2-<release>
sudo python3 install.py
```

Checksums detect damaged or changed contents; they are not a publisher signature.
The installer also checks the extracted manifest, payload and architecture. It
creates a version directory under `/opt/mqtt_dali_v2/releases`, selects it through
`/opt/mqtt_dali_v2/current`, and installs these paths:

| Path | Purpose |
|---|---|
| `/usr/local/bin/mqtt_dali_v2` | Bridge executable through `current` |
| `/usr/local/bin/dali_hat_emulator` | Independent emulator executable through `current` |
| `/etc/mqtt_dali_v2/bridge.env` | Service settings, initially mode 0600 |
| `/etc/mqtt_dali_v2/logging.toml` | Logging destinations, initially console/journal |
| `/var/lib/mqtt_dali_v2` | State directory owned by `mqtt-dali-v2` |
| `/etc/systemd/system/mqtt_dali_v2.service` | Optional systemd service |

Existing config files retain their contents and permissions. Unrelated files at
managed executable/link/unit paths cause refusal instead of replacement. The
installer creates a non-root `mqtt-dali-v2` account if absent; an existing account
must have its own matching primary group. The unit adds `dialout` access for UART.
The installer runs `daemon-reload` but **does not enable, start, or restart** the
bridge. Reinstalling the identical package is supported.

To inspect installation without host changes, use `python3 install.py --root
/tmp/dali-stage`. This stages production paths below that directory, without
account management, systemd calls or binary execution. Absolute links intentionally
name their production targets; the staged tree is not an executable installation.

## Configure and start

Edit `/etc/mqtt_dali_v2/bridge.env` as root. Set `MQTT_DALI_INSTANCE` to the
controller name and `MQTT_DALI_BROKER` to your broker's `host:port`. A saved
controller's name must match the configured instance. The default UART is
`/dev/serial0`. Prepare the Pi UART and HAT according to their hardware guidance;
the installer does not change the UART console, power supply, or networking.

Repeated-command completion defaults to `unverified`. Select `1` or `2` only for a
measured HAT profile; commissioning/configuration operations need this evidence.
For the included emulator's default profile, use `1`. See
[address assignment](commissioning.md) for the limits.

Web access is optional. To enable it, set all three values in `bridge.env`:

```ini
MQTT_DALI_WEB_BIND=127.0.0.1:8080
MQTT_DALI_WEB_ORIGIN=http://localhost:8080
MQTT_DALI_WEB_PASSWORD="your own password"
```

This is systemd EnvironmentFile syntax, not a shell script; do not add `export`
or command substitutions. Set a browser-reachable origin when using another
device, or configure an HTTPS reverse proxy and keep the listener on loopback.
See [web access](web-access.md) for origin, TLS and authentication details. Keep
the env file root-owned/private. The launcher passes the password in the environment,
not process arguments. Leaving web settings unset creates no web listener.

For emulation, set `MQTT_DALI_HAT_TCP=127.0.0.1:5580`; TCP takes precedence over
the UART default, so the launcher passes only one transport. The emulator is a
separate process and is not started by the bridge service. See
[the emulator guide](emulator-web.md).

After stopping any other process using this HAT, start explicitly:

```sh
sudo systemctl enable --now mqtt_dali_v2.service
systemctl status mqtt_dali_v2.service
journalctl -u mqtt_dali_v2.service -n 100 --no-pager
```

The initial controller is empty unless imported below. Use the protected browser
to set up a bus and discover/assign fixtures. Saved inventory does not prove a
device is currently present. The binary's `--check-config` validates/imports
configuration without opening a UART, MQTT connection or web listener.

## Import existing v1 configuration

Stop v1 before copying a consistent configuration, and retain its original file.
Copy it to the v2 import path, substituting the actual source:

```sh
sudo systemctl stop mqtt_dali.service
sudo install -o mqtt-dali-v2 -g mqtt-dali-v2 -m 600 \
  /path/to/v1/dali.json /var/lib/mqtt_dali_v2/dali.json
sudo -u mqtt-dali-v2 /usr/local/bin/mqtt_dali_v2 --check-config --name Controller \
  --config /var/lib/mqtt_dali_v2/dali.v2.json \
  --legacy-config /var/lib/mqtt_dali_v2/dali.json
```

Use your actual legacy service name and saved controller name. If v2 is absent,
validation/startup imports the copy into a separate v2 file. If v2 exists, it takes
precedence. Invalid v2 fails validation; it does not silently fall back. V1 bytes
stay unchanged. To force reimport, stop v2 and back up/remove only its v2 JSON
before validating again. Never remove a lifetime `.lock` while a process owns it.
Review the resulting inventory before starting control. This does not switch
the legacy consumer to BridgeKit MQTT; update that consumer separately.

## Upgrade and back up

Stop the bridge before taking a consistent copy of its config and state. For example:

```sh
sudo systemctl stop mqtt_dali_v2.service
sudo tar -czf /root/mqtt_dali_v2-backup.tar.gz \
  -C / etc/mqtt_dali_v2 var/lib/mqtt_dali_v2
```

Protect the backup: it includes the web password. Install the new verified package
as above, review its release notes/configuration, then explicitly start the service.
The installer preserves old release directories and the config/state files. It
does not migrate state by running the new binary; startup performs applicable
validated migrations. Check version/build in the sign-in screen or retained MQTT
Version data, and inspect the logs and actual fixture/input behavior.

## Roll back

Stop v2 first. Run `sudo python3 install.py` from the previous verified release
directory/package to select its binaries again, then explicitly start the service.
Its installer preserves your current state and config. If that binary cannot read
the newer state schema, restore the reviewed pre-upgrade backup while stopped;
do not assume every future schema is backward compatible. Restore ownership and
private permissions as recorded in the backup. Do not delete saved state merely
to bypass a validation failure.

To return to v1, stop/disable `mqtt_dali_v2.service`, then start your maintained
v1 service with its untouched `mqtt_dali` binary and original v1 configuration.
V2 edits do not export themselves back into v1. Neither binary selection nor
state restoration programs fixture addresses, groups or scene memory; review
any hardware changes made during v2 operation before resuming v1.

Old releases can be removed explicitly when no running process/current link or
rollback plan uses them. No automatic pruning is performed.
