# Managed inventory service (v2 development)

This guide is for bridge contributors and embedders. The service library owns
durable configuration, runtime routing, optional UART health and individual-output
library operations. The canonical
`mqtt_dali` executable now runs it through BridgeKit; `mqtt_dali_v2` is an alias.
`--check-config` remains an explicit offline operation. See
[running the development daemon](running-v2.md). MQTT base fixture properties
use [the output wire contract](mqtt-outputs.md). Web handlers are pending; embedders can use the
[single-output service API](output-service.md).

`DaliService::new(paths, instance)` returns a resource-free service and a
`ServiceHandle`. Start it with a fixed Bridge's `start_managed`/`run_managed`.
Use the same instance as the saved controller name. The service checks both its
constructor identity and the actual LinkContext identity before activation.
Configuration opens inside the managed runtime after framework startup, not in
the constructor. All tracing-init repository dependencies are pinned to one
revision in Cargo.toml/lock; that development commit is currently local and must
be published before promising a portable fresh clone or release.

The first proxy connection attaches the runtime Reporter independently of UART
setup and MQTT availability. Inventory and metadata editing therefore
remain available through retained frontend handles during a broker outage.
Snapshot fields distinguish routing phase and hardware facts; `NotStarted`
means no UART was enabled. Version and UTC build time accompany the saved
controller identity. Resource shutdown remains the managed runtime's ownership
responsibility even if a filesystem worker is blocked.

## Optional UART health

Call `service.with_uart(path, RepeatPolicy::Unverified)` before managed startup.
Optionally follow it with
`with_uart_quiet_period(Duration::from_millis(250))?`; the builder validates a
1 ms..5 s interval and requires UART to be enabled first. The builders open
nothing. The managed service owns one Serial/Transactions
session. Background health uses HAT Version and enabled-bus Status queries;
explicit library output operations share that same owner. Hardware
snapshots distinguish Connecting, Resynchronizing, Initializing, Ready (raw version strings and
bus observations), unavailable retry, waiting for a late reply, ResetRequired
and Closed. Bus power loss remains a bus fact, rather than disconnecting a
responding HAT. Raw input frames are drained but do not yet produce input actions.

A timed-out complete write keeps its owner; a delayed eligible terminal can
retire it, or measured quiet recovery retires its unresolved receive ordering.
Startup/reopen and receive corruption also acquire a quiet boundary before
fresh initialization. The default is 250 ms, with an acquisition budget of the
interval plus the setup allowance (5 s by default), followed by a separate V/D
setup job. Continuous raw-byte activity restarts the timer while complete events
remain observable; failed acquisition retries on the existing paced timer.
Version itself never overrides uncertain ordering.

Known partial-write poison requires an actor-accepted Boot beyond the existing
receive boundary. The owner preserves authoritative poison sampled after actual
helper/actor teardown across an in-process reopen; public Closed/status events
alone are insufficient. Quiet cannot clear the HAT's partial command parser.
There is no persistent UART journal. Quiet recovery is an operational policy,
not a hardware latency guarantee or perfect same-bus response attribution.
A serial-open timeout is terminal for this UART owner, avoiding an
accumulation of blocked openers; restart after resolving the underlying problem.

Helpers remain owned through cancellation and their completed panic results are
inspected before recovery. Hardware and disk cleanup share an absolute 1.5 s
local budget; the managed runtime's shared stop ceiling and actual destruction
remain authoritative. Retained frontend handles keep snapshots, not descriptors.
Low-level `Transactions::resynchronize(quiet, within)` owns exclusive admission,
keeps input events live, and requires a new explicit initialize after success.
Caller cancellation reaches reader reset authorization directly, even if the
actor has not been polled. Its event loop also checks the original deadline
before each select so ready input cannot starve expiry. Cancellation/expiry leaves
recovery required. Managed startup applies that
policy automatically. The MQTT surface exposes metadata, UART health and base
output properties. The output-service API provides physical reads/writes through the same
admission and actual teardown path.

