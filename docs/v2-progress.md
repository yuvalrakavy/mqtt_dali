# v2 implementation progress

Development checkout: `Home/mqtt_dali-v2`, branch `feat/dali-v2`.
Base: `db6f39f63da75a0614eb8f1a3b274a163bc4b307`.
The approved architecture and phase acceptance criteria are in
`superpowers/specs/2026-10-21-dali-v2-design.md`.

## P6c — actual scalar values and live input/job activity

Extend HTTP jobs with individual apply_output/read_output/read_input, reusing the
same atomic scalar intent parser, 2/4/6/8-second budgets, typed physical ownership
and latest-snapshot acceptance as MQTT. Shared pure input acceptance preserves
newer native values over an already-ready query. Complete snapshot states carry
actual values/quality and original generations, with unknown/stale clearing.
The HTTP budget grows to 4 MiB; MQTT inventory remains 512 KiB. A dense valid
model measures 522,331 inventory bytes and 1,054,621 HTTP bytes for 1,824 initially
unobserved scalar states, demonstrating the real 1 MiB boundary.

SSE adds complete jobs, native input actions with exact Notice::current receive
fencing, and explicit loss followed by a current snapshot. One pending notice/
loss plus dirty flags and a three-kind cursor prevent snapshot/job/input traffic
starving one another. Recheck authorization before each emission; no passive
renewal, action replay, detached producer, SDK/MQTT schema or UART policy change.

Both sequential design reviews approve (about 50 and 47 seconds, same-family,
source-only); the first requested the simultaneous-ready fairness regression.
Initial HTTP regressions fail before implementation on missing observations and
native SSE. The native test's nested equal timeouts initially reported its inner
wait rather than outer named criterion; shorten the outer test oracle. Thirty-
nine focused tests pass before the loss-conversion consolidation. These include
all colour families, real actual/quantized values, exact 255-bit inputs, physical
preflight, reads resumed after rename/newer native evidence, ready-source fairness,
lag, receive cutover, busy-stream expiry and real running/completed job updates.
Two new test assumptions were corrected against existing producers: compare
device state without the advancing emulator clock, and xy raw/65536 units.

The first 17-control run produces 16 intended REDs and one GREEN: an await-side
loss conversion is masked by the ordinary snapshot notification before native
prefetch. Remove the duplicated conversion in favor of one InputEvent::receive
used by both paths, and replace the two conversion controls with one shared-rule
control. The affected stream tests pass; the new shared control is RED (10 s).
All **16 current observation controls** now have intended behavioral RED evidence.
The initial GREEN remains in its historical log/ledger; it is not counted as a
successful control. One reanchored metadata control is RED (14 s), and the two
updated baseline SSE revocation/expiry oracles remain RED (30 s).
Logs: `/tmp/dali-web-observation-controls.log`,
`/tmp/dali-web-observation-shared-loss-control.log`,
`/tmp/dali-web-observation-reanchored-control.log`,
`/tmp/dali-web-observation-sse-controls.log`. Restored acceptance passes
**516/516 native tests** across 11 binaries (four processes; execution 52 s,
wall 98 s), scoped strict Clippy (9 s), Pi library/all four binaries (4 s),
doctests (none), changed-file format, affected control plans, whitespace and
wait validation (zero findings). The registry initially needed explicit
"on expiry end the stream" wording; its generated waiter now names event_stream.
Logs: `/tmp/dali-web-observation-acceptance`.

Frozen `f4501ec` is approved by both sequential source gates against `fe7c30d`,
with no material findings (67 seconds each). Both reviews were read-only,
source/test-only and from the same model family; they reran no checks.
Browser workflows and physical HAT validation
remain pending. See [web access](web-access.md) and
[slice design](superpowers/specs/2026-10-22-dali-v2-p6c-web-observations.md).

## P6b — authenticated configuration and commissioning jobs

Add strict POST `/api/jobs` and protected list/result routes. Preparation reuses
the MQTT command parser and shared concrete resolvers, preserving the client's
generation/revision and original 2/10/18-second deadline. Execution calls typed
service paths without an SDK command completion or publication bypass. Explicit
HTTP trace metadata, including absence, crosses the queue. Reauthorize after
JSON upload before preparation/admission; polling never renews a session.

Four bounded command futures live inside the existing Server task; no detached
job task, retry, UART journal, SDK change or new physical owner. Keep 32 admission-
ordered process-local records, evicting completed entries only, with safe codes,
sanitized free-text reasons and a 32 KiB whole-result limit. HTTP disconnect/logout
does not cancel admitted work. Service stop drops its receivers under the existing
shared deadline, preserving interrupted-save uncertainty. Existing structured
partial outcomes remain visible; 202 and a succeeded report do not promise full
physical success. Output apply/read, observations/events and Preact screens follow.

Both sequential design reviews approve (77 and about 40 seconds, same-family,
source-only). The first review's explicit trace-extension detail is incorporated.
The first HTTP metadata regression fails with missing route before production;
the companion absent-route upload setup timeout is not counted as auth evidence.
Initial compilation needed an owned String result annotation; one emulator test
corrected its expected existing group outcome from programmed to verified.
Twenty-eight focused web/facade tests pass; the final suite also exercises a
command with a broker endpoint that never sends CONNACK. Real socket/emulator
tests exercise metadata persistence/shared
MQTT publication, stale authority/revision, strict parameters, slow-upload logout,
input/output assignment, real-instance discovery, physical groups, safe save
failure/uncertainty/recovery, trace continuity, bounded records/futures and stop
while an incomplete HTTP upload drains. No >5-second physical timing measurement
is claimed; original-deadline and disconnected-request ownership are tested
independently using the current emulator's available timing seams.

All **13 new fault controls** detect their declared behavioral failures (2m36s),
then restore source. The affected control-plan sweep found one preexisting stale
unknown-property anchor after colour parsing moved; re-anchor its actual guard
and its repaired row is RED (12 s). Other existing rows pass plan checks.
Restored acceptance passes **505/505 native tests** across 11 binaries (four
processes; 92 s wall clock), scoped strict Clippy (10 s), Pi library/all four
binaries (3 s), doctests (none), changed-file formatting, affected control plans,
whitespace and wait validation (zero findings). Wait lint initially identified
the test observer's inline lock and the prepared-operation await; isolate the
test-only observer and explicitly register the existing request deadline.
No production timing/policy changed. Logs: `/tmp/dali-web-job-controls.log`,
`/tmp/dali-web-job-reanchored-control.log`, `/tmp/dali-web-job-acceptance`.
Both sequential source gates approve frozen **d2f6828** against **df3e924**,
with no material findings (82 seconds and 2m40s). Reviews were read-only,
same-family/source-only and ran no verification or mutations. Browser and
physical hardware validation remain outstanding. See [web access](web-access.md) and
[slice design](superpowers/specs/2026-10-22-dali-v2-p6b-web-commands.md).

## P6a — authenticated HTTP foundation

Implement the independent API/lifecycle foundation while remaining P4/P5 actions
stay open. Add optional Axum server owned by DaliService, explicit startup
password/origin, fixed-size verifier and cookie sessions, Origin/CSRF gates,
protected public-inventory/status snapshots and pull-based SSE. Idle expiry is
30 minutes, absolute 12 hours; explicit user activity alone renews inactivity.
No URL/Bearer auth, persisted sessions, default password, SDK or recovery change.
P7 still supplies the approved HTML/Preact screens and typed mutation/job routes
follow; `/login` currently returns minimum identity JSON, not a finished UI.

Design gate 1 found raw credential retention in the SDK's Debug Settings; consume
the option and strip separate/inline password arguments before Bridge composition.
Both sequential gates approve the amended design. Cached Axum source showed
outer-task abort does not own its spawned connections: accepted IO now carries
forced cancellation and a permit, with actual-drop accounting at close. Share
the existing startup and absolute cleanup ceilings; no second daemon/framework.

The first process regression fails at its missing-password preflight assertion
before production. Initial compile-only module-path failure is not behavioral
evidence. Early socket tests catch URI parsing accepting a nonnumeric port;
explicit authority-suffix validation fixes it. Fifteen focused tests pass before
the added standalone password-verifier control test: direct routes, origin/CSRF,
cookie-only/secure flags, rate/cap/rotation, idle/absolute expiry, live logout,
latest inventory, occupied/expired bind, broker-without-CONNACK usability,
actual managed stop, incomplete-upload forced cleanup and canonical CLI/environment
credential privacy. New dependencies preserve existing futures/tokio-util lock
versions rather than upgrading their graph incidentally.

All **19 behavioral controls** fail at their declared assertions after restoration.
The first stream-cap control was WRONG-REASON: the test helper waited for an
incorrectly accepted infinite response body before asserting status. A captured
diagnostic confirms that timeout. Assert response headers first; the corrected
control is RED. Five additional password/privacy/cookie/upload controls are RED.
Logs: `/tmp/dali-web-controls.log`,
`/tmp/dali-web-stream-control-diagnostic.log`, `/tmp/dali-web-additional-controls.log`.

Restored acceptance passes **492/492 native tests** across 11 binaries (four
processes), scoped strict Clippy, Pi library/all four binaries, doctests (none),
changed-file format, whitespace, control plan and wait-registry validation (zero
findings). Logs: `/tmp/dali-web-acceptance`. The wait helper was initially invoked
from the bridge checkout and inherited its Pi target; rerunning from its native
SDK checkout succeeds. No SDK source or production runtime change resulted.
Both sequential read-only source gates approve frozen **70afa696** against
**d46cb812**, with no material findings (76 and about 70 seconds). Reviews were
same-family/source-only and ran no verification or mutations. Browser and
physical hardware acceptance remain outstanding.
An isolated headless Chrome probe opens about:blank at phone
viewport, confirming local Playwright capability; no feature/browser pass claimed.
See [operator/API guide](web-access.md), [design](superpowers/specs/2026-10-22-dali-v2-p6a-web-foundation.md)
and `v2-web-controls.toml`.

## P5f — one unaddressed input-module assignment

Extend the existing owned commissioning flow with explicit output/input address
space, pure Part 103 frames and an independent input addressing emulator.
`assign_next_input` uses strict controller authority, raw short addresses,
two ordinary 24-bit exchanges under one fixed 100 ms pair deadline and a 200 ms
randomisation allowance. The 16-bit repeat profile remains output-only. Preserve
the original stamp, permit and command completion through search/save/program.
Save a fresh empty InputN module before PROGRAM, then verify/query the raw address
and require fresh status. Separate addressed discovery fills real instance types
under the same module identity. No SDK, quiet recovery or UART journal change.

