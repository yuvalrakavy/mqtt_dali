# P4a: measured UART quiet-period recovery

Tier T3. The owner approved quiet-period recovery on 2026-10-22 and explicitly
chose dropping the proposed persistent UART journal. This supersedes the
unimplemented journal/clean-receipt proposal and the earlier requirement that
every lost/unclean transport session await Boot.

## Grounding and operational contract

The legacy bridge drains input before commands. Its requested 10 ms timeout is
rounded down by termios VTIME to zero, so it does not currently measure 10 ms of
silence. V2 already has one nonblocking 19200/8N1 reader using Tokio AsyncFd;
the reader continuously parses and publishes ATX lines. Preserve that ownership.

Use a measured quiet interval during initial startup, reopen and explicit
recovery after an unresolved fully written exchange or lost receive ordering.
Default 250 ms, configurable with --uart-quiet-ms (positive bounded integer,
1..5000 ms). This is an operational recovery policy, not a manufacturer-proven
maximum HAT execution latency or a UART transaction identifier. Normal completed
transactions do not pay a quiet interval before every command. Interleaved input
events remain visible during acquisition. No persistent UART state is created.

The manual documents 100 ms response/repeat timing and a 200 ms DTR holdoff, but
not a universal upper bound for a HAT command waiting on a busy DALI bus. Do not
claim silence proves physical success or perfect response attribution. Existing
same-bus J/N ambiguity remains. Confirm practical timing with a real I4 before
a hardware-ready release; emulator acceptance is software evidence only.

## Reader and actor changes

Track the time of every actual nonempty receive inside ReadMode, including raw
partial/malformed bytes and fence drains. A watch notification carries this
activity to the transaction actor's quiet acquisition; there is no second UART
reader, buffer flush or unbounded queue. Start the timer no earlier than the
recovery request. Restart it on any raw input. At its expiry ask the sole reader
for a bounded receive fence and recheck activity; newly available bytes restart
the interval. Use one original absolute request deadline through all stages.

The quiet request's total acquisition budget is the configured interval plus
the existing setup allowance, so a 5000 ms interval can actually complete. V/D
initialization is a separate subsequent owner job with its own normal budget.

Framing reset is a guarded reader operation, not a delayed unqualified flag.
Carry the original deadline, a unique recovery/cancellation guard and the
expected raw-activity generation into the reader request. At explicit Empty,
validate and authorize reset under the split-stream lock before clearing stale
framing. Cancellation before authorization, expiry or changed raw generation
prevents that mutation. No suspension occurs between authorization and reset;
later parsing/writing uses the same lock. A delayed old request cannot clear
the guard or framing of a newer recovery. Caller Drop synchronously sets a
shared cancellation flag; reader authorization checks it under a tiny mutex held
through the synchronous framing reset. This does not depend on the actor being
polled. The request-local phase additionally protects cancellation of the direct
monitor future. No holder awaits or performs I/O with the cancellation mutex;
only the reader acquires it under the split-stream lock. Test a queued reset
after public caller cancellation while the actor future remains unpolled.

Recovery is an exclusive actor request using the existing capacity-one permit.
While waiting, the actor continues consuming its event receiver, so public
switch traffic is not swallowed by a command receive loop. Only successful
quiet acquisition may retire an old complete-write tombstone/receive uncertainty,
reset stale host framing and enter Uninitialized. A new explicit V exchange and
bus-status queries follow; V itself never acts as the ordering barrier. Caller
cancellation, continuous traffic, EOF, expired deadline or teardown cannot
publish successful recovery. New writes still require initialized actor state. The event loop checks the
absolute deadline synchronously before each select; ready event traffic must not
starve the lower-priority quiet timer, even if its caller is no longer polled.
Cover this with a continuously refilled event backlog and a live reply channel.

The reader returns its sealed packet sequence and raw-activity generation.
The actor consumes/classifies internal events through that boundary before
retiring old receive uncertainty, then checks for newer activity/loss, closure
and write poison. New activity invalidates the candidate and retries quiet
acquisition within the same deadline. Recheck caller cancellation, stop and
deadline immediately before committing Uninitialized; failure leaves recovery
required. Events arriving after that commit remain queued safety evidence and
must be consumed by ordinary request drain/fence before any initialization
write. Old queued decode/lag errors must not masquerade as new loss, and newer
loss must not be erased by resetting old state.

