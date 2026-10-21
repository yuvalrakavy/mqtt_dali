# Managed inventory service (v2 development)

This guide is for bridge contributors and embedders. The service library owns
durable configuration and runtime routing; UART integration and the canonical
v2 daemon are the next slice. `mqtt_dali_v2` currently remains a configuration
check command. Web handlers and hardware capabilities are not available yet.

`DaliService::new(paths, instance)` returns a resource-free service and a
`ServiceHandle`. Start it with a fixed Bridge's `start_managed`/`run_managed`.
Use the same instance as the saved controller name. The service checks both its
constructor identity and the actual LinkContext identity before activation.
Configuration opens inside the managed runtime after framework startup, not in
the constructor. All tracing-init repository dependencies are pinned to one
revision in Cargo.toml/lock; that development commit is currently local and must
be published before promising a portable fresh clone or release.

The first proxy connection attaches the runtime Reporter before any HAT work.
This is independent of MQTT availability. Inventory and metadata editing therefore
remain available through retained frontend handles during a broker outage.
Snapshot fields distinguish routing phase and hardware facts; `NotStarted`
currently means exactly that. Version and UTC build time accompany the saved
controller identity. Resource shutdown remains the managed runtime's ownership
responsibility even if a filesystem worker is blocked.

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

`__inventory` is read-only and returns the complete observed saved inventory.
Physical property writes, reads and unsupported operations return explicit
Unsupported errors in this slice. Saved inventory does not prove physical state.
Consumers can be any MQTT implementation; no Store service is required. Follow
the [inventory contract](inventory-contract.md), especially replay buffering and
consumer-owned intent continuity.

## Local verification

Tests use ephemeral loopback FakeBroker instances, temporary private config
directories and controlled filesystem faults. They cover actual MQTT replies and
an independent consumer, cancellations, sync/CAS failures, original deadlines,
controller identity, outage independence, notification lag and blocked-owner
shutdown. The declared guard-removal controls are in
`v2-service-controls.toml`, with runner results beside it. The complete native
suite also covers the existing migration/UART/emulator and legacy regressions.
Pi checks compile the library/v2/emulator for armv7; physical I4 behavior, live
Logmon delivery and Pi runtime still require later verification.
