# P3c3c: canonical BridgeKit daemon

## Intent and settled boundaries

Continue the approved T3 v2 architecture from `7ef11d7`. Promote the reviewed
managed inventory/UART service into the executable people run. One controller
process uses BridgeKit's fixed-instance runtime, shared tracing-init, signals,
watchdog and bounded shutdown. MQTT consumers need no Store server. v1 remains
the separate Git maintenance branch/tag; v2 never accepts the old wire protocol.
No HTTP listener, fixture Send/capability engine, deployment, push or release is
part of this slice. UART health retains the documented P4 crash/reset-evidence
prerequisite before mutating fixture operations are introduced.

## Grounding and chosen implementation

Current `src/main.rs` is the legacy custom process. The development
`src/bin/mqtt_dali_v2.rs` implements a bounded, isolated configuration check.
`ConfigStore` already calls `load_or_migrate` under its lifetime lock, inside the
managed runtime; daemon startup needs no pre-read or second migration owner.
`Bridge::run_managed` owns tracing-init initialization, stop registration before
logging/resources, startup cancellation and actual service-runtime teardown.
Its fixed-instance CLI requires `--instance` and `--broker`, supports
`--log-config`, timing overrides and declared value options. Constructors do no
I/O. The saved controller name must match the requested instance, as verified
inside service run after load and again at actual proxy attachment.

Use that existing process API verbatim. Add `src/daemon.rs` as the small CLI
composition layer; it builds a resource-free service and calls run_managed.
Move the existing configuration-check implementation to `src/config_check.rs`
and call it only when `--check-config` is present. Keep its current isolated
file semantics/output deadlines and existing process tests. Daemon options must
never fall through to the legacy parser or pre-initialize logging/resources.
Preflight uses the shared config parser and option table, then renders help or
usage errors through the existing bounded output helper. Direct shared usage
printing was observed blocking on a full unread pipe; valid arguments still
pass unchanged through Bridge's common parser.

Promote `src/main.rs` to a thin canonical v2 wrapper, with the development
`mqtt_dali_v2` entry point calling the same composition function. Move the old
main verbatim to `src/legacy_main.rs`, an explicitly declared
`mqtt_dali_v1` regression harness, so its module paths and established no-hang
tests remain intact. Update those process tests to invoke that harness. This
harness is not a second v2 runtime or a v1 release: operational legacy builds
come from the preserved v1 branch/tag. Bump the package version to
`2.0.0-dev.0`; Version/snapshots identify the current development executable.
No production install scripts switch hosts in this checkpoint.
The legacy test-log helper moves to `src/test_log_tests.rs` with an explicit
test-only path so wait-lint recognizes its non-production scope after relocation.

## Command line and configuration

The daemon uses BridgeKit's common flags, plus these declared value options:

| Option | Meaning/default |
|---|---|
| `--config` | Current v2 file, `dali.v2.json` |
| `--legacy-config` | Read-only import source, `dali.json` |
| `--uart` | UART/PTY path, `/dev/serial0`; literal `none` disables hardware |

`--instance` is mandatory and is the new controller name when creating an empty
inventory. Existing v1/v2 keeps its name; a mismatch fails startup before UART
work. `--check-config --name` retains the existing offline import/check behavior;
it is not a daemon identity override. Empty UART/config values are usage errors.
The repeat policy remains Unverified. Health-only startup does not guess a
firmware profile or publish unsupported physical state. BridgeKit's root default
is DALI and its optional root override remains available.

Set app_name to mqtt_dali for either v2 entry point and retained Version to
`mqtt_dali 2.0.0-dev.0` plus actual UTC build time (the bridge kind remains first).
Use the existing repository
logging.toml; update its commentary for shared-runtime defaults and
`--log-config`/LOG_DESTINATION. Keep skip-on-destination-error policy in shared
code. Tests always supply a private local logging config and scrub inherited
telemetry environment overrides, so no live Logmon or LAN endpoint is contacted.
Logging/tracing compatibility is integration of the shared implementation, not
a claim that a live Logmon domain has been tested.

## Process fault seams

