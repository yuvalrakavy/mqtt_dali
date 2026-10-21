# P4b4: base output properties through BridgeKit

2026-10-22. T3 continuation of the approved architecture. Baseline e88f03c
includes owned single-output jobs and SDK 153d380's optional original read-context
hook. This slice connects standard individual outputs to the existing v2 MQTT
runtime. It changes no v1 behavior, saved schema, Store, addressing or group jobs.

## Verified shape and alternatives

DaliProxy currently refuses physical properties. ServiceHandle already accepts
typed intents, original context and an absolute deadline. The service owns UART
jobs and immutable snapshots independently of MQTT. BridgeKit owns the worker,
mailbox, expiry/conflict policy, reports, replies and publication. Reuse these
boundaries; no second UART path, mailbox or MQTT session is needed.

All physical Reporter calls run on BridgeKit's existing single-threaded worker:
the driver and a LinkContext-owned snapshot helper. The managed service keeps
its existing metadata CAS/inventory publication, but does not publish physical
state directly from its other thread. Otherwise a paused frontend could return
old Current observations after the owner had published a newer offline update.
No new shared SDK state-version API is necessary for this slice.

The helper treats notifications only as wakeups and always reads the latest owned
snapshot. It must not replay buffered Snapshot payloads. Snapshot validation,
projection and Reporter calls have no await between them. The driver's final
read/apply validation and returning/folding/reporting its state likewise have no
await. These paths share one worker thread, so a helper cannot publish an offline
state and then be overwritten by an older suspended read: the read must first
check the latest snapshot after its final await. A later source invalidation is
published by the helper after the synchronous fold. Source snapshots are updated
before their notifications/replies. Mapping changes remain protected by the
existing runtime generation CAS.

Rejected shapes: I/O in a subscription callback would add an unowned UART path;
publishing physical state on both the service thread and worker would create
competing publication order; returning an old notification payload would regress
newer readback; treating HAT absence as a whole Link error would disable metadata
configuration. A per-target device-driver process would duplicate the controller.

## Wire contract

Use the existing target codec and scalar Desired lanes:
`DALI/Desired/Controller/Bus1.Outputs.Light1/PowerState` and `/Brightness`.
PowerState is a strict JSON bool; Brightness is a finite JSON number in 0..1.
Validate the complete PropertyMap with gear::Intent before I/O. Reject unknown
properties, null, wrong types, empty maps and contradictory pairs atomically.
Power true alone recalls maximum; off and normalized DAPC preserve the codec
contract. No cached HA restore policy is added.

Observed State fields are PowerState, Brightness, Available and Quality. Quality
is `current`, `partial`, `unavailable`, `stale`, or `unobserved` before any sample.
Available means a current usable power/brightness pair, not certification that a
fixture is fault-free. It is false for partial/MASK evidence even if the fixture
answered. A Current observation includes actual normalized brightness and power.
Unknown/stale updates carry Available=false and their quality, never an invented
off or maximum. BridgeKit merges properties, so previous scalar values may remain
as last-known data while Available=false; consumers must gate current state by
Available and the controller's Active/Status. Fresh direct read results omit the
unknown scalars, so outage-conflict decisions cannot use them as current values.

Every physical State uses the observation's captured mapping token. Trace context
is independent: an actual completion carries its original cause; an unsolicited
invalidation carries authoritative absence. No DisplayName or identity aliases
are introduced. Description remains in authoritative Inventory metadata.
Groups and unsupported properties stay explicitly Unsupported. Existing metadata
commands, __inventory read-only behavior and retained inventory schema stay intact.

## Driver operations and bounded polling

Use acknowledged writes and observed feedback. apply_with_context parses the
whole map and calls apply_output with its exact original token/cause and one 2 s
deadline. Before reporting success, re-read the latest service snapshot and check
the returned observation still matches its current target/token, raw evidence,
UART stamp and usable lifecycle. Publish actual readback synchronously before
returning Ok, even if the values did not change. This increments Reporter's real
per-property observation counters so runtime inferred write basis cannot replace
clamped/observed values. No success is derived solely from N or intent echo.

Target refresh uses the new read_state_with_request_context hook and the same
2 s service read budget. A supplied generation must match; omission is allowed.
Legacy read_state_with_context uses explicit trace absence. Revalidate after the
last await before returning physical ContextualState. A failed/invalidated result
is a target rejection/uncertain outcome, not a whole-controller Link failure.

Scope::Any reads metadata only for cheap service proof. Scope::All reads metadata
plus saved individual outputs sequentially, with one 15 s total ceiling and at
most 2 s per fixture. Default runtime operation is 20 s and polling is every 30 s;
all remain bounded by the runtime's normal overridable timing. Preserve one
round-robin cursor so a large inventory or exhausted total budget cannot starve
the final bus/addresses. Advance and persist the cursor before awaiting each
selected fixture; a shorter runtime timeout may cancel that very first await.
Do not reset the cursor on reconnect. Return a clear incomplete-read rejection if any target
failed or the total budget prevented full coverage; already obtained observations
remain available through the helper. Do not report an unvisited target as off or
delete it. A healthy no-response pair is a completed unavailable sample and must
not prevent the next healthy fixture from being queried.

Root reads validate an explicitly supplied instance token separately. Per-output
read contexts come from the initial resolved target table with the root's trace
cause, never by applying an instance token to a fixture. At the end of an All
read, validate every collected observation against one fresh latest snapshot,
with no later await before returning. A metadata/mapping change makes the old
collection incomplete; it cannot acquire fresh tokens. Reads remain sequential
samples rather than an atomic snapshot of every physical light.

