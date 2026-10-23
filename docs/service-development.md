# Managed inventory service (v2 development)

Common output settings use `output::settings` and concrete `Work::OutputSettings`
with [checked native plans](gear-settings.md). Shared strict MQTT/HTTP and typed
read/configure keep original context/revision/deadline/permit through settlement.
Preflight the whole patch before DTR, including fresh physical/current limits and
both crossed-limit orders. Preserve readable special255/packed rate0 and allow
unrelated partial diagnostics. Only matching requested readback is configured.
Settings are not bridge ConfigStore data or a scalar State cache. Configure cuts
bus-local base/colour/group actual evidence before possible send and at settlement;
reads preserve it and native input Notices/values. Identification guards apply at
submission and owner admission for writes; read and unrelated gear remain usable.
Read-class MQTT dispatch precedes requiring CommandCompletion. Keep all concrete
Work/Run/Sample/Completion/Message and stop branches exhaustive. Both operations
have one8s ceiling, original caller expiry and original mapping rechecks.

Native input event settings use `input::settings` and a separate
`physical::Work::InputSettings` variant. Read/Patch/Readback retains the original
request, optional MQTT completion, permit and eight-second maximum deadline
through settlement. It performs no ConfigStore edit. Resolve the saved instance
and original write generation/revision before admission and at final acceptance;
use `Hat::evidence_current`, not private stamp access or cached hardware alone.
`commissioning::Bus::command` already implements two ordinary 24-bit sends under
one combined 100 ms bound; do not use the ATX 16-bit repeat flag.

Settings reports never enter the input value/Event cache. Type proofs bracket
the yes/no Enabled query; collisions are not NoResponse. Required filter bytes
must be complete, and FF remains data. At write admission and settlement the
affected module's current values and permanently revocable Notices are cut, even
when a final sample is missing/stale. Read work does not cut or synthesize actions.
The strict shared commands and operator workflow are in
[event settings](input-event-settings.md); fault controls are
`v2-managed-input-settings-controls.toml`. Literal service peers complement the
independent physical emulator. Their task must capture the whole PTY so its slave
keeper survives disjoint field capture.

Optional `settings::ButtonPatch` / `ButtonValues` use `settings/button.rs` under
that same work item. Probe all six timer bytes before the closing type proof;
keep timing quality independent of common settings. Preflight the whole mixed
patch against fresh short/double minima before the first common setting write,
then append the checked timer plans. Actual requested timers must match complete
readback before success. FF and double0 retain their native meanings. The
intentional controls are in `v2-managed-button-timing-controls.toml`; the literal
peer records input commands so the no-write preflight assertion observes actual
wire traffic, including a later invalid timer after an earlier valid one.

Optional `settings::SensorPatch` / `SensorValues` use `settings/sensor.rs` within
the same owner, original permit/deadline and permanent Notice cuts. Probe only
the instance family's parameters before the closing type proof. Keep sensor
quality separate from common settings. HoldFF is raw known unsupported, not
null/zero; report/deadtime/minimumFF remain numeric. A mixed sensor/common patch
needs complete valid sensor evidence and supported requested hold before the
first setting write. Common-only patches tolerate partial optional evidence.
Append checked sensor plans and require actual requested values before success.
Do not save settings or put them in current input State. Literal peers assert
wire command counts before unwrapping a refused result, so an unexpected write
cannot hide behind a later script failure. Controls: `v2-sensor-settings-controls.toml`.

