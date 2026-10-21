# P3b BridgeKit managed process service

Implements the approved small shared process-service extension. Tier T3: shared
startup, supervision and shutdown behavior. Keep existing Bridge/Host behavior
and MQTT grammar unchanged. This slice does not yet add DALI inventory or request
generation metadata; those follow before any mutable target mapping is exposed.

## Current-code findings and choice

`Bridge::run` owns signals-first startup, tracing-init, driver description,
watchdog supervision and bounded shutdown (`process.rs:308–448`). `Bridge::start`
omits logging/signals/panic policy by design. Wrapping start in a DALI-owned main
would duplicate that policy. A driver connect helper would couple the website
and shared UART owner to the link lifetime. Use a managed service inside the
existing process policy, independent of MQTT/HAT connectivity.

The fixed bridge CLI already requires `--instance`. DALI startup validates it
against the persisted controller name rather than changing identity implicitly.
Migration uses the existing config name; no service API for replacing the bridge
instance at runtime is needed. Service construction must be resource-free; Rust
constructs the argument before run_managed is called. CLI usage handling precedes
service execution, and all filesystem/listener/UART work belongs inside run.

## Public shape

Add `ProcessService: Send + 'static`, with associated `Driver: DeviceDriver` and
one async `run(self, ServiceContext<Self::Driver>) -> Result<(), String>`. A
service owns its complete lifetime. It prepares configuration/shared state and
publishes exactly one ready driver proxy using `ctx.ready(driver)`. It then runs
until `ctx.cancelled()` and closes its resources. Its driver proxy may use bounded
cross-thread channels into this service; driver link retries must not destroy it.
The context's ready call evaluates pure driver.info under the startup bound and
validates its timing against the static bridge settings. It synchronously hands
off `(driver, validated DriverInfo)` once and returns Result without waiting for
driver connect, MQTT or an initial read. Duplicate/canceled/expired readiness is
an error; latched stop/death wins the shared transition. Pass that info into
Host::launch without another parent-thread info call. Validate service timings
and available static bridge settings before spawning the service.

`Bridge::run_managed(service)` uses the same process policy as run. An async
`Bridge::start_managed(service)` provides the same no-signals/no-logging test/embed
shape as start and returns the existing Running, including service supervision.
Existing run/start/Host paths are unchanged when no service is installed.
Use a small explicit ServiceTiming with finite nonzero startup/stop bounds (10 s
and 2 s defaults, configurable in the Rust test/embed API). No new CLI flags are
needed for DALI in this slice. The shared 5 s process shutdown ceiling wins.

## Thread and failure ownership

The managed service runs on one dedicated thread with a current-thread Tokio
runtime/LocalSet, matching the device worker isolation pattern. Its futures need
not be Send; the service and returned driver are Send. Reuse the runtime's OpClock,
heartbeat tick, blocked/idle watchdog constants and process-pause grace logic;
factor small shared helpers instead of copying another watchdog policy. This
keeps a synchronous service block from preventing signals, MQTT or the watchdog.
Blocking filesystem work remains on bounded detached workers; DALI already has
the P0 worker boundary and must not retry exhausted startup workers indefinitely.

Stamp the initial heartbeat synchronously before polling service.run, and stamp
again before ready clears the startup deadline. A spawned heartbeat task alone
can be starved by ready followed immediately by a synchronous block.

The parent races readiness, worker termination, startup deadline and stop. Never
join the OS worker unbounded. Readiness/termination are persistent, not lossy
broadcasts. Returning or panicking before readiness fails startup; a premature
Ok return after readiness is a component death too. A stopped/blocked service
cannot advertise Active or serve hardware accidentally. Any unexpected service
death or persistent blocked heartbeat takes the existing process failure path,
bounded shutdown and restart; the MQTT device worker watchdog remains intact.

Dropping a canceled startup future or Running requests forced cancellation and
releases all async-owned resources; a blocked OS thread can only be abandoned.
After ready, a watchdog runs even if an embedder is not polling Running::ended.
It stores its fatal result until observed and independently closes registry
admission/requests cancellation through existing controls. An unpolled embedder
cannot keep serving through a dead service. Install supervision before launching
MQTT/device workers; already-latched death prevents launch. A later death takes
the normal failure path. No dependency on broker connectivity.

Completion is distinct from cancellation requested. Publish persistent completion
only after the service future, LocalSet children and owned runtime resources have
been dropped. A destructor may block too, so only the parent can enforce the
final completion deadline. Retained contexts/proxy handles cannot own live I/O
outside this lifetime. Startup guards also cover timing/description/launch errors
and a dropped start_managed future, including late readiness after timeout.

