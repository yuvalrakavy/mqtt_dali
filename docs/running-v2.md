# Running the v2 development daemon

This is a development release candidate. For packaged service installation, see
[installation and rollback](installation.md). Its executable serves saved
inventory and metadata edits through BridgeKit, checks ATX I4 UART health and
supports [live bus setup](bus-setup.md),
[explicit saved-fixture removal](remove-from-list.md),
[base and selected-family DT8 light control](mqtt-outputs.md),
[input State, current-value queries and native events](mqtt-inputs.md),
[discovery of already addressed devices](discovery.md) and
[one-at-a-time unaddressed output/input assignment](commissioning.md).
An [optional authenticated web interface](web-access.md) serves protected saved
inventory/live status, configuration and commissioning screens, including full
reassignment and lost-address recovery. See the [support matrix](support-matrix.md)
for implemented capabilities and explicit limits. Web is disabled by default and requires an
explicit password; there is no shared default.

Start one controller with an MQTT broker and its own writable config directory:

```sh
mqtt_dali_v2 --instance Controller --broker 127.0.0.1:1883 \
  --config /var/lib/mqtt_dali_v2/dali.v2.json \
  --legacy-config /var/lib/mqtt_dali_v2/dali.json \
  --uart /dev/serial0 --log-config /etc/mqtt_dali_v2/logging.toml
```

The saved controller name must match `--instance`. To develop without hardware,
use `--uart none`, supply the private PTY from `dali_hat_emulator`, or select a
raw TCP HAT endpoint instead:

```sh
mqtt_dali_v2 --instance Controller --broker 127.0.0.1:1883 \
  --config ./dali.v2.json --legacy-config ./dali.json \
  --hat-tcp 127.0.0.1:5580 --uart-repeat-terminals 1
```

`--hat-tcp` accepts numeric IPv4 or bracketed IPv6 and a nonzero port. Do not
also pass `--uart`, including `--uart none`. TCP carries the same raw ATX bytes
as UART, including asynchronous input events; it is not HTTP/WebSocket.
The bridge's optional web UI remains separate, with the existing web options.
Match the emulator's one/two-terminal profile explicitly; TCP does not infer it.
The endpoint must start a fresh HAT parser/output session per connection while
retaining native fixtures. Disconnect retires old observed evidence; the bridge
reconnects and initializes without replaying a failed command. The quiet interval
applies to both transports. Numeric endpoints avoid DNS workers. Plain TCP is
for a local or explicitly trusted emulator; web authentication does not protect
the raw HAT endpoint.

Run the standalone emulator with its separate configuration UI:

```sh
dali_hat_emulator --scenario examples/emulator/colour-i4.json \
  --hat-listen 127.0.0.1:5580 --web-listen 127.0.0.1:5581
```

Open `http://127.0.0.1:5581` beside the bridge's own authenticated web UI.
Use the emulator to configure fixtures, stimulate buttons/sensors and observe
native changes caused by the bridge. PTY/control-socket mode remains available.
See [the emulator UI guide](emulator-web.md) for scenario capture/reset and the
exact loopback security/access contract.

The broker
can have any MQTT consumer. Retained `DALI/State/<controller>/__inventory`
contains the saved inventory; `DALI/Version/<controller>` reports version and UTC
build time. See [metadata commands](service-development.md#mqtt-metadata-commands)
and the [inventory contract](inventory-contract.md). Saved mappings do not prove
that a fixture is present or that a hardware command succeeded.

Address assignment additionally needs a known repeated-command completion profile:
`--uart-repeat-terminals unverified|1|2` (default `unverified`). Ordinary control
keeps its existing default behavior. See the commissioning guide before choosing
a measured hardware or explicit emulator profile.

The v2 file takes precedence. If it is absent, startup imports the v1 file into a
separate private v2 file, or initializes an empty controller if neither exists.
For a new installation, configure its buses before starting fixture discovery;
see [bus setup](bus-setup.md).
Invalid v2 fails startup. v1 bytes remain untouched. Stop the daemon before
deleting v2 to force reimport. Its identity and revision ledger live inside the
v2 JSON. The adjacent `.lock` file provides lifetime ownership; do not remove it
while a process is using the configuration. Only one process may own that
configuration. Offline validation/import uses:

```sh
mqtt_dali_v2 --check-config --name Controller \
  --config ./dali.v2.json --legacy-config ./dali.json
```

The v2 executable is `mqtt_dali_v2`; v1 retains `mqtt_dali`. They can be
installed in the same binary directory without replacing each other. The v2
build does not produce an unversioned `mqtt_dali` executable. Build or install
v2 explicitly with `--bin mqtt_dali_v2`; plain `cargo run` selects it by default.

`mqtt_dali_v1` in this source tree is an automated legacy regression harness.
Use the v1 maintenance branch/tag for the deployed legacy bridge and its
original CLI/MQTT contract. Each version keeps its own configuration file; stop
the current service before switching versions on the same physical HAT.

BridgeKit owns signal handling, tracing-init, MQTT reconnects and bounded stop.
SIGINT/SIGTERM requests shutdown. An owner that cannot actually finish produces
a failed/incomplete stop; a supervisor may restart it. UART and metadata ownership
continue through broker outages. UART health uses only Version/bus-status queries
and the conservative unverified repeat policy. Individual PowerState/Brightness
operations use owned physical jobs and actual readback; see the
[MQTT output guide](mqtt-outputs.md) and the support matrix for other capabilities.

Startup and reconnects wait for **250 ms of actual UART silence** before sending
initialization queries. Change this with `--uart-quiet-ms 1..5000`. Every raw
received byte restarts the timer, including partial lines; complete input events
continue through the reader. An unresolved fully written command or receive
corruption uses the same recovery, followed by fresh initialization. Acquisition
has a total budget of the selected interval plus 5 s; continuous traffic causes
a bounded failure and paced retry. No persistent UART journal or first-start
HAT reboot is required.

A known interrupted/partial UART write still requires a fresh HAT Boot: receive
silence cannot establish the state of its command parser. That evidence survives
an in-process port reopen, including poison discovered during teardown. It is
not inferred across process death. Quiet recovery is the selected operational
policy, not a documented maximum execution delay or a transaction identifier;
the ATX same-bus reply ambiguity and physical I4 validation remain explicit limits.

Select logging with `--log-config`; `RUST_LOG` controls filtering, and
`LOG_DESTINATION` overrides destinations. The repository logging.toml contains
site-specific GELF/OTLP addresses: configure them for your installation. Logging
destinations are lossy and startup skips unavailable destinations. Optional MQTT
v5 traceparent metadata follows requests through worker and metadata queues.
Tests use only isolated loopback brokers, PTYs and a local OTLP collector;
they do not establish live Logmon delivery or physical I4 compatibility.

The shared SDK is pinned to a published HTTPS Git revision; see
[release builds](release.md). Use the versioned [v2 installer](installation.md)
for service installation. Legacy install scripts remain v1-only. Physical I4
acceptance and actual service activation remain deployment checks.
