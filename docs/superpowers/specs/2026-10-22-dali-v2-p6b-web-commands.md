# P6b: authenticated configuration and commissioning jobs

## Problem, grounding and scope

The approved web workflow needs the existing saved metadata, discovery,
assignment and group operations. P6a protects access but has no mutation route.
Existing physical requests take up to 18 seconds; an HTTP handler has a five
second bound. A browser disconnect must not discard an admitted command result.
T3: retain the existing service authority and durable/physical failure semantics.
Standing architecture approval authorizes this increment; no product decision
is outstanding. Design/check budget: about ten minutes, implementation and
verification about 25–40 minutes with the warm cache.

Confirmed seams (current source):

* `service/proxy.rs::parse` implements strict command/target/parameter parsing;
  `validate_request` delegates to concrete routing/discovery/commissioning/group
  resolvers. `execute_with_context` obtains a CommandCompletion from the SDK
  worker. HTTP must not call that driver entry point or invent a completion.
* `ServiceHandle` already accepts original RequestContext and absolute Instant
  deadlines; its private submit/discover/assign_space/groups paths accept no
  MQTT completion. They keep the real shared operation permit through save and
  physical settlement. Receiver cancellation has established semantics.
* P6a middleware checks authentication/Origin/CSRF before JSON extraction, and
  the optional Server owns one task. JSON extraction can await a slow body, so
  mutation admission must recheck the session afterwards.
* Current serializable discovery, assignment and group reports expose public
  evidence and skip internal stamps/context. Assignment/group `reason` can
  originate in private save errors, so HTTP must replace that free text.

Selected shape: small bounded HTTP job records around the existing typed service
commands. Directly awaiting each command would collide with the handler bound
and lose result ownership on disconnect. A second durable job scheduler would
duplicate the service and add recovery semantics the owner did not request.
Jobs are process-local result tracking; they do not retry or journal UART work.
This slice exposes existing command operations only. Output apply/read, observed
values, native input events and the Preact screens follow separately. There is
no claim that all commissioning workflows or browser operations are finished.

## API contract

All routes use the existing session gate and security headers. Mutating POST
additionally requires exact Origin and CSRF. The strict JSON body (existing
8 KiB ceiling) of `POST /api/jobs` is:

```json
{
  "generation": "0123456789abcdef0123456789abcdef.1",
  "target": null,
  "command": "configure_bus",
  "params": {"expected_revision": 1, "bus": 0, "name": "Bus1", "enabled": true}
}
```

`generation`, `command` and object-valued `params` are required; target is null
or the complete current public target string. Unknown envelope/parameter fields,
unsupported commands, invalid types, retired/missing authority and stale
revisions refuse before creating a job (400 malformed/unsupported, 409 current
routing/revision conflict). Reuse the existing MQTT command grammar and target
names; no topic change or new name encoding. The client supplies original
generation; the server never replaces it with the latest token. Middleware must
insert an explicit RequestContext extension as well as instrument its handler;
the captured traceparent (including authoritative absence) passes into the
queued service request and operation span, never inferred from a later task span.

After body extraction, revalidate/record explicit user activity before pure
preparation and nonwaiting admission, with no intervening await. This also makes
revoked slow uploads return 401 before reporting stale/invalid command details.
Logout, expiry or rotation during upload returns 401 and creates no job. Work
already admitted may finish after logout/expiry; a newly authenticated session
can inspect its result. Passive inspection does not renew a session.

Successful admission returns 202 with a job record and `Location: /api/jobs/{id}`.
It means accepted for an attempt, not saved/programmed successfully. Admission
and execution can race other clients/MQTT; the existing service revalidates the
original generation/revision and retains its original single physical permit.
Busy/conflict/physical failure is recorded as a failed job, without automatic
retry. A lost HTTP response may still mean a command ran: list jobs and inspect
current inventory/hardware before repeating it. No idempotency guarantee.

`GET /api/jobs` returns `{ "jobs": [...] }`, oldest admission first, bounded to
32 records. `GET /api/jobs/{id}` returns a record or 404. All authenticated
sessions share one controller's records (one administrator password, no accounts).
Unknown/noncanonical IDs return 404. IDs are random 128-bit lowercase hex and
must be unique among retained records; they are locators, not credentials.

Records contain id, command, optional target, status (`running`, `succeeded`,
`failed`), result or null, and error or null. Success contains the existing
structured public report or `{ "revision": n }` for metadata/recovery. Success
means that the service returned a report: clients must inspect assignment/group
outcome/quality, including `reserved_but_unconfirmed`. Replace any report reason
with fixed public inspection guidance. Errors have a stable code (`busy`,
`closed`, `expired`, `refused`, `uncertain`, `result_unavailable`) and fixed safe
message; never return private error strings, filesystem paths or request bodies.
Sanitized results are at most 32 KiB, otherwise retain an explicit
result_unavailable failure instructing inspection. Never truncate a report.

Four jobs may be admitted concurrently, counting mailbox plus active futures;
excess returns 503 without waiting. This is a memory bound, not another bus
queue: concrete service operations still refuse Busy. Keep at most 32 records,
evicting the oldest completed record when necessary and never a running record.
No persistence, replay, retry, polling keepalive, per-job spawned task, cancel
endpoint or new configuration knobs. Clients poll jobs on the current screen;
SSE job notifications are unnecessary for this small first contract.