Queued transaction requests take priority over idle input dispatch; command
admission still drains and classifies old records and fences the receive stream.
Canceled or expired callers therefore release admission even under continuous
input. The HAT owner processes at most 129 public receive attempts per turn, then
collects a completed helper through its actual join. This keeps recent transport
and bus observations ahead of older query results without starving completion.
Once close starts, later public records cannot cancel its owned helper. A Closed
query or status may arrive before public EOF; successful actual teardown permits
the paced retry, while an actual task failure remains fatal.

## Read and edit

`handle.snapshot()` clones an owned snapshot; no borrowed lock guard escapes.
`subscribe()` is a bounded notification stream. On lag, read a fresh complete
snapshot. Notifications are not a replayable edit log.

Public `MetadataEdit` operations rename an output/group/bus or update the
controller description. Supply the expected current inventory revision, original
item token (instance token for bus/description), and one absolute deadline.
Admission is nonwaiting and exclusive; Busy is a refusal to wait in another queue.
The actor rechecks phase/revision/token before saving. No public metadata call
can claim a physical replacement, address assignment or group-member change.
Those require verified commissioning jobs in a later slice.

Writing/Uncertain phases reject new edits. A lost response cannot undo a save:
inspect the latest owned snapshot before retrying. If it is Uncertain, explicitly
recover using the prior controller token; the owner reloads recognized bytes and
activates them only after runtime CAS/publication. A Ready snapshot already
contains the confirmed result and needs no recovery. Unknown disk edits are not
adopted. A persistent runtime authority conflict requires restart.

## MQTT metadata commands

Commands use `DALI/Command/<controller>` and exactly one MQTT v5
`target-generation` property. They are momentary WithReply commands; provide a
response topic and correlation data when awaiting the result.

| Command | JSON fields in addition to `command` | Generation |
|---|---|---|
| `rename` | `target`, `name`, `expected_revision` | Current output/group token |
| `rename_bus` | `bus`, `name`, `expected_revision` | Controller token |
| `describe` | `description` (string or null), `expected_revision` | Controller token |
| `recover` | None; valid only while uncertain | Prior confirmed controller token |

For example, a rename payload is
`{"command":"rename","target":"Bus1.Outputs.Light1","name":"Kitchen","expected_revision":1}`.
The service captures the exact active command's completion capability and passes
it to its owner. A successful durable change returns one reply containing
`"value":{"revision":2}`, with the original request token/correlation. This
does not reuse the retired token for hardware work. All other queued old work
retires. HTTP/embedded handle edits use ordinary runtime retirement without that
capability.

Optional MQTT v5 `traceparent` follows each command through BridgeKit's worker
and the metadata actor. Inventory publication uses the committed generation
with the original trace; the success reply keeps the original request generation,
correlation and cause. The shared runtime validates trace metadata independently
of generation admission. Local OTLP tests verify exported worker/actor operation
spans, rather than only an ingress span.

`__inventory` is read-only and returns the complete observed saved inventory.
Individual PowerState/Brightness writes and reads use the owned physical service.
Groups, color and other unsupported properties return explicit Unsupported errors
in this slice. Saved inventory does not prove physical state.
Consumers can be any MQTT implementation; no Store service is required. Follow
the [inventory contract](inventory-contract.md), especially replay buffering and
consumer-owned intent continuity.

## Local verification

Tests use ephemeral loopback FakeBroker instances, temporary private config
directories, real Unix PTYs, the independent ATX emulator and controlled faults.
They cover actual MQTT replies and
an independent consumer, cancellations, sync/CAS failures, original deadlines,
controller identity, outage independence, notification lag and blocked-owner
shutdown, late/reset/reopen boundaries, helper panic handling and blocked-opener
incomplete stop. The declared guard-removal controls are in
`v2-service-controls.toml`, `v2-uart-service-controls.toml` and
`v2-quiet-controls.toml` and `v2-progress-controls.toml`, with runner results beside them. The complete native
suite also covers the existing migration/UART/emulator and legacy regressions.
Pi checks compile the library/v2/emulator for armv7; physical I4 behavior, live
Logmon delivery and Pi runtime still require later verification.