Both sequential design gates approve, with no material findings. The first MQTT
test failed with unknown command before production; the independent emulator
literal test failed before model wiring. Fifty focused tests pass, covering all
six random-address boundaries, raw short addresses 0/63, range versus exact
collision/invalid replies, one shared pair deadline/no retry, randomise readiness,
initialization expiry, native event handling, cancellation/held/failed/uncertain
saves, bus cycles during save, metadata-only recovery, strict payloads/authority,
saved/occupied slots, empty reservations and separate real-instance discovery.
An independent consumer preserves existing intent/module identity and refuses
pre-discovery generations for new children. The save-cancellation fixture was
corrected to expect the existing Uncertain phase and explicitly retry recovery
while the released disk worker finishes; no production recovery change.

All ten new behavioral fault controls and four re-anchored shared commissioning
controls fail at their declared assertions, then restore source. Full acceptance
passes **475/475 native tests** across 11 binaries (four test processes), scoped
strict Clippy, Pi library/all four binaries, doctests (none defined), changed-file
formatting, wait registry, whitespace and both control-table plan checks.
Logs: `/tmp/dali-input-assignment-controls.log`,
`/tmp/dali-input-shared-controls.log`, `/tmp/dali-input-assignment-acceptance`.
Both sequential read-only source gates approve frozen **f016463** against
**29ccd37**, with no material findings (about one minute and 45 seconds).
Reviews used the same model family and ran no verification or mutations.
Physical I4/fixture timing remains unverified. Full bus
readdress/restore, identification and browser orchestration remain unfinished.
See [commissioning](commissioning.md), [design](superpowers/specs/2026-10-22-dali-v2-p5-input-assignment.md)
and `v2-input-assignment-controls.toml`.

## P5e — per-fixture group membership

Implemented saved-group creation and strict MQTT/typed APIs for captured reads,
single-bit assignment, reviewed hardware adoption and explicit full-mask repair.
Reuse existing group reducers and the common physical owner. Assign saves desired
intent before programming only the selected bit; adopt changes metadata only;
repair can explicitly clear unnamed slots. Report captured halves, quality,
staleness, expected/drift masks and confirmed revision/partial outcome. Preserve
the original stamp, permit and command completion across save/program. Group
membership changes rotate only affected group contracts; fixture identity and
intent continue. Read-class dispatch precedes mutation completion capture and
supports absent MQTT generation/trace headers. No SDK or UART recovery change.

Both sequential design gates approve; the first noted the Read-class completion
seam, now implemented. The initial two MQTT regressions failed with unknown
command before production changes. Local tests cover literal boundary frames,
FFFF and incomplete/collision masks, both known repeat counts, strict payloads,
create/rename/no-op, preservation of unnamed bits/other fixtures, adoption/repair
review, physical mismatch/partial failure, held/failed/uncertain/canceled saves,
bus changes through save, cancellation/expiry/native input and independent
consumer continuity. A literal peer initially captured only its master descriptor;
moving its whole PTY owner into the task fixes that fixture. The native-input
assertion initially used a target-specific topic; actual events carry target in
the controller-topic payload. Expiry tests distinguish request settlement from a
late terminal: admission can release after backend retirement while UART readiness
still refuses new work. Their local retry pace is shortened to fit the fixture's
bound; production's existing five-second recovery pace stays unchanged.

The first ten fault controls produced nine declared RED outcomes; the save-time
bus-cycle case stayed GREEN when the original-stamp guard was removed because
another receive-epoch check still refused programming. A focused already-observed
cycle handoff test isolates that admission boundary. An additional adoption test
checks stale evidence after confirmed save. Final control/restored/source-review
results are recorded below once run. Browser/pattern/bus-wide group orchestration,
group light/color operations and physical I4 validation remain unfinished.

The first restored gate passes 462 native tests, scoped strict Clippy, Pi
library/all four binaries, doctests (none), changed-file format/wait/whitespace.
Self-review then identified that a correlated group-setting collision was
classified as UART stream loss. A fail-first regression checks immediate link
usability and explicit readback after partial repair. Treat that valid bus terminal
as an operation failure, preserving the ready UART and saved intent without retry.
No change to transport ordering/recovery is required.

The next full run passes all group tests and 461/462 overall; an unchanged
atomic-intent test observes level 64 after its invalid request sequence. It passes
in isolation. Inspection finds its loop waits for one new Error, although one
rejected map reports one Error per property and pending Desired values coalesce.
Move its existing worker-order refresh barrier inside the loop, so each invalid
complete map finishes before submitting the next, and assert both errors from the
first two-property rejection. No SDK or output-routing code changes. Repeated
focused and restored results follow before source acceptance.

The corrected atomic-intent test passes **20/20 stress iterations**. Final
restored acceptance passes **462/462 native tests** across 11 binaries with four
test processes, scoped strict Clippy, Pi library/all four binary compilation,
doctests (none defined), changed-file formatting, wait registry and whitespace.
All **12 new behavioral controls** and **3 affected activation/reply/input controls**
are RED at their declared assertions. Logs: `/tmp/dali-group-controls.log`,
`/tmp/dali-group-additional-controls`, `/tmp/dali-group-collision-control.log`,
`/tmp/dali-groups-restored-gate`, `/tmp/dali-groups-final-gate` and
`/tmp/dali-groups-acceptance-gate`. The middle final-gate log retains the
461/462 test-synchronization failure; the acceptance log is the corrected result.
Both sequential read-only source gates approve frozen **88656fe** against
**c9fc2b3**, with no material findings. Reviewers ran no tests or mutations;
same-family review and absent physical I4/fixture validation remain explicit
limits. Review learning: distinguish per-property errors from whole-request
completion, preserve bus collision versus UART-loss scope, and isolate an
authority guard when another guard masks its removal.

See [operator/API guide](groups.md),
[design](superpowers/specs/2026-10-22-dali-v2-p5-groups.md) and
`v2-group-controls.toml`.

## P5d — explicit Remove from list

`remove_from_list` exposes existing `Edit::RemoveOutput` / `RemoveInput` through
thin metadata/proxy/item-authority mappings. It removes saved fixture lifetimes,
including all instances of a selected input module, and preserves hardware
addresses, levels, scenes, physical groups and input configuration. Output removal
cleans saved group references; continuing group identity remains stable while
membership binding tokens rotate. Original MQTT reply metadata survives deleting
the selected target. Unknown payloads/unsupported targets and stale revision/item
authority are refused. Rediscovery creates fresh lifetimes; consumers retire old
intent rather than transferring it by address/name. Store Home equipment handling
remains P8 work.

Both sequential design gates approved with no material findings. The first real
MQTT regression failed with unknown command before production changes. Seven new
tests and the existing explicit-removal consumer regression pass: output removal
and State retirement, physical output/input preservation, same-address rediscovery,
input subtree/queued-action retirement with continuing output 0, strict payloads,
item/controller token distinction, held/failed/uncertain saves and explicit
recovery, and independent-consumer identity/intent continuity. No SDK, UART
recovery or new persistence machinery was needed. All **5 new behavioral controls**
and **3 affected identity/activation controls** fail at their declared assertions.
The restored gate passes **444/444 native tests** across 11 binaries with four
test processes, scoped strict Clippy, Pi library/all four binary compilation,
doctests (none defined), changed-file rustfmt, wait registry and whitespace.
Logs are `/tmp/dali-removal-controls.log`, `/tmp/dali-removal-affected-gate` and
`/tmp/dali-removal-restored-gate`. Both sequential read-only source gates approve
frozen `cc2b986` against `ff75fca`, with no material findings. Reviewers performed
no tests or mutations. Same-family review and absent physical I4/fixture evidence
remain explicit limits. Review learning: expose existing reducers through thin
entry points, and prove target deletion preserves the original command completion
while retiring every other use of that lifetime.

See [operator/API guide](remove-from-list.md),
[design](superpowers/specs/2026-10-22-dali-v2-p5-remove-from-list.md) and
`v2-removal-controls.toml`.

## P5c — first-install bus setup

`configure_bus` / `MetadataEdit::ConfigureBus` upserts an explicitly named physical
bus through the existing durable owner. New buses have no fabricated fixtures;
updates preserve saved children, descriptions, memberships, color selections and
Origins. Enable-only changes preserve child bindings, rename rotates descendant
bindings, and identical setup is a no-op. Committed enabled buses update live HAT
polling without restarting the process. Actual enabled membership is distinct from
the internal bus-0 health fallback. Disabled caches/listeners are retired; native
notices carry private receive stamps checked against the latest snapshot before
SDK reporting, so disable/re-enable cannot revive old actions. Asynchronous bus
health keeps its existing best-effort semantics, without a new freshness or UART
recovery framework.

Both sequential design gates approved after clarifying queued-event lifetimes and
removing an unnecessary fresh-health promise. The first real MQTT regression
failed with unknown command before production changes. Nine focused tests pass:
empty-install bus creation and nonzero-bus discovery without restart, strict MQTT
and service authority, model validation/no-op/preservation, queued press retirement
and fresh release after re-enable, held/failed/uncertain saves, owned stale health
helper cancellation and independent-consumer continuity through rename/reopen.
The held-save test initially attempted recovery before its released filesystem
worker finished; a bounded retry of the explicit worker-busy response fixes that
test race. All eight original mutation controls and three affected recovery/
completion/native-helper controls fail at their intended behavioral assertions.
The first restored gate passes 436 tests, scoped strict Clippy, Pi library/all
four binaries, doctests (none), changed-file rustfmt, wait registry and whitespace.
Self-review then identified an overly strict event phase guard: a queued valid
action must survive an unrelated save while the old model is still active.
A new fail-first regression reproduces that loss; allowing Writing while retaining
listener/stamp validation fixes it and preserves the disable/re-enable rejection.
The ninth control also fails at its intended assertion. The next full run passed
436/437 tests and exposed the other side of the held-save fixture race: Uncertain
phase can be published just before its canceled owner releases the global permit.
The test now retries that explicit Busy response under the same original bound,
as well as the worker-busy response; the focused recovery test passes. Production
strict Clippy, Pi compilation, doctests and formatting passed that run unchanged.
The subsequent default-concurrency run passed all new bus tests but missed two
unchanged CLI startup tests' 2 s deadlines. Both pass in isolation; the complete
suite with four concurrent test processes passes **437/437** in 39.5 s. This
supports test contention as an explanation, not a proven host-level diagnosis;
no startup code or existing deadline changed. Logs are
`/tmp/dali-bus-setup-restored-gate`, `/tmp/dali-bus-setup-final-gate`,
`/tmp/dali-bus-setup-acceptance-gate` and
`/tmp/dali-bus-setup-bounded-concurrency-gate`. All nine new controls and three
affected controls are RED at their declared behavioral assertions; logs are
`/tmp/dali-bus-setup-controls.log`, `/tmp/dali-bus-setup-writing-control.log`,
`/tmp/dali-bus-setup-affected-controls.log` and
`/tmp/dali-bus-setup-event-control.log`. The final explicit original-correlation
assertions pass in all nine focused bus tests; changed-file format/whitespace
also pass. Both sequential read-only source gates approve frozen `c461bb2`
against `fea28ef`, with no material findings. Reviewers ran no checks or mutations;
same-family review and absent physical I4/fixture validation remain explicit
limits. Review learning: keep unchanged identity separate from receive lifetime,
check the event predicate during unrelated saves too, and avoid inventing a
health-freshness framework for the owner's accepted best-effort bus behavior.

