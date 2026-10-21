# P4b3: owned single-output jobs and observed readback

2026-10-22. T3 continuation of the approved architecture. Baseline 43343ed has
bounded request/helper progress; fd86341 has the pure output codec. This slice
adds actual short-address output jobs through the shared service. It does not
change the v1 binary, saved schema, Store, browser UI or shared BridgeKit API.

## Problem, evidence and shape

The service currently exposes metadata changes only. Physical operations need
one owner shared by MQTT and HTTP, atomic original-generation resolution, real
readback and cancellation settlement. Hat already owns the only transaction
session and one helper; service mutation admission already excludes metadata
changes. Extend these owners rather than exposing Transactions to frontends or
creating another mailbox/UART path.

Rejected shapes: doing I/O in the proxy would couple the UART to broker worker
lifetime; resolving an address before posting to the owner would permit stale
mapping races; letting a helper own the service permit would release it on abort
before its independently running transaction actor stopped transmitting.

Use one owner-held physical request and the existing service admission permit.
Hat schedules its helper ahead of the next health poll, after any existing owned
helper actually settles. Ordinary command steps use the existing Transactions
actor and one absolute operation deadline. Add a crate-private execute_until
path carrying the original Instant, retaining the public Duration wrapper.
Carry that Instant through correlated write/fence internals too, rather than
rebuilding it from remaining durations. Check expiry at admission and before
polling a new transport write. No DTR/device-type state is changed in
this slice, so no new multi-frame DTR contract is needed. No public transaction
clone escapes the service; health and physical helpers cannot run concurrently.

## Admission and lifetime

Add typed ServiceHandle operations to read one named output or apply a validated
gear::Intent then read it. Reads share admission because physical resolution must
remain stable across their observations. Cached snapshot access stays nonwaiting.
Only Phase::Ready is routable. Writes require the exact original target token;
reads may omit it, but an explicitly supplied token must match. Resolve the target,
enabled bus and short address atomically in the owner against its current Model.
Capture that mapping's token, original trace cause and current UART epoch once.
Never fill a delayed result from the latest mapping token.
Before the first fixture frame, require the captured epoch still matches and the
same session is Ready; a request waiting behind health/recovery cannot silently
adopt a newer session. Reject it without fixture I/O if that check fails.

The same capacity-one service permit covers metadata and physical jobs. Keep it
in the owner-held request, outside the abortable helper. Skip canceled/expired
requests before hardware admission. Once started, a request may finish within its
original deadline even if its frontend disappears; its helper remains owned.
No later command step starts after expiry. No queued/expired request writes.

Dropping a Transactions caller does not prove backend retirement: its existing
outer timeout can return while an actor request still holds admission. Add a
crate-private cancellation-settlement operation which actually acquires that
same transaction semaphore, under one supplied absolute ceiling, then releases
it. It is called only after the frontend helper has actually joined, while the
service's sole physical admission is held. Successful acquisition proves all
preceding actor requests have retired; it does not prove session teardown. Check
terminal/stopping status and worker state before and after acquisition. A
terminal/stopping session, closed semaphore or inability to settle requires actual
transport close/join even if its permit is available. Keep the
physical request and service permit through that close result.

Operation expiry prevents admission of new command steps and bounds software
submission waits; bytes already accepted by the kernel/HAT can still have later
physical effects. This is not a hard real-time guarantee about a UART wire.
Cleanup has one fixed 1 s ceiling selected once on entry to settlement/teardown;
it may outlive the caller's
deadline, while admission remains held. It must not renew on retries or stages.
Managed shutdown still uses its existing shared absolute ceiling. If actual
teardown fails, fail the service rather than release routing for new writes.

Boot, EOF, lag and helper failure preserve the current ownership rules. Increment
a private UART observation epoch on events/status that invalidate a result:
session replacement/reopen, Boot, EOF, lag, write poison and loss of transaction
ordering (ResetRequired, unresolved late reply, or unexpected Uninitialized).
Detect status-first invalidation as well as public records. Bus power changes
invalidate the affected bus's observations/start token without declaring healthy
buses unavailable; capture a per-bus epoch alongside the UART epoch. A completed
readback captured before such an event cannot become a fresh observation. Boot
or lag cannot hide a completed panic. EOF starts owned close; the physical permit
survives helper cancellation until actual teardown. Retained handles own data,
not descriptors. A canceled containing changed future loses no helper/result.

## Output operations and observation truth

Apply validates the complete Intent before queuing. It sends its single standard
frame (OFF, recall maximum, or DAPC), then individually queries actual level and
status at the same short address, with the original operation deadline throughout.
Read performs those individual queries only. A write's N terminal is normal
transport completion, not fixture confirmation. Preserve collision/transport
errors; no retries that might duplicate an operation are added here.

Level/status queries are sequential samples, not an atomic fixture snapshot.
Return/cache a typed observation carrying its captured target context, actual
level/status and availability/quality. Actual 0 means known off. Actual 1..254
means known on and normalized code/254. Actual 255 means answered-but-level-
unavailable, never maximum. No response and collision mean unavailable/unknown,
never off or an invented capability. Status remains raw plus the existing typed
flags; fade-running does not mean the final requested level was reached. If a
later query fails, retain partial evidence with degraded quality and the sample's
captured context; do not present the pair as one current confirmed state. An
invalidated UART/bus epoch makes the whole result stale. Preserve failure
evidence without echoing intent.