## Startup and shutdown sequencing

Generalize the private process launch callback to an async future. Keep existing
signals-first logging/description flow and race the launch future against stop.
Old synchronous launches wrap their existing action in a ready future. Install
the service worker guard before awaiting readiness so cancellation owns cleanup.
No second global logger, signal handler, root Tokio runtime or panic hook.

Stop closes BridgeKit admission, requests service cancellation, and drives device
disconnect/MQTT flush plus service teardown under the shared shutdown ceiling.
The service stops HTTP admission and its own jobs, restores identification where
possible, then closes UART. Driver disconnect must tolerate an already closing
service. If soft cancellation does not finish under its bound, force-drop the
service future on its worker runtime; if that thread is blocked, abandon it and
return a failed bounded stop. Use one absolute 5 s shutdown deadline: soft-stop,
forced teardown, device disconnect and MQTT flush share it, with no fresh
sequential budgets. Forced cancellation counts as success only when teardown is
confirmed before this ceiling. Abandonment returns false and changes the managed
SIGTERM path to a failure exit; unmanaged exit semantics remain unchanged.
Root-runtime/logging cleanup remains outside the existing component ceiling.
Final-handle drop also requests forced cancellation.
Never make the service wait for MQTT shutdown while MQTT disconnect waits for it.

## Files, verification and documentation

Use an isolated tracing-init worktree. Add a small service module and tests;
change process.rs, minimal watchdog/heartbeat factoring in process/link/worker,
exports and Running's optional service slot. Update wait registry, runtime spec,
README, tutorial and developing-mqtt-bridges guidance together. Pin the consuming
bridge to the resulting shared revision before its daemon integration is final.

Failing-test-first cases: existing run/start behavior; service ready plus pure
driver description; service active while broker refused/down; driver reconnect
does not cancel it; failure/panic before/after ready; no-ready async timeout;
actual blocked service thread detected by an independent watcher; stop/cancel
during startup and ongoing operations; ignored soft cancellation; retained Running
handles after stop release resources; no new service work once stop wins; process
SIGTERM while service startup/logging/output is held. Existing conformance gates
must remain green. Negative controls prove readiness/stop/watchdog guards matter.

Review the API/failure invariants before shared edits and the final core diff
after. Different-family review remains unavailable; record same-family reviews
honestly. No deployment or dependency publication is part of this slice.

Implementation review refinements: serialize completion/fatal facts and worker
admission under the managed lifecycle gate, establishing fatal cleanup controls
before releasing that gate. Startup races readiness with persistent worker death,
including a context retained after worker exit. Every teardown branch uses the
earliest latched shutdown deadline, including a later explicit stop after fatal.
Runtime destruction on the isolated service thread confirms blocking helpers
have ended; a timeout that abandons helpers cannot publish successful completion.
Watch borrows are released before taking the lifecycle gate. The gate never
awaits or invokes driver methods; host launch uses already validated info.

## Verification record (2026-10-21)

The full kit suite passed 160 tests, including 16 service cases, the absolute
deadline helper case, tutorial examples and the wait registry. All-target Clippy
with warnings denied and the armv7 Linux musl library compile check passed.
The full conformance suite passed 174 tests, including four new process cases:
SIGTERM during no-ready startup confirms listener destruction; blocked teardown
exits unsuccessfully; broker refusal/full output does not stop the service; and
error/panic/blocked service deaths with full output exit unsuccessfully.

The shell's inherited RUST_LOG=warn hid INFO shutdown records in two existing
conformance assertions; the full conformance run used explicit RUST_LOG=info.
No production logger/filter policy was changed to make those assertions pass.

Six negative controls and their ledger are under mqtt-bridge-kit/docs. Each
failed its declared regression: abandoned blocking helper, lost fatal admission
check, renewed shutdown budget, ignored worker death during startup, disabled
service watchdog and polling ready work after deadline. The startup mutation
initially failed type inference; retaining its typed select branch as pending
produced the intended assertion failure. No compile failure counts as evidence.

Two sequential design reviews approved the contract with refinements. A fresh
core review required runtime destruction before completion, admission/failure
serialization, a shared earlier deadline and persistent death during startup;
these were reproduced, fixed and verified. A further absolute-deadline edge and
actual Running::stop regression were fixed and reviewed with approval. Reviews
were source-only and from one model family. No physical hardware, deployment,
publication, push or merge was performed.