## Ownership and implementation

Add a crate-private PreparedCommand façade beside the existing parser. Extract
the current command admission match into one shared helper used by MQTT and HTTP.
Preparation parses once, captures typed operation/target/context and an original
absolute deadline: metadata/recovery/read_groups 2 s, discovery 10 s,
assignment/group mutation 18 s. Executing calls the existing ServiceHandle
internals with completion None. Keep MQTT driver dispatch/completion semantics
unchanged. No Reporter or publication bypass. HTTP results need not wait for a
broker; existing snapshot helpers publish saved changes through the SDK worker.

Add `web/jobs.rs`: bounded nonwaiting mailbox, semaphore, bounded record book
behind a short mutex, and a FuturesUnordered of at most four owned command
futures. A queued command keeps its captured deadline; scheduling never renews
it. The job loop and Axum serve future are polled inside the same Server task;
there are no detached job tasks. Normal stop drops command futures promptly,
closing their reply receivers and preserving existing service cancellation/save
uncertainty. Unexpected job-loop completion fails the server/managed service.
Unexpected server end drops the job loop. Outer abort synchronously drops both
futures; existing actual accepted-IO barrier still proves HTTP descriptor drop.
Keep all cleanup under the original shared deadline. Retained State/handle clones
own no running job future; after close admission must refuse.

State owns the job sender/book, but the receiver is moved into the Server task;
avoid a receiver/sender lifetime cycle. Mutexes never span awaits. Each active
future returns its id/result/permit; completion updates its record before the
permit is released. Admission inserts the record and try_sends under one short
book lock, rolling back on closed/full mailbox. The bounded FIFO tracks admission
order independently of random IDs; completion never changes order. Service Busy
is a job outcome, not an HTTP transport failure. Panic remains an owned server
failure collected by the existing lifecycle; do not swallow infrastructure panic.

Files: `src/service/proxy.rs`, `src/web/{jobs,handlers,server}.rs`, `src/web.rs`,
focused tests under `src/web/tests`, wait registry, HTTP guide, support/progress
and control table. No SDK/Cargo/UART/Store changes.

## Evidence and review

Write and run the first HTTP regressions before production changes. Use private
loopback sockets, FakeBroker and independent PTY emulator, never the real broker
or hardware. Verification and adversarial rows:

| Property | Seam / concrete test tool | Counterfactual |
|---|---|---|
| Authenticated metadata saves, original tokens, strict grammar | actual POST/GET sockets + managed service; compare disk and existing public model | missing route, rewritten generation or loose parameters fail assertions |
| Slow body cannot outlive authorization | partial socket upload; logout/expire before final body bytes, then inspect inventory/jobs | omit second session check, observe admission/save |
| HTTP closure/logout does not cancel admitted work | private socket admission, held save/physical request, close originating connection/session, inspect result after release | ownership tied to handler loses result |
| Actual addressed discovery and one input/output assignment | independent emulator + HTTP job, inspect physical and saved inventory | missing typed dispatch or fabricated success disagrees |
| Original deadline, service Busy/CAS, uncertainty | managed save hooks / held physical operation and captured context; job result plus disk/state | renewed deadline, skipped guard or echoed error detected |
| Trace context survives queued work, including absence | actual HTTP trace/headerless requests, captured PreparedCommand/service context | later task span or dropped extension changes the original context |
| Four active, 32 history, 32 KiB result, no live eviction | job book/permit seam and HTTP list, deliberately reversed random/admission ordering | cap/eviction/result guard removed is RED |
| Shutdown owns pending futures and private errors stay private | held service operation, retained web State, bounded close; synthetic private error report through same serializer | detached future/permit or exposed path detected |
| Polling has no session renewal, no auth bypass | real GET job/list, short injected Auth policy, direct URLs | reused P6a gate plus new route checks |

Add new behavioral controls per mechanism and plan-check existing tables; rerun
only reanchored/changed controls per current workflow. Batch tests/edits, restored
full native suite once with four processes, scoped strict Clippy, Pi four bins,
doctest/fmt/wait/diff. Freeze source then two sequential read-only gates, explicitly
same-family. Browser interaction acceptance remains P7; emulator agreement does
not certify I4/fixture firmware.

Self-review 1 opened actual proxy parsing/validation/dispatch, typed handle
admission and report serialization, Auth expiry and Axum ownership. It rejected
driver execute outside the SDK worker and per-job detached tasks. Self-review 2
checked stale original tokens after parsing/queueing, expiry during body await,
mailbox/record/active-future bounds, history ordering, closed sender/retained State,
partial report success versus physical success, private reason strings and shared
cleanup. Actual slow (>5 s) physical timing is verification during implementation;
if the existing emulator cannot pace it, test captured deadlines and disconnected
request ownership independently rather than claiming a slow-path measurement.

Both sequential design gates approve (77 and about 40 seconds, same-family,
source-only). The first gate's trace-extension detail is incorporated explicitly.
The fail-first metadata HTTP test returns 404 instead of 202 before production.
The companion slow-body test cannot yet reach extraction without a route; its
initial setup timeout is not counted as auth-guard evidence. Controls must prove
that guard after implementation. Early verification caught only a Rust future
type-inference annotation and a test's incorrect `programmed` spelling for the
existing `verified` group outcome. Twenty-eight focused web/facade tests pass
before the added draining-HTTP cancellation isolation.