See [bus setup guide](bus-setup.md),
[design](superpowers/specs/2026-10-22-dali-v2-p5-bus-setup.md) and
`v2-bus-setup-controls.toml`.

## P5b — one unaddressed output assignment

`assign_next_output` searches only unaddressed outputs, skips saved/missing and
physically occupied addresses, commits a fresh named reservation before PROGRAM,
then verifies its programmed short address and fresh status. Search/save/program
retain one permit. The continuation uses the original captured physical stamp
and a fresh private target generation while the MQTT reply retains its original
token/trace. Physical failure after confirmed reservation returns structured
reserved_but_unconfirmed; disk/CAS uncertainty retains existing freeze/recover.
No current light values or automatic retry/resume are invented.

The daemon exposes existing RepeatPolicy through
`--uart-repeat-terminals unverified|1|2`, default unchanged. Unknown profiles
refuse commissioning before the sequence; both explicit counts are tested with
independent literal peers. Hardware profiles remain unverified. Cleanup stays
within active helpers; an abandoned between-phase initialization uses its standard
timer or the next initial Terminate, without a cleanup job or UART journal.

Both sequential design gates approved after clarifying structured partial
outcomes and narrowing cleanup to the executor that can perform it. The first
MQTT test failed with unknown command before production changes. The implemented
test assigns a third unaddressed fixture to free address 2 while preserving
physical 0/1, input 0, existing names/groups and one original-context reply.
Local tests also cover the full search boundaries, both terminal counts,
exact/range collision distinctions and invalid replies, saved/physical capacity,
strict admission and CLI arguments, deadline/cancellation/native events,
save failure/recovery and save-time bus cycles, programming shutdown, failed
post-write queries and independent-consumer continuity after reopening the saved
reservation. A CLI fixture reused retained inventory as a startup signal;
fresh private brokers per accepted launch fixed that test race.
All **8 new mutation controls** and **2 affected metadata controls** fail at
their declared behavioral assertions. Logs are
`/tmp/dali-commissioning-controls.log` and
`/tmp/dali-commissioning-affected-controls.log`. Restored checks and frozen
source-review evidence follow before this slice closes. The initial restored
run passed 427 tests and failed only its wait checker: an internal `send`
wrapper collided with the checker's channel-send method name. Renaming it to
`exchange` preserves execution and the existing tagged leaf transaction wait;
the corrected acceptance run is in `/tmp/dali-commissioning-final-gate`.
That final gate passes **428/428 native tests**, scoped strict Clippy, Pi
library/all four binary compilation, doctests (none defined) and whitespace.
Changed-file rustfmt also passes. Both sequential read-only source gates approve
frozen `e05ea13` against `e2ed360`, with no material findings. Reviews ran no
verification or mutations; same-family and absent physical validation remain
explicit limits. Review learning: put post-commit partial outcomes in the public
type and promise cleanup only where an executor still owns it.

Input addressing, full reassign/restore, identification controls, group/scene
workflows and the protected browser UI remain unfinished. See
[operator/API guide](commissioning.md) and the
[slice design](superpowers/specs/2026-10-22-dali-v2-p5-unaddressed-output.md).

## P5a — addressed discovery

One bounded `discover_address` operation probes an output or input module and
durably adopts verified additions with default names. Existing names, Origin,
binding generations, missing items and smaller saved input lists are preserved.
New input instances receive their own child lifetimes. Full output multi-type
enumeration uses 254 termination and preserves incomplete prefixes. Input
discovery retains unknown valid types and empty-module metadata.

The physical permit spans queued work, actual settlement and durable save.
Original MQTT completion survives controller generation rotation; repeat scans
are no-ops. Captured `probe` reports never create current fixture availability.
Save-time hardware disappearance leaves confirmed additions reserved. Existing
uncertain-save recovery is reused; no UART journal, outage retry or SDK change.

Both sequential design gates approved after clarifying the 254/255 distinction
and explicitly historical probe semantics. The first real MQTT regression failed
with unknown command before production changes and passes after implementation.
Focused local tests cover literal full/malformed type sequences (including more
than nine types), missing/collision/partial input evidence, both address spaces,
common/unknown/zero input instances, child expansion and independent-consumer
continuity, type mismatch/smaller counts, strict requests/retired authority,
native events during cancellation, permit retention, physical epoch loss and
held/noncommitted/uncertain saves. A literal peer initially lost its PTY slave
through Rust field capture; retaining the whole PTY fixed that test fixture.
All **7 new mutation controls** and **2 affected metadata controls** fail at
their intended behavioral assertions. The restored gate's 411 behavioral/native
checks passed, with one registry failure: the new owned `discover` wrapper needed
an explicit local-method declaration. After that registry-only correction,
the complete suite passes **412/412 tests**. Scoped strict Clippy, Pi library/all
four binary compilation, doctests (none defined), changed-file rustfmt and
whitespace pass. Logs: `/tmp/dali-discovery-controls.log`,
`/tmp/dali-discovery-affected-controls.log`,
`/tmp/dali-discovery-restored-gate` and `/tmp/dali-discovery-final-gate`.

Both sequential read-only source gates approve frozen `a180167` against
`af3648c`, with no material findings. Reviewers inspected source/test contracts
and ran no verification or mutations. Same-model-family review and absent
physical hardware evidence remain explicit limits. Keeping captured probe
semantics separate from current State and reusing the existing durable owner
held through both reviews; retain those boundaries for addressing work.

This is discovery of already addressed devices. Unaddressed assignment, complete
readdress/restore, identification controls, group/scene workflows and the approved
web UI remain future increments. No physical I4 or fixture evidence is claimed.
See [operator/API guide](discovery.md) and the
[slice design](superpowers/specs/2026-10-22-dali-v2-p5-addressed-discovery.md).

## P4c4 — current input readback

Saved input instances now query physical type, resolution and all value/latch
bytes through the same UART owner/permit as light operations. Exact 1..255-bit
values use canonical bytes/hex; contact/button/occupancy values are interpreted
only with compatible resolution/type. Missing/colliding/changed-type evidence
clears old State rather than inventing released/off. Instance and aggregate MQTT
refresh now query inputs, retaining the original aggregate ceiling/fair cursor.
Native events remain distinct; polling generates no actions, and a newer native
observation wins over a latched query. The independent emulator has optional
current-value profiles and explicit value controls without changing old scenarios.

Verified locally on 2026-10-22:

- The first real MQTT quiet-button refresh test failed before production changes.
  **399 native tests passed**, including 17 added cases for exact/padded/wide
  values, common types, independent latches, collision/missing/type mismatch,
  merged State clearing, original traces, newer native events, cancellation,
  rename, ready-reply expiry, final acceptance and managed shutdown.
- A 255-bit literal peer paced at 80 ms/reply passes the fixed 4 s MQTT budget;
  changing it to 2 s fails the intended completion assertion.
- **12 new fault controls and 8 affected existing controls** all fail at their
  declared behavioral assertions. Old anchors now reference shared operation
  names and the current aggregate budget path. No recovery behavior changed.
- Scoped strict Clippy, Pi library/all four binary compilation, changed-file
  rustfmt/whitespace, wait registry and doctests (none defined) pass. Logs:
  `/tmp/dali-input-readback-controls`, `/tmp/dali-input-readback-restored-gate`
  and `/tmp/dali-input-readback-final-gate`. The registry's missing explicit
  expiry wording was corrected before the final full-suite pass.
- Both sequential design and source gates approve frozen `5c485172` against
  `b3a72b9`, with no material source findings. Reviews inspected source/tests
  without running verification or mutating files; same-family limits apply.

Remaining input work: hardware discovery/addressing, event configuration,
feedback, unresolved group/source routing, calibrated units and physical device
validation. Native absolute/light events retain their existing ten-bit raw
meaning; full-resolution/Boolean contact projection comes from a query. No live
UART, production broker, Store, physical fixture or firmware validation is claimed.

## P0 — configuration boundary implemented

The library `src/config.rs` and development binary `mqtt_dali_v2 --check-config`
select a valid existing v2 first, otherwise import v1 into a separate file or
initialize an empty inventory. They preserve legacy bytes and names, addresses,
ordering and group membership. New files are private, size-bounded and published
atomically without replacing another creator's destination. Invalid v2 never
falls back. Synchronous filesystem work and diagnostic output run on detached
workers with bounded caller waits. This is a check command; daemon startup remains
legacy until P3 integrates the new service and promotes the executable.

Verified on 2026-10-21:

- Native full suite: **72 passed**, including 27 migration/library/process cases
  and the existing legacy regressions; all use isolated fakes, never the live bus
  or broker. The wait registry passes.
- Strict Clippy for library, v2 binary and migration process tests: passed with
  warnings denied. New Rust files pass rustfmt; `git diff --check` passed.
- Raspberry Pi `armv7-unknown-linux-musleabihf` library/v2 compile check: passed.
  This does not verify linking, deployment or physical hardware.
- Tests were observed failing before implementation. The full-output-pipe
  regression was also observed failing before its fix.
- Five reproducible negative controls all failed for their expected reasons;
  see `v2-migration-controls.toml` and its recorded ledger. They remove alias,
  no-follow, no-replace, output-deadline and worker-detachment protections.
- Separate sequential design/contract reviews and a fresh code review completed.
  The code review requested bounded diagnostics, stronger alias tests and an
  explicit default Cargo binary; all were fixed and reviewed again with an
  approval for P0. Available reviewers are from one model family; cross-family
  review remains unavailable.

Limitations: permission checks skip root; check output is best effort after
200 ms; directory ownership is trusted. Crash-during-write/retry is tested, but
power-loss durability and malicious directory replacement are not. Input, scene
and capability fields will extend the inventory in their owning phases; this
pre-release schema rejects unknown fields rather than discarding them.

Review learning: process-exit tests must include full unread output pipes as well
as stalled workers; path-safety tests must start with absent files, since an
unrelated parse error can otherwise hide a missing guard. No shared skill files
were changed during bridge implementation.

## P1a — typed wire, monitoring and serial substrate implemented

`src/atx/wire.rs` preserves bus, width, collision and version fields; its
incremental framer bounds storage and partial-line lifetime without discarding
idle events. `monitor.rs` owns one continuous reader and serializes byte writes
under caller deadlines. Subscribers get explicit lag, persistent lifecycle status
and error/EOF/panic notification. Cancellation during a possibly partial write
prevents further writes. Close cancels an active writer and releases both stream
halves even when client handles remain.

