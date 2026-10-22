# Managed inventory service (v2 development)

This guide is for bridge contributors and embedders. The service library owns
durable configuration, runtime routing, optional UART health, individual-output
operations, selected-family DT8 MQTT controls and native input observations/actions. The canonical
`mqtt_dali` executable now runs it through BridgeKit; `mqtt_dali_v2` is an alias.
`--check-config` remains an explicit offline operation. See
[running the development daemon](running-v2.md). MQTT base fixture properties
use [the output wire contract](mqtt-outputs.md). Optional authenticated HTTP status,
sessions, SSE, scalar reads/applies and configuration/commissioning jobs use
[web access](web-access.md); embedded Preact assets and browser verification use
[browser development](browser-development.md). Embedders can use the
[single-output service API](output-service.md).

`DaliService::new(paths, instance)` returns a resource-free service and a
`ServiceHandle`. Start it with a fixed Bridge's `start_managed`/`run_managed`.
Use the same instance as the saved controller name. The service checks both its
constructor identity and the actual LinkContext identity before activation.
Configuration opens inside the managed runtime after framework startup, not in
the constructor. All tracing-init repository dependencies are pinned to one
revision in Cargo.toml/lock; that development commit is currently local and must
be published before promising a portable fresh clone or release.

`with_web(web::Settings::new(bind, origin, password)?)` enables the optional server
without opening resources in the builder. The managed owner binds after config
identity validates, before proxy readiness, and collects unexpected completion.
HTTP/HAT/config cleanup share the existing absolute deadline. Axum's outer task
is not the whole connection owner: cancellable accepted IO holds permits until
actual drop; clean teardown includes an all-permits barrier. Session/rate locks
never span await. Use `v2-web-controls.toml` for auth/lifecycle fault controls.
HTTP command jobs use a crate-private prepared façade around the same strict
command parser and concrete service resolvers. They supply no SDK command
completion and never publish around its worker. The original context/deadline
survives the bounded mailbox. At most four futures are polled inside the existing
Server task, with 32 process-local result records; no detached job task or new
physical owner. Stop drops receiver futures under the existing lifecycle ceiling.
Use `v2-web-job-controls.toml` for this boundary.
HTTP-only prepared read/apply actions reuse the scalar intent parser, current
2/4/6/8-second budgets and exact final acceptance used by MQTT. Shared pure
projections supply complete replacement snapshot states. Native HTTP events must
call `Notice::current` against the latest snapshot after awaits, then add the
original generation; `Notice::value` alone is not acceptance. Keep MQTT's payload
unchanged. The one pull stream rotates snapshot/job/native readiness, holds at
most one notice/loss, and uses one common broadcast-result conversion. Job watch
notifications happen after the book lock drops. See `v2-web-observation-controls.toml`.

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
responding HAT. Resolved input L frames now produce observed instance State and
native non-retained actions; see [MQTT inputs](mqtt-inputs.md). The same reader
obtains receive-only mapping boundaries without changing quiet recovery.

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

Public `MetadataEdit` operations select an output's public colour representation,
configure a bus by physical index (create, rename or enable/disable),
remove a saved output/input module (including its instances), create an empty named group,
rename an output/group/input module/input instance/bus or update the
controller description. Supply the expected current inventory revision, original
item token (instance token for bus/description), and one absolute deadline.
Admission is nonwaiting and exclusive; Busy is a refusal to wait in another queue.
The actor rechecks phase/revision/token before saving. Input and instance names
use the same MQTT `rename` command as outputs, with their exact qualified target.
Renaming a module rotates descendant mappings together; the original module
token still applies if only a child was renamed earlier. See the
[input inventory contract](inventory-contract.md). No public metadata call
can claim a physical replacement, address assignment or group-member change.
Group membership uses the owned [probe/save/program API](groups.md), with explicit
hardware adoption distinct from reviewed full-mask repair. Address assignment
uses the [commissioning API](commissioning.md).

`Snapshot.inputs` contains typed observed input data; `input_listeners` contains
routes with installed receive boundaries, not a device-presence claim.
`subscribe_inputs()` supplies a bounded capacity-64 native action stream; handle
Lagged as action loss. Snapshot watch updates may coalesce, and must not be used
to recreate button actions. Only the LinkContext-owned SDK worker helper reports
MQTT events. Device-link helper recreation starts a fresh receiver; broker
outages retain BridgeKit's existing bounded queued-event delivery policy.