An applied result requires actual post-write fixture readback; failure after
submission reports uncertainty rather than claiming the request was never sent.
Healthy missing-fixture responses are target failures, not a dead controller.
Saved inventory is retained for a missing fixture. Hardware outage/reset makes
cached output observations unavailable; it never manufactures off or renames an
item. Cache updates are accepted only for the original mapping and UART epoch.
Metadata activation retains only observations whose exact target token survives.

This is a service-library execution boundary. Proxy physical writes/refresh stay
explicitly Unsupported until the following MQTT integration slice, which must
solve the observed original-request trace gap in read_state_with_context(scope)
and pin its wire behavior with real FakeBroker tests. Group execution/readback is
also deferred: stored membership alone cannot prove physical group membership.
No group query is used as collision-free aggregate evidence. These are sequencing
boundaries within the already approved comprehensive scope, not removals.

## Files and proof

Likely files: service.rs/owner.rs/routing.rs; new service/output.rs with its tests;
hat.rs plus owned-job/teardown tests; minimal Transactions settlement method and
engine tests. Add typed snapshot observation storage where necessary. Existing
metadata, generation, controller identity and health tests remain unchanged in
intent. Emulator remains an independent implementation.

- Deterministically hold a real actor's queued/fence/write path, cancel its actual
  frontend, and require settlement to remain pending until actor admission is
  actually retired. Prove a subsequent ordinary command still works. Remove the
  settlement wait as an isolated behavioral negative control.
- An expired original Instant never becomes a new duration-based window after
  preemption. A terminal actor with a free request permit still requires actual
  close/join, including failure cleanup. Use the real ownership path.
- With the independent PTY emulator, read a saved output's real initial level,
  apply power/normalized brightness, inspect emulator physical state and verify
  returned actual readback, including 100% => 254, off and recall maximum.
- Invalid/contradictory intents and wrong/retired generations make zero writes.
  Resolve again after enqueue; metadata renames cannot pass held physical
  admission. Delayed old results cannot acquire a new token or overwrite a
  replacement, Boot or reset observation.
- A queued request whose epoch changes behind an existing helper is refused
  before its first fixture frame. Status-first ordering loss, write poison and
  one-bus power loss cannot produce a fresh cached observation.
- Cancel before start, during response/fence and after submission. The service
  permit stays held until actual settlement/close; queued canceled work cannot
  transmit. Retained clients cannot preserve a descriptor after close.
- Missing fixture, raw 255, collision, partial write, EOF, Boot and helper panic
  preserve truthful state/uncertainty and actual teardown. Missing one fixture
  must not prevent a later healthy fixture operation.
- Broker refusal does not prevent direct service reads/writes; shutdown remains
  bounded with active/held work and includes actual owned task destruction.

Run behavioral RED before bug fixes where existing APIs permit it; new APIs may
initially fail to compile and that is not a behavioral RED. Every safety mechanism
gets an intended guard-removal control after implementation. Run targeted then
full native tests, strict scoped Clippy, Pi checks, formatting/wait registry and
the affected controls, followed by restored checks. Two sequential read-only
design/source reviewers cover this T3 boundary; only same-family models are
available. Record measured results and remaining physical validation in the
contributor guide, support matrix and phase ledger. No live hardware/broker/Store
is used.

## Parent checks before review

Read actual service admission/activation, Hat helper/event/close ownership,
Transactions enqueue/actor retirement and the independent emulator control API.
The normal oneshot response retires its actor permit before waking the caller;
timeout/cancellation does not supply that receipt. Existing helper Drop aborts
its task, hence the physical service permit cannot live there. Request traces
and observation generations are independent; preserve both explicitly.

### Lossless invalidation evidence

SessionState is a watch value, not a transition log: Late may return to Ready
before Hat polls. Add an actor-owned monotonic invalidation counter shared with
Transactions. Bump it for reset/ordering loss, entry to unresolved Late, trusted
Boot and actor lifecycle termination, before publishing the corresponding result
or status. Hat samples this durable counter at request admission, before its first
fixture frame and when accepting completion. The UART stamp combines descriptor
generation, actor invalidation counter and the affected bus's epoch; it never
depends on seeing every watch notification. Counter exhaustion invalidates
admission rather than reusing a stamp. Public loss/poison evidence still
invalidates immediately; the counter complements it.

Withhold Hat polling across an actual timeout and late terminal until the actor
is Ready again. Require the prior observation and queued start stamp to be stale
despite that current Ready status. Removing the counter increment must make this
test fail for that reason. Likewise inspect actor death with a freed request
permit: retirement alone must not be reported as completed transport teardown.

Both design reviews required these concrete ownership/deadline/epoch amendments.
The parent swept the revised rules across enqueue, receive fences, correlated
writes, actor mode changes, HAT startup/completion and cache acceptance. No new
user-facing policy or architecture decision is introduced.

The full group/color/input/settings extensions remain subsequent P4 work. This
slice deliberately changes no public MQTT capability declaration before it has
its own tested wire path.

## Source review and reproduced corrections

Both sequential read-only source reviews approved the final implementation.
The first review found two timing defects. A free semaphore could let settlement
succeed after its absolute cleanup ceiling; explicit clock checks now surround
acquisition. Independent cache/reply validation could leave a stale cached
observation paired with a successful current reply; both now use the same final
validated sample. Each regression first failed for the reported behavior, then
passed with the fix. The acceptance test injects production bus-state events at
the disputed boundary through a test-only hook.

The expired absolute-request fixture now initializes the actual transaction
actor before its deadline expires, so its no-write assertion exercises a usable
transport. Both reviewers remained read-only; the parent owns test and control
runs. Reviews use the same model family, not independent model families.