`serial.rs` uses nonblocking Unix descriptors with raw 19200/8N1 settings on both
Pi UART and desktop PTY. Opening/configuration runs on a detached worker under a
2 s caller deadline. A local PTY test checks actual terminal settings, idle input
events and exact outgoing bytes through the production adapter.

Verified on 2026-10-21:

- Full native nextest suite: **92 passed**, including 20 new wire/monitor/PTY
  cases and all 72 previous regressions. Tests and close/write-failure regressions
  were observed failing before their corresponding implementations/fixes.
- Strict Clippy for the library, v2 binary and config process tests: passed;
  Raspberry Pi library/v2 compile check: passed. Neither is a Pi runtime test.
- Six reproducible UART negative controls all failed for their expected reasons:
  line size/lifetime, write poisoning, close cancellation/resource release and
  raw mode. See `v2-uart-controls.toml` and its recorded ledger.
- Fresh source review requested descriptor release and EINTR handling. Fixes were
  reviewed again and approved for this substrate scope. EINTR retry was inspected,
  not fault-injected; physical UART teardown remains unverified.

Review learning: resource ownership must be tested after close while public
handles remain alive. Tests also distinguish canceling a queued write from
canceling a potentially partial write, since only the latter poisons admission.

The raw `write` receipt means only bytes accepted by the kernel. P1b supplies the
exclusive transaction owner above it.

## P1b — transaction dispatcher implemented

`Transactions` privately owns the monitor and a continuously running dispatcher.
Capacity-one admission covers queued and active work. The same reader fences old
buffered/partial lines; framing and the first accepted-write sequence boundary
share the split-stream lock. Explicit drain emptiness is distinct from retry
Pending. Async input events, other-bus replies and echoes cannot complete a query.
L bit 16 distinguishes commands from input events.

Fully written canceled/timed-out exchanges retain their remaining terminal count.
Corruption, lag, interference or a potentially partial write requires a later boot;
an arbitrary reply or version message cannot reopen admission. Initialization is
explicit. Unverified j/t completion is refused; verified/emulated repeat profiles
can declare one or two terminals. Close resolves an active caller and releases
the stream. No transaction result reports physical state confirmation.

Verified on 2026-10-21:

- Full native nextest suite: **118 passed**, including 46 ATX cases and all
  migration/legacy regressions. Production Serial plus Transactions exchanges
  actual bytes through a local PTY with interleaved input and other-bus traffic.
- Strict scoped Clippy passed; Pi library/v2 compile check passed; diff whitespace
  and Rust formatting checked. The wait registry passes after its required tags,
  local-method declaration and explicit expiry outcomes were completed.
- Six new negative controls all failed for their expected assertions: retry/empty
  confusion, incomplete-line boundary, caller and actor expiry, lost late slot,
  and unverified repeats. See `v2-transaction-controls.toml` and recorded ledger.
- Sequential design review restored a dedicated actor to keep idle events drained.
  Fresh code review found drain/first-write and ready-result deadline races; fixes
  and focused regressions were reviewed again and approved. Cross-family review
  remains unavailable. Physical HAT firmware behavior remains unverified.

Review learning: generic AsyncRead Pending may mean retry/cooperative yield rather
than an empty buffer. Correlation requires a stronger explicit drain contract.
Deadline tests must separately exercise actor completion and a caller resuming
after a ready result, since either check can hide a missing guard in the other.

## P2 — independent standalone ATX emulator implemented

`src/emulator` owns strict physical scenarios independently of bridge inventory,
with separate UART parsing/formatting and literal vectors. Its base gear model
supports levels/MASK, steps, limits, selected status/configuration queries, DTRs,
groups/scenes and commissioning address selection/search/program/withdraw. Model
time can be realtime or frozen/advanced; seeded randomization is reproducible.
Duplicate addresses affect all selected fixtures and conflicting replies collide.

`dali_hat_emulator` creates a private PTY and optional private Unix JSON-control
socket. It uses tracing-init, bounded detached startup/output workers, bounded
queues/timers/clients, persistent UART write offsets and independent transport
deadlines. Controls inject events/faults, inspect actual fixture state and simulate
address loss, bus power changes and HAT reboot. Reboot preserves fixture DTR and
initialization state and delimits truncated output. Both PTY descriptors are
close-on-exec and released before successful close.

Verified on 2026-10-21:

- Full native nextest suite: **146 passed**, including 24 emulator library tests
  and four standalone process tests, plus all 118 previous regressions. Real PTY
  tests use production Serial/Transactions and verify independent snapshots.
- Process checks cover private socket permissions, refusing existing paths,
  invalid/oversized/unfinished controls, and SIGTERM with full unread output.
  Model tests cover range versus exact selection, binary search, repeat windows,
  expiry, RESET retention/isolation and unsupported operation behavior.
- Strict scoped Clippy passed. Pi library/v2/emulator compile check passed.
  Wait registry and formatting passed. Cross-linking and Pi runtime remain P9.
- Six emulator negative controls failed for their declared assertions: repeat
  gating, exact selection, reboot delimiter, retired controls, timer bounds and
  partial-line lifetime. See the control table and recorded ledger.
- Sequential design/code reviews completed. Code findings for truncated-record
  recovery, mixed-width repeat invalidation and STEP UP/RESET semantics were
  observed failing in regressions, fixed and reviewed again with approval.
  A further two-fixture RESET test closes the review's nonblocking coverage gap.
  Reviews are source-only and from one model family; physical behavior unverified.

Limits: immediate direct/scene levels, no timed UP/DOWN/fade physics yet. DT8,
semantic inputs, emergency and memory/data models belong to P4. The synthetic
version/repeat profiles are not real-firmware evidence. Unix socket cleanup is
best effort after exit. Usage and capability boundaries are in `emulator.md` and
`support-matrix.md`. Browser emulator controls follow the protected HTTP phases.

Review learning: track UART record boundaries from accepted bytes across frames,
including deliberate faults; HAT reset must not reset separately powered gear.
Independent models need adjacent opcode/default tests as well as roundtrips.

## P3a — qualified target codec frozen and implemented

`src/targets.rs` provides typed construction/parsing for output/input/group/scene
targets and input instances, with a separate inventory route. Uppercase UTF-8
percent escaping preserves names exactly while keeping dot-separated targets
inside one MQTT topic segment. Strict canonical decoding rejects aliases and
invalid names; the 4096-byte encoded bound counts separators and namespaces.

Verified: four focused tests cover literal/collision/Unicode/metadata vectors,
invalid forms and exact 4096/4097-byte output/instance boundaries. Three meaningful
cases were observed failing before implementation. Three negative controls caught
removed canonicality, dot escaping and encoded-size guards. Separate design and
code reviews approved the slice; review additions cover lowercase valid-name
escapes and the exact instance bound. Full native suite: **150 passed**; strict
scoped Clippy and Pi compile checks passed. No MQTT/Store caller uses this codec
yet; complete topic/packet validation belongs to integration.

The remaining P3 work includes shared managed-service lifecycle, authoritative
inventory/request-generation fencing, the DALI service and canonical v2 daemon.
The managed-service design is grounded in current BridgeKit's run/start split;
it retains the single shared logging/signals/watchdog policy.

## P3b — managed process service implemented in BridgeKit

Shared checkout: `Projects/tracing-init-dali-v2`, branch
`feat/dali-managed-services`, commit `a3db09e` (local, unpublished).
`ProcessService`, `ServiceContext` and managed run/start APIs keep process-owned
resources independent of MQTT/device link lifetimes. Readiness validates the
proxy once; an independent heartbeat watcher closes admission after service
death. Startup cancellation retains cleanup, and all component teardown shares
one absolute deadline. Success confirms future, local-task and runtime-resource
destruction; blocked helpers/destructors cannot claim a clean exit.

Verified in the shared checkout: **160 kit tests** and **174 conformance tests**
passed, strict all-target Clippy for both crates passed, and the armv7 Linux musl
library compile check passed. Conformance used explicit `RUST_LOG=info`; the
inherited `warn` filter hid INFO in two existing log assertions. Six negative
controls detected their declared removed protections, and the restored full kit
suite passed. Design and fresh core reviews approved after readiness/admission,
runtime-teardown and absolute-deadline fixes. Reviews are source-only, same model
family; physical hardware remains unverified.

The bridge has not adopted this dependency yet. Portable dependency pinning is
required before daemon integration is final. Inventory reconciliation and stale
request fencing are the next P3 contract boundary. The shared spec and its
verification record are mirrored in `superpowers/specs/2026-10-21-dali-v2-p3b-services.md`.

## P3c1a — pure inventory/reconciliation layer implemented

`inventory::Model` validates complete schema-1 wire snapshots, initializes the
legacy schema-2 baseline, replays immutable named history and proposes atomic
edits. It distinguishes continuity from replacement, preserves semantic group
membership through address moves, and rotates dependent group fences on address
reuse. Revisions allocate above the private high-water value. Present null or
incomplete ledgers fail validation. Imports preflight canonical targets and the
whole snapshot before creating v2, with exact document/encoded-target bounds.

A separate consumer algorithm and literal wire fixture verify offline rename
chains, rename-back, swaps, removal/reuse, preserved device record ids, intent
continuity, old-generation observation rejection and atomic rejection of invalid
or partial documents. These are independent-consumer contract tests, not actual
Store migration/binding tests. The latter remain P8.

Verified: **164 full native tests passed**, six declared negative controls caught
their removed guards, and the armv7 library/v2/emulator compile check passed.
Scoped strict Clippy is part of the checkpoint gate. New behaviors were observed
failing first, including identical-text replacement and rename-back intent
continuity. Two sequential design reviews cleared this slice after uncertain
save, transition-chain and runtime-bypass refinements. Fresh source review found
missing intermediate bus-name/index consistency and incomplete metadata checks
in the reference consumer. Three regressions reproduced those gaps; fixes were
reviewed again and approved. The complete gate below includes those regressions.
Guidance and current limitations are in `inventory-contract.md`.

This remains a pure proposal/snapshot API; the durable owner below applies its
proposals. Generation-context enforcement and runtime
state retirement ordering require the P3c2 gate before mutable daemon integration.

## P3c1b — durable configuration owner implemented

`config_store::ConfigStore` owns one runtime blocking worker and an adjacent
private lifetime lock. Save admission is nonwaiting and serialized. Both candidate
documents are preflighted before an old-mapping revision reservation is saved,
then the new mapping is atomically replaced and synced. Every public revision,
including descriptions, is reserved; v1 is never created or modified by this owner.

Timeout/cancellation preserves the active worker's slot and lock. Uncertain
replacement stays frozen until explicit recovery reloads a recognized candidate
and confirms file/directory durability. Cancellation and successful finalization
share one state transition. Startup confirms durability too. Close releases file
ownership across retained handles; a blocked worker returns false under the
caller deadline and remains part of managed-runtime teardown.

Verified on 2026-10-21:

