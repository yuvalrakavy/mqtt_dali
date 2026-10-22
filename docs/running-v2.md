# Running the v2 development daemon

This is an unfinished development version. Its current executable serves saved
inventory and metadata edits through BridgeKit, checks ATX I4 UART health and
supports [base light control](mqtt-outputs.md) plus
[native input State/events](mqtt-inputs.md). Commissioning, input discovery and
the web interface remain in development.

Start one controller with an MQTT broker and its own writable config directory:

```sh
mqtt_dali --instance Controller --broker 127.0.0.1:1883 \
  --config /var/lib/mqtt_dali/dali.v2.json \
  --legacy-config /var/lib/mqtt_dali/dali.json \
  --uart /dev/serial0 --log-config /etc/mqtt_dali/logging.toml
```

The saved controller name must match `--instance`. To develop without hardware,
use `--uart none`, or supply the private PTY from `dali_hat_emulator`. The broker
can have any MQTT consumer. Retained `DALI/State/<controller>/__inventory`
contains the saved inventory; `DALI/Version/<controller>` reports version and UTC
build time. See [metadata commands](service-development.md#mqtt-metadata-commands)
and the [inventory contract](inventory-contract.md). Saved mappings do not prove
that a fixture is present or that a hardware command succeeded.

The v2 file takes precedence. If it is absent, startup imports the v1 file into a
separate private v2 file, or initializes an empty controller if neither exists.
Invalid v2 fails startup. v1 bytes remain untouched. Stop the daemon before
deleting v2 to force reimport. Its identity and revision ledger live inside the
v2 JSON. The adjacent `.lock` file provides lifetime ownership; do not remove it
while a process is using the configuration. Only one process may own that
configuration. Offline validation/import uses:

```sh
mqtt_dali --check-config --name Controller \
  --config ./dali.v2.json --legacy-config ./dali.json
```

`mqtt_dali_v2` calls the same entry point. `mqtt_dali_v1` in this source tree is
an automated legacy regression harness. Use the v1 maintenance branch/tag for
the deployed legacy bridge and its original CLI/MQTT contract.

BridgeKit owns signal handling, tracing-init, MQTT reconnects and bounded stop.
SIGINT/SIGTERM requests shutdown. An owner that cannot actually finish produces
a failed/incomplete stop; a supervisor may restart it. UART and metadata ownership
continue through broker outages. UART health uses only Version/bus-status queries
and the conservative unverified repeat policy. Individual PowerState/Brightness
operations use owned physical jobs and actual readback; see the
[MQTT output guide](mqtt-outputs.md). Further capability paths remain in progress.

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

The shared dependency commit is currently local to the development branch.
Publish it and complete release validation before promising a reproducible fresh
clone or deploying v2. No install/deployment scripts are switched by this slice.