Known indeterminate/partial UART writes retain the existing write-poison latch:
quiet reception cannot establish what bytes remain in the HAT's command parser.
These require a later accepted Boot to rearm. Preserve this distinction across
in-process reopen. Snapshot authoritative write-poison state after query/actor
teardown: EOF can precede a cancellation-induced WriteIndeterminate event, and
Hat suppresses public events while closing. Public status may already say Closed
and is insufficient. Test EOF during a partial write and cancellation, then prove
the reopened actor remains Boot-only. No persistent journal is added to infer it across process
death. Document this limitation and avoid claiming a generic software reset.

Add an explicit Resynchronizing state for in-progress recovery. Monitor must
support discarding only stale incomplete host framing at the established quiet
boundary; complete records remain published. Reset must run under the same
split-stream lock as parsing/writing. New raw activity during the boundary
invalidates its candidate interval. Preserve current first-write sequence fences.

## Resource lifetime

Keep the useful actual-join and panic-cleanup fixes from the preceding
investigation. Remove journal-only clean receipts, lifetime Clean eligibility,
and final Clean-seal machinery: no consumer remains for those concepts. Public
close stays Result<(), Error>. Stop closes admission; the actor owns monitor
force-close and actual reader join even if its close caller cancels. A lifecycle
join supervisor also owns bounded failure cleanup after actor panic, drops its
captured monitor before publishing completion, and has no caller ownership cycle.
All cleanup stages use the first original absolute deadline.

## Managed owner and options

Hat schedules quiet acquisition on startup/reopen and when a timed-out complete
exchange cannot retire normally. Keep metadata available while hardware is busy.
Busy/expired quiet acquisition retries on the existing paced timer; it cannot
spawn accumulated workers or block MQTT/configuration. Normal asynchronous input
frames do not cancel healthy polling or require recovery by themselves.

Thread the configured interval through the service builder and canonical daemon.
Validate its range before any resources are opened. Offline --check-config and
--uart none remain independent of hardware state. Keep standalone PTY tests on
private fixtures; no live UART/broker or user home state is accessed.

## Verification and documentation

Use behavioral RED before fixes: delayed input restarts a real timer; buffered
complete/partial stale replies cannot complete the first new request; quiet
startup does not require Boot; a timed-out exchange recovers without Boot after
the interval; ongoing input stays observable and acquisition expires; canceled
quiet acquisition does not admit writes; known partial-write poison cannot be
cleared by silence; retained frontend handles/canceled close callers/actor panic
do not retain real transports. Cover PTY and canonical process startup/restart.

Update old Boot-only recovery tests and guard-removal controls to assert the
new authorized policy, retaining their safety intent. Run full native tests,
scoped strict Clippy, wait-registry check, Pi compilation, intended behavioral
negative controls and restored checks. Two sequential same-family design/source
reviews cover this replacement; earlier receipt reviews do not approve it.

Update running-v2, support/progress and UART design records. Explain the quiet
setting, bounded busy/retry behavior, continued event delivery, partial-write
exception and physical-validation limits. No Store changes, deployment, upstream
publication, or journal integration belongs to this slice.

## Self-review

The changed operational policy comes from the owner's explicit reply, rather
than being inferred from UART silence. Two parent passes checked timing vs
termios rounding, preservation of asynchronous inputs, raw-byte vs decoded-frame
activity, timer/fence races, cancellation and one absolute deadline, existing
write poison, caller-independent resource release, and unverified physical
latency. Source: src/dali_atx.rs, src/hat_line.rs, src/atx/{serial,monitor,
transactions/engine}.rs, src/service/hat.rs and the
[ATX I2/I4 manual](https://atxled.com/pdf/AL-DALI-HAT-I.pdf), pages 6–9.

Two sequential same-family read-only design reviews approved implementation
after the guarded reset, actor boundary and quiet acquisition budget refinements.
The second highlighted authoritative poison handoff after teardown, now recorded
above. Two sequential read-only source reviews approved the final cancellation,
raw-activity, actual-join and deadline-starvation corrections. Behavioral RED and
final local verification are recorded in docs/v2-progress.md; physical validation
remains outstanding. Reviewers are from the same model family.