- **182 full native tests passed**, including 15 persistence tests and three
  inventory review regressions. Local fault injection covers before/after both
  replacements, cancellation, queued/startup cancellation, failed sync/recovery,
  revision burning/restart, unknown external edits and lock/path safety.
- Eight reproducible negative controls all detected their declared removed
  guards; see `v2-persistence-controls.toml` and its recorded ledger. The restored
  full suite and wait registry passed. Strict scoped Clippy and the armv7
  library/v2/emulator compile check passed; changed Rust formatting/diff checked.
- Fresh source review found a finalization/cancellation race and cleanup armed
  before temporary-file ownership. Deterministic regressions were observed
  failing first; fixes passed re-review. Review was source-only, same model family.

This is a library checkpoint. Process-level stalled-filesystem SIGTERM tests
belong to canonical daemon integration; physical power-loss behavior and Pi
runtime are unverified. Operator-owned directories and ordinary local Unix
filesystems are assumed. No new daemon MQTT/hardware routing is admitted yet.

## P3c2 — shared request-generation fencing implemented

Shared dependency commit: `273062f05c1f0df05cb35f19bad458f2691d8dea` on
`feat/dali-managed-services` in `Projects/tracing-init-dali-v2`. The reviewed
contract and verification record are mirrored in
`superpowers/specs/2026-10-21-bridge-target-generations.md`.

Fixed Bridge instances (including managed startup) can opt into complete target
generation tables. MQTT metadata, queued work, configuration, commands, reads,
reports/removals and sealed publications retain their original context. Atomic
admission/table retirement prevents old intent replacing fresh queued work;
late observations cannot enter a replacement's model or conflict history.
Default unfenced behavior remains compatible. Dynamic factory fencing is
explicitly unsupported and rejected before connection.

The full kit gate passed 194 tests and the existing black-box conformance suite
passed 174 tests. Strict all-target Clippy, three rustdoc tests and the armv7
library compile check passed. Sixteen guard-removal controls each failed at its
intended behavior assertion, including admission/post interleaving, stale direct
and proving reads, original output metadata, late removals and legacy retry/cache/
partial-state compatibility. The scheduling test is controlled coverage with a
200 ms window, not an exhaustive interleaving proof. Two sequential source
reviews approved after four reproduced compatibility findings were fixed;
cross-family review is unavailable. No live broker, HAT or Pi runtime was tested.

The shared runtime spec, README, tutorial and canonical bridge skill were
synchronized. The dependency commit is still local/unpublished. Actual DALI
service routing, durable-save/retirement/publication ordering, and canonical
daemon integration remain next; this bridge has not adopted the new dependency
yet. `mqtt_dali_v2` remains the configuration-check development entry point.

## P3c3a — managed inventory service implemented

`service::DaliService` now owns the durable inventory and routing phase in a
managed BridgeKit service. Owned snapshots and bounded notifications feed
frontend handles; one nonwaiting operation permit spans each admitted save or
recovery. Public edits are metadata-only. Hardware replacement/readdress/group
membership require later verified commissioning jobs.

Reporter attachment is independent of broker/HAT availability. A durable change
retires the complete old runtime table, queues Inventory and only then exposes
Ready. Uncertain saves and failed runtime CAS remain frozen. Explicit recovery
compares the full public model, including recognized mappings recovered with
`changed: false`; runtime-only retry retains the original exact CAS precondition.
The proxy checks both persisted and actual runtime controller identity and
preserves original caller deadlines. Version and UTC build time are in snapshots.

The bridge pins all tracing-init repository dependencies to local development
commit `259d963fd448ea2512e36a00e2f22850af0bc1f7`. This includes a reviewed
single-use command-completion capability: a metadata MQTT command can return one
original-token success across its own inventory commit; all unrelated/queued old
work still retires. The dependency is unpublished; a portable fresh clone/release
still requires publication. Tokio 1.50 in the bridge lock resolves the older
lock's missing OwnedNotified API.

The independent consumer now buffers bounded State-before-Inventory observations,
separately from identity and intent. Exact-token retractions, accepted/ignored
inventory outcomes and total key/payload cache budgets are exercised, along with
an actual MQTT inventory rename preserving consumer records and owned intent.

Verified on 2026-10-21:

- **201 complete native tests pass**, including actual output/group/bus/description
  MQTT success replies, canceled-save recovery via MQTT and direct handles,
  original generation/correlation, stale requests, directory-sync/CAS failure,
  absolute caller deadlines, instance checks, bounded update lag and actual
  blocked-filesystem incomplete stop. Existing migration/UART/emulator/legacy
  regressions and the wait registry remain green.
- Nine service/consumer controls each failed at the intended assertion; the runner
  restored source and recorded `v2-service-controls.ledger.json`. Strict scoped
  Clippy, armv7 library/v2/emulator compilation and changed-file formatting/diff
  checks passed. The shared completion boundary separately passed 200 kit tests,
  174 existing unfenced conformance tests and four controls.
- Sequential source reviewers approved after three reproduced integration findings
  were fixed: own-command reply retirement, ready-after-deadline acceptance and
  actual runtime instance mismatch. Review is source-only and same model family.
  The identity rejection test has a controlled 500 ms absence window; it is not
  an exhaustive scheduling proof.

See `service-development.md`, `inventory-contract.md` and the P3c3 contract.
This is still a service-library checkpoint. UART ownership/health and canonical
daemon startup come next. Physical capabilities, web UI, Store persisted migration,
live Logmon delivery, hardware validation and release/deployment remain pending.

## P3c3b — managed UART health implemented

Historical checkpoint: the general Boot-only recovery prerequisite below is
superseded by the owner's measured quiet-period decision in P4a.

The optional `with_uart` builder starts one Serial/Transactions owner inside the
managed service runtime. Version and enabled-bus status queries use one original
setup deadline; metadata remains available during initialization, MQTT/HAT loss
and poisoned-response waits. Hardware snapshots preserve raw version/status
facts independently of saved routing phase, including Connecting and actual
Closed. Bus power faults do not masquerade as a disconnected HAT.

Late/reset states keep the same owner. Reopen retains a reset latch and a
no-write receive fence excludes already-readable complete/partial Boot records.
Only qualifying Boot plus the transaction actor's Uninitialized state admits
initialization. This does not attribute upstream-delayed data to a physical reset.
Before fixture Send is enabled, P4 must also preserve unresolved transport
ambiguity across process crashes/startup; health queries do not claim that wider
guarantee. Firmware fields never select repeat policy implicitly.

Open/configuration uses a runtime-owned blocking worker. Open timeout is terminal
for the UART owner; actual runtime teardown still includes a blocked opener.
Owned helper results survive canceled selects, and panic results are inspected
before recovery. Hardware/configuration cleanup shares one absolute local budget.

The focused PTY/emulator cases cover late Version retirement, poisoned-owner
Boot recovery, actual transport replacement, complete/partial buffered Boot,
quiet retry after failed boundary, completed-helper panic during Boot/loss,
completed-boundary/fresh-Boot ordering, proactive observation ordering, metadata
during unanswered setup and actual descriptor release with retained handles.
The blocked-opener case confirms incomplete managed stop and no retry eligibility.

Source reviews reproduced and fixed three recovery/ownership issues: a missing
retry timer, a completed panic discarded by a simultaneous hardware event, and
a successful boundary discarded before a qualifying Boot was evaluated. Connecting
and Closed publication were corrected too. These are source-only same-family
reviews. Final verification on 2026-10-21: **215 full native tests passed**,
scoped strict Clippy and armv7 library/v2/emulator compilation passed, and changed
Rust formatting/diff checks passed. All eight controls in
`v2-uart-service-controls.toml` failed at their intended assertions; restored
source passed the full gate. No physical I4, live broker/Logmon or Pi runtime is
implied. The UART slice remains a library integration; canonical daemon startup
is next.

## P3c3c — canonical daemon and original request traces

The canonical `mqtt_dali` and `mqtt_dali_v2` alias now start the managed v2
service through BridgeKit. Explicit `--check-config` preserves the isolated
offline operation. The former main is byte-preserved apart from its test-helper
path in `mqtt_dali_v1`, a regression harness; deployed legacy fixes remain on
the separate v1 maintenance branch/tag. Package/retained Version reports
2.0.0-dev.0 and actual UTC build time. No install scripts or live services changed.

The common CLI parser preflights options and renders usage through bounded
output, then valid arguments pass unchanged to BridgeKit. The standalone offline
flag is recognized only in flag position; even a controller named --check-config
works with split or inline option values. Saved identity is checked before UART.
First startup migrates v1, restart uses v2, and deletion of v2 reimports with a
new inventory epoch while preserving v1 bytes.

Real canonical process tests cover MQTT metadata replies, clean/incomplete stop,
held logging/config/save startup, actual broker-outage-independent UART health,
full unread output pipes, idle CPU and private local OTLP export. The shared
trace defect exposed by that test was corrected in reviewed local tracing-init
commit `96a03bcb9f932d8d49d479a38cc95f9ad9e08ff6`, consistently pinned across all
shared dependencies. Validated owned causes cross queue boundaries independently
of generation authority; coalesced callers retain their own traces. The DALI
actor exports an operation span, and committed Inventory uses the new generation
with the original cause. The exact success reply retains original generation,
correlation and trace. Actual decoded OTLP JSON proves worker and actor spans
under the incoming trace at INFO, rather than an ingress-only match.

Shared restored verification passed 211 kit tests, 174 existing conformance
tests, strict all-target Clippy in both crates, rustdoc and armv7 compilation.
Eight shared controls failed at their intended assertions; README/tutorial,
runtime design and canonical bridge skill were synchronized. Two sequential
source reviews approved after reproduced attribution gaps were fixed. An
existing proving-read test race was corrected with an actual connected barrier.

Daemon source review approved after fixing an Inventory-publication wait, a CLI
mode/value ambiguity and a displaced Markdown separator. The new daemon controls,
changed recovery control and relocated legacy/migration controls are recorded
beside their ledgers. Final restored daemon verification passed **225 native
tests**, scoped strict Clippy, armv7 library/canonical/alias/emulator/legacy-harness
compilation, changed Rust formatting, wait registry and diff checks. The explicit
test-feature logging-start case also passed. All **60** controls in this batch
failed at their intended assertions: five daemon, one changed recovery, five
relocated migration and 49 relocated legacy cases. Restored source passed after
all mutations. No physical behavior or Pi linking is implied by compilation.

See `running-v2.md` and `service-development.md`. The dependency commit is local;
portable upstream clones still require publication. HTTP/capabilities, Store
migration, physical I4/Pi runtime and live Logmon validation remain pending.
Fixture Send is still disabled pending the P4 capability engine. The earlier
crash-evidence prerequisite is superseded by the owner's P4a recovery policy.
Available source reviewers are same family; no cross-family independence is claimed.