`read_input(target, context, deadline)` queries a saved instance's physical type,
resolution and complete value through the shared physical owner. `Evidence`
distinguishes native Event from Query; polling never enters the Notice stream.
`physical::Work` pairs typed input/output requests and routes under the existing
Active/Stage, single permit/helper and fixed settlement ceiling. Frontend
cancellation does not prove backend retirement. Input caller expiry rejects a
read; generations and transport/bus stamps are rechecked at admission and final
acceptance. A newer native observation wins over a latched query; new receive
boundaries reset sequence tracking. MQTT selects a 4 s input budget within the
existing 15 s aggregate ceiling. See [input queries](mqtt-inputs.md#query-current-values)
and `v2-input-readback-controls.toml`.

Writing/Uncertain phases reject new edits. A lost response cannot undo a save:
inspect the latest owned snapshot before retrying. If it is Uncertain, explicitly
recover using the prior controller token; the owner reloads recognized bytes and
activates them only after runtime CAS/publication. A Ready snapshot already
contains the confirmed result and needs no recovery. Unknown disk edits are not
adopted. A persistent runtime authority conflict requires restart.

## MQTT metadata commands

`configure_bus` creates or updates a saved bus without a process restart; see
[bus setup](bus-setup.md) for strict fields, lifetime and disabled-cache behavior.

`remove_from_list` exposes the existing durable removal edits, leaving hardware
addresses/settings unchanged. It requires the selected output/module's current
item token. See [removal](remove-from-list.md) for subtree, group and rediscovery
semantics; the original command reply survives target retirement.

Addressed hardware discovery uses the same durable owner and original-command
completion but queries hardware before adopting additions. See
[addressed discovery](discovery.md) for the typed API, MQTT command, captured-probe
report and cancellation/save contract. It does not fabricate fixture State.

`assign_next_output` / `assign_next_input` add one unaddressed fixture/module using search, durable reservation
and programming phases under the same permit. Its structured outcome distinguishes
assignment from a reserved-but-unconfirmed address. See
[commissioning](commissioning.md) for the typed/MQTT contract, CLI repeat profile,
phase budgets and recovery limits. Inputs use explicit bounded 24-bit pairs,
independent address reservations and raw short addresses. The returned input module
has no instances until a separate addressed discovery fills actual types under its
continuing identity. Explicit recovery never resumes programming.

Commands use `DALI/Command/<controller>` and exactly one MQTT v5
`target-generation` property. They are momentary WithReply commands; provide a
response topic and correlation data when awaiting the result.

| Command | JSON fields in addition to `command` | Generation |
|---|---|---|
| `blink` / `stop_blink` | Individual output `target`; no other fields | Current output token |
| `configure_bus` | `bus` (0..3), `name`, `enabled` (bool), `expected_revision` | Current controller token |
| `remove_from_list` | Output/input-module `target`, `expected_revision` | Current selected item token |
| `rename` | `target`, `name`, `expected_revision` | Current output/group token |
| `set_color_representation` | Individual output `target`, `color_representation` (`temperature`, `xy`, `rgbwaf` or null), `expected_revision` | Current output token |
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

For example,
`{"command":"set_color_representation","target":"Bus1.Outputs.Light1","color_representation":"temperature","expected_revision":1}`
saves the public colour format and retires old requests without replacing the
fixture's physical identity. The field is required; null clears it. This metadata
command performs no UART operation and does not configure a physical colour mode.
The selected-family MQTT properties use the same owned physical service; see
[colour controls](mqtt-outputs.md#colour-controls). A setting alone does not prove
hardware capability or change persistent operating modes.

Optional MQTT v5 `traceparent` follows each command through BridgeKit's worker
and the metadata actor. Inventory publication uses the committed generation
with the original trace; the success reply keeps the original request generation,
correlation and cause. The shared runtime validates trace metadata independently
of generation admission. Local OTLP tests verify exported worker/actor operation
spans, rather than only an ingress span.

`__inventory` is read-only and returns the complete observed saved inventory.
Individual PowerState/Brightness writes and reads use the owned physical service.
Selected-family colour properties also use that service with fresh capability
preflight and actual readback. Groups and unsupported properties return explicit
errors. Saved inventory does not prove physical state.
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