The runtime may prove the managed service available while the UART is unavailable;
metadata configuration must remain accessible. Hardware availability is carried
by output state. Busy and missing fixtures are target outcomes; only an actually
stopped service is a Link failure. Actual UART cleanup remains owned by the
service, including when the driver future is dropped by the runtime.

An All refresh with enabled outputs and no ready UART must reject before queuing
physical work or taking shared admission. This preserves resource-free metadata
editing during automatic polling. Skip saved outputs on disabled buses; if there
are no enabled outputs, return Inventory and unavailable projections without I/O.
Both admission cases have deterministic no-UART regressions. The first reproduced
an unwanted queued physical request before its early readiness check was added.

## Snapshot publication helper

Attach the existing Reporter, clone ServiceHandle's private watch receiver and
start one LinkContext::spawn helper. It owns only snapshot access and report data,
never a UART descriptor or service admission. Each turn clones the latest value
with borrow_and_update before synchronous projection, then awaits changed. Watch
coalescing preserves a later update without replaying buffered Snapshot payloads
or needing a new subscription API. Report its latest projections directly;
BridgeKit alone owns coalescing and the publication cache. A helper-local skip
map is incorrect when the driver also reports: A -> driver B -> source A could
be skipped as unchanged by that map, leaving B on MQTT. Repeating an unchanged
projection preserves its actual sample cause; unrelated health does not restamp
that sample with trace absence. Runtime CAS already retires old mappings' State.
New mappings start unobserved.

Projection keeps valid values during an ordinary metadata save; uncertain routing
or service stopping/closed makes them unavailable. Bus/UART invalidations already
mark observations Stale. The helper publishes actual observed context for a new
sample and explicit trace absence for invalidation/unobserved state. It may end
after a terminal service snapshot; its normal end means link loss. The runtime
owns cancellation/destruction of helpers on disconnect/stop. Existing Active=false
and Status semantics remain authoritative at process shutdown.

## Files, proof and completion

Likely files: proxy.rs plus new proxy test module; narrow service/owner constructor
fields for bounded cursor/test gates if needed; pure observation-to-wire helpers;
service/daemon process tests, support matrix, MQTT guide and wait registry.
Do not change the independent emulator to share production codec/model behavior.

Meaningful regressions and controls:

- Real managed bridge + ephemeral FakeBroker + independent PTY emulator:
  initial read, power on/off, 100% => 254, actual clamped values, valid original
  State trace and authoritative absence. Inspect physical emulator state.
- Invalid types, unknown/contradictory properties and retired/missing tokens make
  no intended physical change; rename keeps original mapping protection.
- Missing one fixture reports unavailable without off, retains inventory and
  permits a healthy fixture operation. Bus power loss updates availability without
  another refresh; restoration requires fresh samples. Broker outage does not
  prevent direct service operations; reconnect republishes current context/state.
- Hold an actual driver read after service completion, let a bus invalidation
  become visible on MQTT through the worker helper, then release the read. It must
  reject stale completion and never overwrite offline State with Current. Removing
  the final snapshot check must fail this behavioral oracle.
- Coalesce old watch notifications behind a newer sample and ensure the helper
  publishes the latest complete snapshot. Reusing its initial snapshot must fail
  the latest-state oracle rather than silently maintaining stale availability.
- All-read aggregate expiry keeps the original ceiling, visits remaining targets
  on a later pass, validates earlier collected samples after its final await and
  treats a missing healthy responder as local failure. Use controlled PTY/work
  fixtures rather than sleeps as ordering proof. Cancel during the first fixture
  await and require the next pass/reconnect to retain the advanced cursor.
- Apply must report actual values before the worker records write basis; exercise
  readback differing from the request and unchanged readback after clamping.
- Managed stop/disconnect cancels the snapshot helper and actually releases the
  UART owner; retained frontend handles do not keep resources alive.

Use failing-test-first where behavior can be reproduced, then guard-removal
controls for final acceptance, whole-map validation, helper resnapshotting,
actual readback, generation/trace preservation and aggregate deadline/fairness.
Run targeted then full native tests, strict scoped Clippy, Pi library/binaries,
format/wait checks and affected controls followed by restored verification.
Two sequential read-only design/source reviews; same-family limitation remains.
Record the concrete wire contract and proof in contributor/operator docs. No
physical hardware validation, Store migration, web implementation or publication
is implied by this local MQTT slice.

## Parent self-review before gate

Read the current proxy/service APIs and constructor sites, Hat hardware snapshot
semantics, SDK contextual read dispatch and synchronous fold, LinkContext::spawn's
single-thread ownership, Reporter merge/seen behavior, and the existing AC policy
of retaining last-known scalar values under unavailable state. The new read hook
is adopted and the full DALI suite passes at its exact pin. Rechecking physical
evidence only in the service owner is insufficient for a delayed frontend; the
final worker check and same-thread publication ordering address that boundary.
An aggregate read must validate earlier samples after its final await and must
not continually restart from the first target when its budget is exhausted.

Both sequential source-grounded design reviews approved. The first required
cursor advancement before a cancellable fixture await and retention across
reconnects; this is now explicit above. The second highlighted that library
Observation::available accepts answered MASK evidence, whereas this wire
Available contract requires Current usable scalar values. Keep both meanings
distinct and pin the latter with a MASK/status projection regression.

The first implementation's helper-local projection cache reproduced a concrete
ordering bug: pause the helper after A, report B through an actual MQTT refresh,
change the independent fixture back to A through the service, then resume the
helper. The cache skipped A and left B on MQTT. A behavioral RED established this
before the fix. Remove that cache and let BridgeKit coalesce the latest reports;
the watch/single-worker publication shape and final validation remain unchanged.