## P4a — measured quiet recovery implemented and verified locally

The owner chose measured UART silence instead of the proposed persistent
clean/dirty journal. No journal, state directory or first-start reboot requirement
was implemented. One continuous Tokio AsyncFd reader still processes raw
19200/8N1 input and publishes complete records during recovery. The new
`--uart-quiet-ms` option accepts 1..5000, default 250, before opening resources.
Every nonempty read restarts quiet acquisition, including partial/malformed
records; its total budget is the interval plus the existing setup allowance.

Exclusive actor recovery uses a guarded reader reset with original deadline,
cancellation identity and expected raw activity, then checks the sealed boundary
before entering Uninitialized. It retires complete-write timeouts/receive loss;
V/D initialization is still explicit. Known partial-write poison remains
Boot-only and is captured after actual teardown before a port reopen. Real joins
and lifecycle-owned panic cleanup release descriptors even with retained handles
or canceled close callers. Journal-only clean receipts/history were removed.

Behavioral RED→GREEN tests cover the timer, raw input, complete-write/corruption
recovery, automatic startup/reconnect, invalid CLI values and teardown bugs.
Controlled reader/actor tests cover canceled/expired reset, new activity after
proof, input visibility and partial poison. A canonical process test feeds input
before launch through inventory readiness, measures the configured interval,
kills an unanswered process and restarts with an old version buffered, without
Boot. The partial-write EOF test delivers owner closure before actor cancellation
and proves post-join poison survives into Boot-only reopening.

Two sequential design reviews approved the replacement. First full source review
approved production behavior and requested removing an independent-MQTT timing
race from the subprocess test; the feeder now starts before process launch.
Final source review additionally required caller cancellation to reach the reader
without actor polling. A shared synchronous authorization guard and final actor
check now enforce it; deterministic regressions were observed RED before each
fix. A final stress probe exposed starvation of the quiet timer by ready event
traffic. The actor now checks its deadline before each select, independently of
caller polling; that regression was also observed RED before the fix. Both
reviewers approved the corrected source. Reviews use the same model family.

Verified on 2026-10-22:

- Full native nextest: **246 passed**, including legacy and canonical process
  regressions. Strict scoped Clippy and Raspberry Pi library/all four binary
  compile checks passed. The feature-gated blocked-logging SIGTERM test passed;
  doctests passed (none defined). The wait registry passes.
- **16 new recovery negative controls** all failed for their intended behavioral
  assertions. The 25 existing UART, transaction, UART-service and daemon controls
  also behaved as declared. The affected transaction controls were rerun after
  the final deadline guard. Tables and ledgers preserve the actual evidence.
- Source formatting and whitespace passed. The full restored native suite passed
  all **246 tests** after the guard-removal mutations.

Physical I4 timing, Pi runtime, live Logmon and upstream publication remain P9.

Review learning: a cancellation flag must reach the resource owner directly,
actual task joins must precede resource-release claims, and a biased event loop
must check its total deadline independently of lower-priority timer polling.

## P4b1 — typed base-output codec implemented and verified locally

Two sequential design reviews approved a pure Part 102 codec. It validates
short/group addresses, normalizes ordinary levels within 0..254, creates basic
write/query frames and preserves query-specific MASK/type/status meanings plus
no-response/collision outcomes. Twelve literal-vector tests pass; the initial
new-API build could not compile until its types were implemented. This was not
a behavioral regression RED. Thirteen guard-removal controls subsequently failed
for their intended behavioral assertions, establishing sensitivity to removed
bounds, address/control bits, max/MASK distinctions, positive-level handling,
intent validation, no-response meaning and status polarity. The ATX integration
vector uses the production encoder.

Verified on 2026-10-22: all **258 native tests passed**, including the full restored
suite after controls; scoped strict Clippy and Pi library/all four binaries compile
checks passed. Formatting, whitespace, wait registry and doctests (none defined)
passed. Two sequential read-only source reviews approved the final codec; the
only requested cleanup was a stale design-status sentence. Reviews remain
same-family, and physical fixture behavior is unverified.

No MQTT fixture writes, configuration changes or emulator codec sharing are
introduced. Physical execution, original-generation admission, readback and
fair collection of jobs/events remain the next integration boundary. Reference:
[dali output contract](dali-output-contract.md).

## P4b2 — bounded dispatch and helper collection

Deterministic fail-first probes reproduced queued admission being stranded by
continuous input, completed HAT helpers being starved by renewed observations,
and normal EOF becoming fatal when its query result preceded public delivery.
Transaction dispatch now prefers queued requests after stop; ordinary command
drain/fence/deadline checks remain intact. The HAT owner consumes a bounded prefix
of 129 public receive attempts, then actually joins a completed helper and retires
the stored handle without a suspension between those steps. Queued bus facts keep
priority over earlier query snapshots. Processing work returns promptly even
when it leaves the hardware snapshot unchanged.

A Closed query or status starts owned teardown; its actual join determines
whether a paced retry is allowed. Late EOF cannot abort an owned Close helper.
Actual helper/actor failures remain fatal and post-join partial-write poison is
preserved. Five new tests cover cancellation/expiry, renewed input plus completed
panic/newer bus data, actual PTY EOF ordering, late EOF, status ordering and an
actual aborted actor with delayed public evidence. Six new guard-removal controls
all failed for their intended assertions. The test fixtures use private streams
and PTYs and remain bounded.

The full native suite passed **263 tests**; strict scoped Clippy,
Pi library/all four binaries compilation, wait registry, formatting, whitespace
and doctests (none defined) passed. All 30 existing transaction, UART-service and
quiet-recovery controls failed for their intended assertions. After restoration,
the full native suite again passed **263 tests**. Both sequential source reviews
approved without material findings. Reviews use the same model family.

This closes scheduling prerequisites; physical property execution and readback
remain pending. No live HAT, broker, Store or deployment changed.

## P4b3 — owned output operations and actual fixture readback

The shared service now reads an individual saved output and applies validated
power/brightness intents through its sole UART owner. Metadata and physical
operations share admission. A frontend timeout does not release that admission:
the owner keeps it until actual actor retirement or transport teardown. One
original deadline reaches transaction admission, receive fences and every new
write poll; cleanup has a fixed ceiling. No new frontend owns a UART clone.

Post-write actual-level/status queries provide observed results. Missing fixtures,
MASK, collisions and partial samples remain typed evidence, not an invented off
or echoed request. UART and per-bus counters invalidate stale work even when a
watch value returns to Ready before the owner polls. Mapping changes retain only
exact surviving tokens. Cache and reply share the same final validated sample.

Two source-review findings were reproduced behaviorally before correction:
an expired cleanup could accept a ready permit, and a bus transition could leave
a stale cache paired with successful current readback. Both regressions now pass.
Both sequential source reviews approved; they use the same model family.

The initial full native gate passed **282 tests**, plus strict scoped Clippy,
Pi library/all four binary compilation, wait registry and whitespace checks.
All **19 new guard-removal controls** detected their intended behavioral faults.
All **42 existing affected controls** also behaved as declared: transaction,
UART monitor, service lifecycle, quiet recovery and dispatch progress. The
status-ordering control now targets the current early-close detection site;
its original failure assertion is preserved. After restoration, all **282 native
tests** passed again; strict Clippy, Pi compilation, formatting, whitespace and
doctests (none defined) passed. Hardware validation remains pending.

This is a library execution boundary. Physical MQTT properties, group execution,
color and inputs remain subsequent P4 work. See [the service API](output-service.md).

## P4b4 prerequisite — explicit request context for SDK reads

Shared commit `153d38051f5197b060fa4281efe6136cffee7419` adds the optional
`read_state_with_request_context` hook. Queued refresh and individual conflict
reads now pass their owned original context; compatible defaults preserve
existing contextual/plain drivers and independently captured observation tokens.
Four focused tests include actual ephemeral-broker State/reply traces and
explicit absence across a driver-owned task without OTel. Both dispatch
regressions failed behaviorally before the worker fix.

Both sequential design/source reviews approved; reviews remain same-family.
The restored shared gate passed **215 kit tests** and **174 full conformance
tests**, all-target strict Clippy, Pi library compilation, formatting and wait
registry. Three new and ten affected existing controls detected their intended
faults. All four DALI dependencies now pin this exact commit; DALI's full
**282 native tests**, strict scoped Clippy and Pi library/all four binaries
compile checks pass with the new pin. Physical MQTT property execution is next.

The revision remains local and unpublished. The bridge-development skill and
shared README/tutorial describe the new method; no Store code changed.

## P4b4 — MQTT base output properties

Individual saved outputs now expose PowerState/Brightness through BridgeKit's
Desired/State interface. Actual level/status readback supplies the result, including
fixture clamping. State has explicit Available/Quality; retained last-known scalar
values are unusable while Available=false. MASK and missing fixtures never become
invented off values. Missing fixtures do not block healthy output control.

Physical publication stays on the SDK worker. Driver completions revalidate the
latest service snapshot after their final await; a LinkContext-owned helper reads
latest snapshots and lets the SDK coalesce reports. Source generation, UART/bus
evidence, original cause and explicit unsolicited trace absence remain separate.
Even unchanged post-write readback is reported before acknowledgement, so the
runtime's inferred intent cannot replace observed conflict/reconnect evidence.

Aggregate polling uses a persistent cursor, 15 s total ceiling and 2 s per target.
Cursor advancement precedes the cancellable await and survives disconnect. Final
validation covers early samples after the last fixture await. Disabled buses skip
I/O; polling without a ready UART rejects before taking physical admission, keeping
metadata configuration accessible.

Fifteen new regressions exercise the real managed runtime, independent emulator or
literal PTY HAT, and isolated FakeBroker. They include delayed read/outage ordering,
A-to-B-to-A publication, incomplete aggregate reads, cancellation and reconnect,
actual clamped readback, full-map rejection, missing devices, generation admission
and original request trace with no OTel dependency. The helper-cache and no-UART
admission bugs were reproduced before their fixes. Removing the direct unchanged
readback report also fails the real reconnect/conflict oracle as intended.

The initial complete gate passed **297 native tests**, strict scoped Clippy,
Pi library/all four binary compilation and whitespace checks. All **15 new
guard-removal controls** and **8 affected existing metadata/trace controls**
failed for their intended behavioral assertions. After restoration, all **297
native tests**, strict scoped Clippy, Pi library/all four binary compilation,
formatting, whitespace and doctests (none defined) pass. Both sequential source
reviews approve the documented bounded-rejection contract with no additional
material source finding. Reviews use the same model family. Implementation is
committed as `5943268`, with the explicit outage limitation in `1dee55e`.