Managed basic feedback uses `input::feedback` and `Work::InputFeedback` in the
same physical owner. Module and instance routes preserve original context,
generation/revision, completion, eight-second deadline and permit. Fresh bounded
same-scope feature enumeration, feature32/capability and closing Type/Status proof
precede any activation claim. A silent Active query alone cannot distinguish
inactive from missing; collisions stay unknown. Preflight complete supported
evidence, send one Activate/Stop even for an equal value, then require complete
matching actual readback. Final acceptance re-resolves the original action and
checks current physical stamp/deadline/token. Box fallback reports; mark writes
started without input-settings' module-wide Notice/value cuts. Indicator work
does not replace input observations or save config. Shared MQTT/HTTP strict
read/set commands and frozen browser controls use this path; scalar Desired
properties remain later work. See [operator/API guide](input-feedback.md),
[feature plans](dali-input-contract.md#feature-enumeration-and-basic-feedback-plans)
and `v2-managed-feedback-controls.toml`.

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
means no HAT endpoint was enabled. Version and UTC build time accompany the saved
controller identity. Resource shutdown remains the managed runtime's ownership
responsibility even if a filesystem worker is blocked.

## Optional HAT transport and health

Call `service.with_uart(path, RepeatPolicy::Unverified)` before managed startup,
or `service.with_hat_tcp(socket_address, repeat_policy)` for a raw TCP emulator.
These builders select one endpoint (the last builder wins), perform no IO and
leave the existing physical owner in charge. The CLI rejects simultaneous
`--uart` and `--hat-tcp`. Numeric TCP endpoints require a nonzero port.
`atx::connection::{Endpoint, Connection}` delegates serial/TCP IO without a
second protocol driver; Tokio TcpStream implements the existing DrainRead
contract using an actual nonblocking recv, including distinct EOF handling.
An async TCP connect has a two-second bound and is retryable; a UART open timeout
retains the terminal policy for its possibly outstanding blocking worker.
New TCP connections must own fresh remote parser/output sessions, so their
adoption clears inherited parser-poison flags; UART reopen retains its Boot
requirement. Current-connection receive recovery and all command/evidence checks
remain owned by the existing monitor/transactions path. The emulator TCP listener
is a separate implementation; see the [handoff](handoffs/2026-10-23-emulator-ui.md).

For either endpoint, optionally follow the builder with
`with_uart_quiet_period(Duration::from_millis(250))?`; the builder validates a
1 ms..5 s interval and requires a HAT endpoint to be enabled first. The builders
open nothing. The managed service owns one Connection/Transactions
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
Admission is exclusive and normally nonwaiting; Busy refuses overlapping work.
Discovery and identification use fair admission within their original deadlines.
Discovery validates its original root/generation after acquiring the permit so
background state polling can delay a scan without aborting it or renewing authority.
The actor rechecks phase/revision/token before saving. Input and instance names
use the same MQTT `rename` command as outputs, with their exact qualified target.
Renaming a module rotates descendant mappings together; the original module
token still applies if only a child was renamed earlier. See the
[input inventory contract](inventory-contract.md). No public metadata call
can claim a physical replacement, address assignment or group-member change.
Group membership uses the owned [probe/save/program API](groups.md), with explicit
hardware adoption distinct from reviewed full-mask repair. Address assignment
uses the [commissioning API](commissioning.md).

Scene metadata also uses this owner: `CreateScene`, `RenameScene`, `DescribeScene`
and `RemoveScene` save named slots without UART access. Creation requires the
original controller token; targeted edits require the original scene token, even
when an unrelated edit has advanced the controller generation. Description-only
edits preserve the scene binding. Creation/removal does not program/clear native
fixture contents. MQTT and authenticated HTTP jobs share the same strict parser
and save path; see [scene metadata](dali-scenes-contract.md#named-slot-metadata).

`read_scene` inspects one output's native slot; `recall_scene` sends a saved scene
to broadcast or a numeric group through `physical::Work::Scenes`. They retain
original context, deadline and permit through settlement and final acceptance.
Stored reports never enter actual output State. Every possibly-sent recall,
including a failed/cancelled one without a sample, invalidates bus cached actual
values at admission and again before completing; the request retains its bus
independently of the sample. Admission publication precedes any send, covering
caller timeouts that can return before the physical owner has finished cleanup.
Recall refuses continuing Blink claims on that bus at submission and owner
admission. See [scene operations](dali-scenes-contract.md#stored-reads-and-native-recall).

Private rows use `config::scene_settings::{Row,Setting}` and
`Edit::SceneRow`. Validate Origins after ledger replay, prune retired outputs,
and strip contents centrally in public `Model::state_bytes`. Do not change the
public scene projection when only its desired contents change. The root revision
still advances. `service::scene_settings` and concrete `Work::SceneSettings`
retain a boxed request through Probe/ConfigStore/Program. Original scene authority,
fixture selection, completion and physical stamp survive the save. Set still
programs on a no-op save; adoption writes only configuration; repair never adopts
drift and repeats the review immediately before native programming. Capture
reads actual output without inserting it into the State cache, then reads stored
data under the same permit. See [private rows](dali-scenes-contract.md#private-per-fixture-settings)
for payloads, MASKs, failure outcomes and browser composition.
`read_scene_settings` uses a pure shared snapshot helper before MQTT completion
is required. It checks Ready phase and any supplied scene token, but requires
neither enabled bus nor physical permit. HTTP retains its mandatory generation,
session and CSRF. The complete reply identifies every output, including unmanaged
null rows. Its HTTP budget is 1 MiB; ordinary jobs retain 32 KiB. Public snapshots
still use the central stripped Inventory reader. Job lists/SSE contain summaries;
only explicit per-id lookup carries a full result.

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

`reset_address_space` uses the same controller authority and physical owner.
One atomic removal vector and normal ConfigStore/SDK activation retire all saved
rows in the selected space before any native clear frame. Preserve its original
physical stamp through save, enqueue and final acceptance. Its own save retires
the controller token, so post-save checks use the committed revision/Ready phase
without silently retagging the original request. Empty saved spaces still clear
unsaved hardware. The report distinguishes cleared from retired-but-unconfirmed;
the served UI composes assignment/refresh separately. See
[reassignment](reassignment.md) and [fault controls](v2-reassignment-controls.md).

`restore_identified_address` probes the absent old address and present compatible
candidate, then atomically removes the candidate and readdresses the original
saved output or input subtree. It uses normal ConfigStore/SDK activation with
the original command completion; the new mapping legitimately rotates its token.
There is no native address write after saving. The resulting report retains the
previous address, resulting address/current generation and own committed root.
The old reservation becomes free; original Origins, desired group references and
per-Origin private scenes continue. `restore_unaddressed_address` instead reuses
the old reserved address after proving a sole unaddressed candidate. Its native
plan shares the extracted bounded commissioning helpers. Permanent per-route
Notice revocation prevents queued presses resurfacing across this unchanged-token
shortcut. See [address recovery](address-recovery.md) and its
[fault controls](v2-restoration-controls.md).

`clear_short_address` uses a distinct Clear action/plan in the same physical
owner. It confirms present, sends the existing addressed clear plan, then checks
absence. There is no ConfigStore edit or identity transition. Include Clear in
mutating/profile admission and unconfirmed-report conversion. Keep the original
stamp/authority and permanent Notice cuts at admission/settlement. `cleared`
describes physical address removal; the report's address remains the saved
reservation. The browser performs no automatic follow-up. See
[address recovery](address-recovery.md) and `v2-address-clear-controls.toml`.

Commands use `DALI/Command/<controller>` and exactly one MQTT v5
`target-generation` property. They are momentary WithReply commands; provide a
response topic and correlation data when awaiting the result.

| Command | JSON fields in addition to `command` | Generation |
|---|---|---|
| `blink` / `stop_blink` | Individual output `target`; no other fields | Current output token |
| `configure_bus` | `bus` (0..3), `name`, `enabled` (bool), `expected_revision` | Current controller token |
| `reset_address_space` | `bus` (0..3), `space` (`outputs` or `inputs`), `expected_revision`; no target | Current controller token |
| `remove_from_list` | Output/input-module `target`, `expected_revision` | Current selected item token |
| `rename` | `target`, `name`, `expected_revision` | Current output/group token |
| `set_color_representation` | Output or saved group `target`, `color_representation` (`temperature`, `xy`, `rgbwaf` or null), `expected_revision` | Current selected item token |
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
hardware capability or change persistent operating modes. Saved groups use the
same optional metadata and identity rules; native control/readback is documented
in [group color](group-color.md).

Optional MQTT v5 `traceparent` follows each command through BridgeKit's worker
and the metadata actor. Inventory publication uses the committed generation
with the original trace; the success reply keeps the original request generation,
correlation and cause. The shared runtime validates trace metadata independently
of generation admission. Local OTLP tests verify exported worker/actor operation
spans, rather than only an ingress span.

`__inventory` is read-only and returns the complete observed saved inventory.
Individual PowerState/Brightness writes and reads use the owned physical service.
Selected-family colour properties also use that service with fresh capability
preflight and actual readback. Group base scalar Desired/refresh use the same
native group owner as the explicit
[`read_group_output` / `set_group_output` path](group-output.md), with actual
individual-address samples and no desired membership changes. Group colour and
unsupported properties return explicit errors. A private evidence reply preserves
the accepted Observation/context/stamp for final scalar acceptance; public native
wrappers still return Report. Snapshot group reports share an Arc, retire on root
revision/bus/output mutation and publish explicit nulls for mixed/unknown fields.
The pending Work predicate runs before polling HAT progress, covering typed/web/
Blink mutations too; input-only work and nonactivating scene editing are excluded.
Cache validity reuses individual-output handling of ordinary Writing/Awaiting;
final acceptance still requires Ready and exact original evidence. Group All
reads share the existing fair cursor/15s ceiling. See [MQTT group bindings](mqtt-outputs.md#hardware-groups).
Saved inventory
does not prove physical state.
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