For subprocess verification, extend the existing debug-only configuration-gate
environment convention into the durable worker. Before initial load,
`MQTT_DALI_V2_TEST_CONFIG_GATE` may name a controlled FIFO read; after the mapping
rename, `MQTT_DALI_V2_TEST_SAVE_GATE` may hold the worker similarly. Tests own
and release these gates. They are absent from release builds and inert when
unset; they do not become operator configuration. tracing-init deliberately
ignores a FIFO as a config file, so it cannot hold logging startup. Enable the
existing mqtt-bridge-kit `test-seams` feature through an explicit default-off
`process-test-seams` feature, only for its dedicated process test. The shared kit
also requires build environment `MQTT_BRIDGE_TEST_SEAMS=1`; ordinary tests/builds
remain feature-free. That test-built child can use
`MQTT_BRIDGE_TEST_LOGGING_GATE` with a FIFO. Normal production builds do not
enable that feature. Verify the child reached its read
using a nonblocking writer-open handshake before signaling, and keep the gate
held until process exit. Reuse the shared seam rather than wrapping/replacing
its logging initializer.

SIGTERM during an unfinished managed filesystem owner must exit within the
shared process bound and report failure/incomplete stop, not success. A stop
during the shared logging start follows the shared process semantics. Clean
metadata-only or healthy PTY stop succeeds. No blocking worker is called
completed merely because the main process can exit under its outer runtime
shutdown bound. Retain v1 bytes in every fault/migration case.

## Files, risks and verification

Touch Cargo.toml/lock, new daemon/config_check modules and lib exports, canonical
and development wrappers, verbatim legacy-main relocation, legacy process test
binary selection, debug-only durable-worker seams, new tests/v2_process.rs,
wait registry, controls/ledger and contributor/operator/progress guides.
Relocate existing legacy control source paths/binary names and the configuration
check control paths too; verify every anchor still resolves after relocation.
No shared-runtime edit is planned. If its public process API proves inadequate,
stop reimplementation and review the concrete shared correction first.
The actual OTLP daemon test exposed lost trace metadata across shared queues.
That correction was separately designed/reviewed in tracing-init's
`2026-10-21-bridge-request-traces.md`; this slice pins its reviewed revision
and carries the admitted context across the DALI actor too.

Primary risks are executing legacy code by default, touching UART before config
validation, using a second migration owner, a process falsely reporting clean
stop with blocked filesystem work, accidentally contacting live telemetry from
tests, and misreporting development version/build. Verify each at its real
boundary rather than asserting only the composition helper's return values.

Use failing-first subprocess cases for canonical v2 migration and MQTT inventory,
metadata rename/restart with original-token reply, both configuration precedence
and preserved v1 bytes, invalid/mismatched inventory refusing UART work, broker
outage with independent PTY health, actual traceparent propagation (isolated
local HTTP OTLP collector, since file-only logging installs no OTel layer), clean stop,
SIGTERM during logging-start/config-start/save gates, and undrained diagnostic
pipes where relevant. Reuse ephemeral FakeBroker and the standalone ATX emulator
or explicit real PTY transcript; no live broker/HAT/LAN services. Retain the
existing offline check and legacy no-hang suite. Inspect emitted local logs for
the real application/runtime and startup/stop verdicts; do not equate compilation
with live Logmon delivery. Sample idle CPU for the runnable development daemon
against the local emulator.

Run complete native tests, scoped strict Clippy including new process tests,
changed-file formatting, generated wait registry, armv7 canonical/v2/emulator
compile and meaningful declared negative controls followed by restored full
tests. Two sequential source design reviews and a fresh code review are required
under T3. Update the service/support guides, logging comments and progress ledger
with exact gates and remaining physical/portable-dependency limitations. Existing
architecture approval authorizes this implementation; no new user decision is
needed.

## Implemented verification

Canonical and alias entry points now use the managed service. The shared
request-trace correction is pinned consistently at local commit
96a03bcb9f932d8d49d479a38cc95f9ad9e08ff6. The real OTLP JSON acceptance checks
worker and metadata-actor operations under the incoming trace, committed
Inventory with its new generation/original trace, and the original-context
successful reply. No live Logmon destination is used.

Source review approved after fixes for independent Inventory-publication waiting,
flag-position mode detection (including a controller named --check-config), and
the wait-table Markdown separator. Review was same-family and source-only.
The CLI-value regression was observed RED before its fix. The earlier direct
usage print was also observed blocking on full unread output before bounded
rendering was added. Legacy main differs only in its explicit test-helper path;
the moved helper is byte-identical.

Final restored verification: 225 full native tests passed; strict scoped Clippy,
armv7 library/all four binaries, formatting/wait/diff checks passed. The explicit
default-off test-feature logging-start process test passed separately. Sixty
controls failed at intended behavioral assertions (5 daemon, 1 changed recovery,
5 relocated migration, 49 relocated legacy); their ledgers are retained and the
restored source passed the final gate. No upstream publication, deployment,
physical I4, live broker/Logmon or Pi runtime/linking validation was performed.
