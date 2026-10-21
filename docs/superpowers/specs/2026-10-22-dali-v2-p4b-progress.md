# P4b2: bounded request and helper progress under input traffic

2026-10-22. T3 continuation of the authorized capability engine. P4b1 is committed
as fd86341. This closes the named scheduling probes before physical fixture jobs;
it does not enable a new MQTT command or change the approved architecture.

## Grounding and hypotheses

Transactions::drive currently biases the event receiver before the request queue.
Hat::changed similarly favors public events before a helper's join. Cooperative
budget exhaustion yields the whole task, so it does not establish fairness between
these branches. P4a proved the same issue for a lower-priority recovery timer and
added an independent clock check; dispatch/helper collection still need probes.

Hypothesis one: a continuously ready input backlog can prevent queued requests
from being processed/retired, even after their caller cancels. Hypothesis two:
it can prevent collecting a completed HAT helper. Do not change either path until
a deterministic behavioral regression reproduces it.

Existing event priority has a purpose: consume queued Boot/EOF/collision/power
observations before interpreting query results or Closed status. Preserve it with
a bounded prefix of public records rather than requiring the queue to become
permanently empty. Do not discard events, lose a completed panic, or release a
resource based only on a task's is_finished hint.

## Proposed implementation after confirming the probes

For Transactions dispatch, prioritize stop, then queued requests, then idle input.
Every ordinary command already drains/classifies old events before admission,
fences input before writing, and checks its deadline. Explicit resynchronization
only receives until its guarded boundary. These existing protections must still
run; priority is not permission to skip them. A full bounded backlog may return
Busy instead of transmitting. The progress requirement is completion/retirement,
not guaranteed transmission on an arbitrarily saturated bus.

For Hat, consume a finite prefix of its public queue at the start of each changed
turn, only while not closing; stop immediately if an event starts closing. Bound
it by at most monitor capacity + 1 receive attempts, with every Lagged consuming
one attempt. This prevents a delayed EOF from canceling an owned Close helper or
an intentional Stopped record from becoming a false failure. Keep
the monitor capacity as an internal shared constant rather than copying another
unrelated bound. Dispatch every retained record through the existing event method.
Then check for a completed helper and actually await its owned JoinHandle before
returning a snapshot or waiting again. A pending helper remains stored. Reuse the
current result processing so discard flags still inspect panic results. No await
may occur between consuming a ready join result and retiring its stored handle;
canceling the containing select must never lose or re-await that result.

Preserve the normal event-first select for waits after this bounded preparation;
a completed helper must be collected on the next turn despite renewed traffic.
Retain the original shutdown deadlines, partial-write poison handoff, fresh-Boot
boundary and paced retries. Existing proactive bus observations must keep priority
over an older completed query snapshot, via the prefix drain and fill_missing.

A Closed query/status may become visible before its public closure record on
another runtime thread. Probe this neighboring race while changing collection.
If confirmed, initiate owned close/join instead of immediately calling it an
unexplained fatal exit. The actual transaction join result must still surface actor
panic/reader failure; ordinary transport EOF/I/O may retry only after teardown.
Do not turn a panic into a silent reconnect or trust one public status flag as
proof that the backend has stopped. Keep this correction scoped to confirmed
failure ordering, with the existing normal-EOF and helper-panic tests retained.

## Files, tests and acceptance

Primary files: src/atx/transactions/engine.rs and tests there; src/service/hat.rs
and hat/tests.rs. Monitor capacity may be named in src/atx/monitor.rs. Add only the
minimal cfg(test) fixture support needed to control real actor/owner scheduling;
no production environment knob, alternate actor implementation or fake success.

- A public transaction request is already queued before the actual actor is polled
  with a continually refilled event backlog. Require it to complete/refuse and
  retire its admission permit, instead of being stranded behind input. If refusing
  Busy, do not claim a UART write occurred. Keep stop and normal queries usable.
- A real owned helper is proven complete before HAT turns run with renewed public
  input. Require actual join/result collection under a finite number of turns,
  including a completed panic, and preserve queued newer bus/Boot/EOF evidence.
- Exercise a Closed helper result before public EOF delivery. Require ordinary
  transport teardown/retry while actual task panic remains fatal. Use the real
  transaction ownership/join path; a synthetic successful join proves nothing.
- Deliver delayed EOF after owned Close has already started; require the same
  Close helper to remain owned and joined successfully.
- Keep all prior UART, quiet-recovery, transaction, service and daemon regressions.
  Add isolated guard-removal controls with intended domain failures. Update any
  control anchors whose actual mechanism changes; do not weaken their oracles.

Run RED before each correction, targeted and full native tests, wait registry,
scoped strict Clippy, Pi compilation, relevant controls and restored checks. Record
source reviews, actual gate results and unresolved physical validation in progress
and service documentation. Two sequential read-only reviewers cover the T3 change;
all exposed review models remain the same family.

## Parent review and scope

Parent source checks traced command pre-admission drain/fence, recovery expiry,
Hat cancel_query/discard/panic handling, owned helper close, fill_missing and the
post-join poison handoff. New ordering must preserve those safeguards together.
The follow-on physical service contract must separately hold the routing permit
until backend cancellation is actually settled; joining only its frontend helper
is insufficient. Original-generation checks, readback, target availability and
refresh traces through a bridge-owned queue remain that next contract's work.

No Store, broker, HAT, deployment, shared runtime or persisted-schema change is
part of this scheduling probe. Physical fixture control remains disabled.

## Implementation record

Both sequential design reviews approved after specifying that the bounded prefix
must skip/break while closing and count every Lagged receive attempt. The three
main behavioral probes failed before their corrections: admission stayed held,
a completed helper remained uncollected, and normal PTY EOF became fatal before
its public delivery. The first helper-collection implementation also needed to
return after progress with no hardware-value change; its replacement passes.
The canceled/expired caller fixture uses the real public call and actor run;
the EOF/actor-failure fixtures exercise real serial/transaction ownership.

Six new controls all failed for their intended assertions. Native 263, scoped
strict Clippy, Pi library/four binaries, wait registry and doctests passed.
All 30 existing transaction, UART-service and quiet-recovery controls remained
sensitive to their guard removals; the final restored native suite again passed
263 tests. Both sequential source reviews approved without material findings.
Reviewer models are from the same family. No physical behavior is claimed.

Review learning: preserve the existing closing guard when adding a second event
consumption site. Prefer actual task joins over a particular public notification's
arrival order when classifying transport failure. Neither a bounded queue nor
Tokio's cooperative yield makes a biased select fair between its branches.