Source review confirmed an outage policy boundary: the SDK abandons
Rejected state requests, so HAT recovery alone does not retry Desired consumed
during an outage or before UART startup readiness. This slice explicitly uses
target rejection to preserve metadata and healthy fixtures. The owner explicitly
accepted dropping commands during rare bus outages and stopped the proposed
deferral work on 2026-10-22. No production deferral code was implemented; the
exploratory test scaffolding was removed. The guide states the accepted behavior.
Proceed with the requested switch/color capabilities. Assess likelihood, impact
and simpler alternatives before expanding infrastructure for another edge case.
See [the MQTT output guide](mqtt-outputs.md) and the
[slice spec](superpowers/specs/2026-10-22-dali-v2-p4b-mqtt-outputs.md).

## P4c1 — common input protocol codec

The pure `dali::input` module decodes all five standard input-source schemes,
preserves device/instance type ambiguity and the full 10-bit information, and
distinguishes commands from events. Native Part 301 actions, Part 303 flags and
raw Part 302/304 data are typed without invented gestures or universal lux units.
Read-only device/instance query builders use validated independent input addresses.
This adds no UART reader, scheduler, saved schema or MQTT interface.

Eight literal-vector tests pass. Before implementation, seven tests failed for
missing decode/query behavior while the API skeleton's bounds checks passed. The initial
full gate passed **305 native tests**, strict scoped Clippy, Pi library/all four
binary compilation and whitespace checks. All **7 focused protocol controls**
failed at their intended behavioral assertions. After restoration, all **305
native tests**, strict scoped Clippy, Pi library/all four binary compilation,
formatting, whitespace, wait registry and doctests (none defined) pass. Both
sequential source reviews approve with no material finding. Reviews were
source-only and used the same model family; they did not independently rerun tests.

Both source-grounded design reviews approved this limited boundary; source
reviews remain same-family. Named discovery, persisted input identities, current
state, MQTT actions/configuration/feedback and semantic emulator controls remain
required P4 integration work. See [the codec guide](dali-input-contract.md).

## P4c2 — named input inventory and durable rename

Saved configuration now has separate input modules and nested typed instances.
The complete inventory gives both levels targets and mapping revisions. Empty
input arrays are omitted, preserving existing output-only documents and v1
import behavior. Input/output short addresses are independent; unknown native
instance types remain metadata. Input metadata uses the existing durable save,
complete-table runtime activation and MQTT rename command.

Module/bus rename preserves descendant lifetimes and rotates affected mappings.
Module replacement/removal retires all descendants even with unchanged text.
Producer and independent consumer validate hierarchy after every batch and
refuse a continuing child attached to a replacement parent lifetime. Instance
type changes retire mapping authority and prevent unsafe binding-intent carry.

The first JSON/projection test failed on unsupported inputs before implementation.
Nine new tests cover validation, literal nested projections, rename/replacement,
invalid history, independent consumer behavior and actual MQTT rename/persistence
with retired-token refusal. The initial gate passes **314 native tests**, strict
scoped Clippy, Pi library/all four binary compilation and whitespace checks.
All **6 new focused controls** and **6 affected existing identity controls**
failed at their intended behavioral assertions. The restored gate passes all
**314 native tests**, strict scoped Clippy, Pi library/all four binary
compilation, changed-file formatting, whitespace and doctests (none defined).
The existing wait registry test passes; this boundary adds no waits. Final
sequential source reviews approve frozen `c87bde8` with no material finding.
Reviews were source-only and used the same model family. Hardware discovery,
current input State, native action events/configuration/feedback remain required.
See [the input inventory spec](superpowers/specs/2026-10-22-dali-v2-p4c-input-inventory.md).

## P4c3 — live input State and native events

The existing UART reader/HAT owner now resolves enabled named input instances
and publishes observed State plus native non-retained events through BridgeKit.
Exact device/instance sources and unique saved device/type sources route;
ambiguous/unassociated sources do not receive guessed targets. Native button
actions remain native; absolute/light values remain raw 10-bit fields and
occupancy flags are decoded. Unknown/stale values explicitly clear applicable
properties to null, including a previously pressed button. Input Desired remains
unsupported. Refresh uses current observed projections without another hardware
query; aggregate refresh projects inputs after its final output await.

Per-route generation/UART receive boundaries reuse the existing same-reader drain
and partial-line-aware minimum sequence. Inline rename cannot attach a queued or
partially received old event to the new target token. Unchanged routes retain
their boundary. Metadata does not wait for UART, and no P4a quiet-recovery behavior
was reopened or changed. Snapshot input_listeners reports established routes,
separately from physical presence. Frozen routing suppresses action publication.

Native actions use a capacity-64 broadcast plus bounded HAT batch; loss is
explicit and never reconstructed from retained State. Only SDK-owned worker
helpers publish. Original source generations and absent traces survive the queue,
including events while a traced light command awaits its acknowledgement.
Broker outages preserve the existing SDK's bounded queued-event delivery policy;
there is no new SDK extension or broker-offline discard guarantee.

Three behavioral tests failed against the API skeleton before implementation.
Ten new tests cover source ambiguity, native actions and sensor fields, nullable
state, bounded notices, rename-back, actual MQTT during pending light commands,
old partial-line rename cutover, the existing ATX PTY emulator, cached refresh and
a controlled aggregate-read/input-change ordering. The first literal-peer test
exposed a test-owner capture bug: its retained PTY slave was dropped before the
bridge opened it. Keeping the whole owner in the task fixed the test fixture;
no production UART workaround was added.

The corrected initial gate passed **323 native tests**, strict scoped Clippy and
Pi compilation; the final aggregate regression then passed with the focused set.
All **11 new controls** and **6 affected existing helper/progress controls** failed
at their intended behavioral assertions. After restoration, **324 native tests**,
strict scoped Clippy, Pi library/all four binary compilation, changed-file
formatting, whitespace, wait registry and doctests (none defined) pass. The first
full attempt caught only wait-registry classification/tag issues; those were
corrected before the passing gates. Both sequential read-only source reviews
approved frozen `c0951fb` with no material finding. Neither reviewer ran tests or
controls; the parent ran the verification above. Physical-device validation
remains outstanding.
Both source-grounded design reviews completed; their nullable-state and SDK
broker-delivery clarifications are incorporated. Reviews use the same family.

Hardware input discovery/full-value reads, event configuration/feedback,
unresolved group/source mapping and semantic emulator controls remain required.
See [MQTT inputs](mqtt-inputs.md) and the
[live-input spec](superpowers/specs/2026-10-22-dali-v2-p4c-live-inputs.md).

## P4d1 — typed DT8 colour protocol

The pure colour codec now validates temperature/mirek, fixed-point xy and
normalized RGBWAF channels separately from power/master brightness. Complete
short/group/broadcast write plans load the correct DTR bytes, enable DT8 before
each extended command and ACTIVATE once. Unrequested channels remain MASK.
Individual read plans distinguish byte/word selectors, refresh colour reporting
and obtain low bytes only for word values. Raw bitmap FF, byte-value MASK and
full-word MASK remain distinct; no response/collision fabricates colour data.
Feature/status decoding preserves reserved counts and ambiguous active types.

Both sequential source-grounded design reviews approved this boundary with no
material finding. Seven focused tests pass, including all valid raw coordinate,
temperature and channel conversion round trips, literal command/query sequences,
transport failures and independent address spaces. Three command-plan tests
failed behaviorally against the empty API plans before implementation; initial
test-only type/ownership mistakes were corrected before that evidence was taken.
All ten fault-injection controls failed at their intended assertions in 55 seconds.
The restored gate passes **331 native tests**, strict scoped Clippy, Pi
library/all four binary compilation, changed-file formatting, whitespace,
wait registry and doctests (none defined). Both sequential read-only source
reviews approved frozen `64c05f9` without material findings. Reviews were
same-family, source-only, and did not run tests. No emulator/SDK/Store/runtime
path was changed. Physical validation remains outstanding.

Service discovery, physical limits/mode handling, actual colour readback, MQTT
representation properties, independent emulator semantics and colour scenes
remain subsequent implementation work. See [colour protocol](dali-colour-contract.md)
and the [slice specification](superpowers/specs/2026-10-22-dali-v2-p4d-colour-codec.md).

## P4d2 — independent DT8 HAT emulator

The existing standalone emulator now accepts optional strict colour profiles and
models selected Tc/xy/RGBWAF staging, activation, capability/status/value queries,
Tc limits and MASK. Whole triples retain unchanged actual channels; staged MASK
replaces earlier pending numeric values and activation clears temporaries. Base
master brightness stays independent. Existing addressing, group selection, reply
collision aggregation and DTR storage remain the one physical model path.

Primary gear-source inspection corrected the draft's overly broad frame rule:
the modeled DT8 selection is consumed by standard 16-bit commands before address
filtering/repeat-first return, while special DTR/24-bit input traffic preserve it.
No input-event invalidation hook was added. The combined design gate approved the
amended MASK lifecycle/selection contract. This uncertified reference is a
cross-check, not formal standards/firmware proof. The emulator remains independent
of the production codec. `colour-i4.json` supplies a separate four-bus example.

Four literal semantic tests failed first against the old unsupported path; valid
profile validation already passed. Six new model tests plus one real standalone
PTY process test now cover capabilities, unsupported modes, staged/actual state,
clamping, byte widths, group/broadcast selection, native input traffic and master
brightness. A missing generic parameter in the new process-test helper was fixed
before the passing run. All **11 new controls** and **2 affected existing base
controls** failed at their intended assertions. The restored gate passes **338
native tests**, strict scoped Clippy, Pi library/all four binary compilation,
changed-file format/whitespace, wait registry and doctests (none defined).
Fresh source review found that word colour queries left DTR1 stale. A literal
regression reproduced AA instead of the returned MSB; the model now updates both
registers. The added test covers normal/full-MASK words and known/MASK byte
queries preserving both registers. Its new control and two affected word/byte
controls failed as intended. The corrected restored gate passes **339 native
tests** and the same build/lint/format/wait/doc checks. Final read-only source
re-review approved frozen `2b60104`, confirming the finding resolved with no
remaining material issue. Review was source-only/same-family and ran no tests.

No service/MQTT/SDK/Store or recovery behavior changed. The emulator explicitly
does not yet model alternate RGBWAF control modes/channel assignment, physical
gamut/photometry, fades, colour scenes, automatic activation or DT8 reset/settings.
These remain subsequent work rather than inferred from base command success.
See [emulator colour profiles](emulator.md#colour-fixtures) and the
[slice specification](superpowers/specs/2026-10-22-dali-v2-p4d-colour-emulator.md).

## P4d3 — owned colour service

The individual output service now discovers DT8 capability (including bounded
multiple-type enumeration), physical RGBWAF assignments and active native values.
`read_colour`/`apply_colour` use the existing owned HAT job, permit, context,
deadline, stamps and final acceptance. Setters require fresh manual-activation
confirmation and supported representation; RGBWAF also requires normalized mode
and the requested physical colour assignments. Automatic activation and combined
base/colour ordering remain required before general MQTT colour mutation.

Actual readback retains clamping/range/limit evidence and must match the requested
representation to confirm a colour write. Fresh incidental colour samples have
their own quality and cannot fail successful base control. Whole-output epoch
invalidation marks colour stale. There is no new UART recovery, SDK queue or Store
dependency. The independent emulator now answers physical channel assignments.

The first actual-colour test failed behaviorally against the API skeleton before
execution was connected. Eleven new focused tests pass, including Tc/xy/RGBWAF,
untouched WAF/master independence, type enumeration, unsupported preflight, failed
word bytes, wrong active type, native inputs during cancelled colour work and
final epoch invalidation. Rust reply ownership errors were fixed before the
passing run. All **12 new controls** and **6 affected existing output controls**
failed at their intended behavioral assertions (91 s and 43 s). The restored
gate passes **350 native tests**, strict scoped Clippy, Pi library/all four binary
compilation, changed-file format/whitespace, wait registry and doctests (none
defined). The first source gate found a practical timing regression: known-colour
base operations still used the proxy's 2 s budget despite 73 correlated exchanges
for a six-channel RGBWAF+Tc sample. Three paced UART/MQTT regressions reproduced
expiry before a bounded correction: select 6 s for same-generation known-colour
targets before admission, retain 2 s for plain targets, and preserve the original
15 s aggregate ceiling and explicit service caller deadlines. All three paced
regressions and the original aggregate-clipping test pass. Their three new timing
controls and three affected existing MQTT controls fail as intended (28 s/24 s).
The corrected restored gate passes **353 native tests** and the same lint, Pi,
format/whitespace, wait and doc checks. Both sequential read-only source gates
approve corrected frozen `96424cc`, with no remaining material finding. The first
gate narrowly re-reviewed its timing correction; the second reviewed the full
`c050907..96424cc` change. Both were same-family, source-only, with no tests run
by reviewers. Physical fixture/firmware validation remains outstanding. Evidence logs include
`/tmp/dali-colour-service-controls`, `/tmp/dali-colour-service-restored-gate`,
`/tmp/dali-colour-paced-controls` and `/tmp/dali-colour-paced-restored-gate`.

See [colour operations](dali-colour-contract.md#owned-service-operations) and the
[slice specification](superpowers/specs/2026-10-22-dali-v2-p4d-colour-service.md).

## P4d4 — automatic activation and combined colour/base operations

The typed service now accepts both confirmed manual and automatic activation.
`apply_colour_with_output` retains one owner, deadline and final acceptance for
both intents. All colour preflight precedes either mutation; off commands precede
colour, while positive base commands follow the complete colour plan. Pre-write
actual evidence is cleared before the first mutation. Success requires actual
base plus requested-representation colour readback. Partial effects remain
Uncertain, with no retry/rollback or persistent gear-setting change. Command
order does not promise darkness or completed fades.

The independent emulator adds an optional automatic-activation flag and applies
staged colour through its existing activation routine for supported immediate
power commands, including DAPC MASK. Manual scenarios preserve their default
behavior and serialized shape; fades/colour scenes/settings remain unfinished.

Both design gates approved after clarifying OFF versus zero-DAPC fade semantics.
Four new tests failed behaviorally before implementation (feature bit, physical
automatic activation, service acceptance and combined master write). Eighteen
focused tests now pass, covering seven new tests plus prior colour/paced/native
event coverage; an async result type annotation was corrected before that run.
The existing cancelled-write/native-input test now uses the combined request.
All **8 new controls**, **5 affected colour controls** and **2 affected base
readback controls** fail at their intended behavioral assertions (53/34/16 s).
The restored gate passes **360 native tests**, strict scoped Clippy, Pi
library/all four binary compilation, changed-file format/whitespace, wait registry
and doctests (none defined). Both affected control tables have no stale anchors.
Both sequential read-only source gates approve frozen `3d6cc06` against
`80f358b`, with no material findings. Reviews were same-family/source-only and
ran no tests, builds or controls. Physical fading and firmware behavior remain
unverified. Evidence logs are in
`/tmp/dali-combined-colour-controls` and `/tmp/dali-combined-colour-restored-gate`.

The manual-only policy control is intentionally replaced by unknown-feature
rejection and known-enabled acceptance controls. A moved base-readback control
anchor was also repaired; its intended actual-versus-echo assertion is retained.
MQTT scalar/profile choice is pending and does not block this native increment.
See the [slice design](superpowers/specs/2026-10-22-dali-v2-p4d-combined-colour.md).

## P4d5 — durable public colour representation

The approved one-family-per-fixture choice is now saved as optional
`color_representation` (`temperature`, `xy`, `rgbwaf`). The metadata service and
MQTT `set_color_representation` command use existing durable activation, original
output-token admission and single command completion. Selection changes preserve
physical Origin/name/address and consumer object ID, rotate the output binding
generation and retire old contract intent/observations. Repeating a value is a
no-op. Groups and unrelated outputs retain their bindings. No gear settings or
UART recovery behavior change. Scalar MQTT colour and supported-format State
remain the next boundary.

Six initial focused tests executed before production edits: four failed on
rejected valid selection/continuity or unknown command, while two malformed-input
checks passed. The completed nine-test selection set passes, including durable
save/reopen, original-token reply, stale request refusal, combined rename/format
change and illegal metadata in past history. All **8 new controls** detect their
declared behavioral fault; the omitted-projection control initially tripped the
generation consequence first, so assertion order was corrected and that row
rerun successfully. Two affected existing reply/input-type controls also remain
RED. Restored native suite: **369 passed**. Scoped strict Clippy, Pi library/all
four binary compilation, changed-file format/whitespace and doctests (none
defined) pass. Logs: `/tmp/dali-colour-selection-controls`,
`/tmp/dali-colour-selection-affected-controls` and
`/tmp/dali-colour-selection-restored-gate`.

Both sequential design reviews approved with no material blocker; the old
strict-reader compatibility clarification is incorporated. Updated readers
accept old files, while new-field documents need updated strict consumers.
Source reviews follow the frozen implementation. Same-family/source-only review
limits remain explicit. See the [selection specification](superpowers/specs/2026-10-22-dali-v2-p4d-colour-selection.md).

First source review found two concrete validation gaps: externally tagged enum
objects could pass the promised string-only setting, and current input/instance
metadata could be ignored by the independent consumer. Both were reproduced
behaviorally and fixed with strict shared-string parsing and two current-record
guards. Ten selection tests and all **370 native tests** pass after correction;
all **11 selection controls** detect their intended behavioral faults. Scoped
strict Clippy, Pi compile, format/whitespace and doctests pass again. One new
injector required a locked-API correction; its compile failure was not evidence.
Logs: `/tmp/dali-colour-selection-review-controls` and
`/tmp/dali-colour-selection-review-restored-gate`. Corrected frozen-source
acceptance follows; no UART/SDK/physical setting behavior changed.

Corrected frozen `1a2a472` is approved by both sequential source gates against
`dfd10de`, with no remaining material findings. Reviewers inspected source/tests/
controls only and ran no verification; same-model-family limitation applies.

## P4d6 — selected-family scalar MQTT colour

Saved individual DT8 outputs now accept Kelvin temperature, scalar xy and
normalized RGBWAF channels for their one configured public family. Requests can
combine colour with PowerState/Brightness through the existing owned combined
operation; all validation/preflight precedes mutation. Colour-only requests leave
master power/brightness unchanged. Partial xy explicitly masks its other setter,
and RGB subsets preserve other channels. No new SDK, gear settings or UART
recovery policy was introduced.

State reports actual selected-family values, separate availability/quality,
active format and nullable supported-format/channel/limit capabilities. Inactive,
invalid, missing or stale colour values are explicit nulls, including in actual
merged MQTT State; base availability remains independent. Initial configured
reads discover colour immediately. Budgets are 2 s plain, 6 s colour read/base
write and 8 s colour/combined write, inside unchanged 15 s All/20 s SDK limits.

Three initial MQTT/emulator tests failed behaviorally before enabling the path.
All twelve focused cases now pass, including clamping, original traces, partial
channels/coordinates, malformed requests/evidence, native assignment order,
invalidation and retained-family retirement through reconnect. A test sent an
invalid and valid request in one coalescing window; actual broker errors proved
the expected whole-batch refusal, so it now waits for rejection before testing
the next valid scalar. No runtime workaround was needed.

All **13 new controls** and **3 affected old timing controls** detect their
declared faults. The paced six-channel+Tc MQTT write proves 8 s success and 6 s
failure; first configured reads also detect 2 s regression. Restored native
suite: **382 passed**. Scoped strict Clippy, Pi library/all four binary compile,
changed-file format/whitespace, wait registry and doctests (none defined) pass.
Logs: `/tmp/dali-mqtt-colour-controls` and `/tmp/dali-mqtt-colour-restored-gate`.
Both sequential design reviews approved; frozen source reviews follow. Review
coverage is same-family/source-only; physical firmware and live deployment remain
unverified. See the [MQTT guide](mqtt-outputs.md#colour-controls) and
[slice specification](superpowers/specs/2026-10-22-dali-v2-p4d-mqtt-colour.md).

Frozen `5e7dc2e` is approved by both sequential source gates against `96c818a`,
with no material findings. Both were read-only/source-only/same-family and ran
no verification. Physical hardware validation remains outstanding.

## Remaining phases

| Phase | Status | Next concrete boundary |
|---|---|---|
| P1 UART | Implemented and verified locally | Physical firmware transcripts remain a hardware-validation item; conservative unverified policy is the default |
| P2 ATX emulator | Implemented and verified locally | P4 capability extensions and physical validation remain explicit |
| P3 BridgeKit/service | Codec, lifecycle, inventory, persistence, fencing, managed service/UART and canonical daemon implemented | Capability paths; local shared dependency publication before portable clones/releases |
| P4 capabilities | Quiet recovery, base output codec/service/MQTT, named input metadata/live native events, DT8 codec/emulator/owned service and selected-family scalar MQTT with durable representation selection implemented | Input event configuration/feedback, remaining colour modes/settings, split-address component bindings and remaining explicit capability support |
| P5 commissioning | Addressed discovery/default adoption, one unaddressed output/input assignment, bus setup, removal and per-fixture group membership implemented | Full reassign/restore, identification, browser/pattern group workflows and native scenes |
| P6 authenticated HTTP | Session/status, command jobs, scalar observation/apply and native/job SSE accepted | Protected frontend integration and remaining P4/P5 actions |
| P7 browser UI | Pending | Approved responsive interface and browser acceptance |
| P8 Store integration | Pending | Scoped dev-track and persisted-data migration preserving bindings |
| P9 release/docs | Pending | Cross-build, deployment/rollback guides and hardware status matrix |

No v2 release, production installation, live Store migration, push or merge has
been performed. The legacy master checkout and v1 maintenance branch retain their
existing application source.
