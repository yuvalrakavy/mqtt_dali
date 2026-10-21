# P1b transaction implementation boundary

Recovery amendment, 2026-10-22: follow the owner's
[quiet-period policy](2026-10-22-dali-v2-quiet-recovery.md) for explicit
resynchronization and managed startup/reopen. It replaces the general Boot-only
prerequisite below while preserving the known partial-write poison exception.

Refines the approved P1 transport contract after the P1a substrate review.

`Transactions::start(stream)` exclusively owns a private `Monitor` and one
dispatcher task; all handle clones share that task and its admission permit.
There is no second UART reader or exposed raw writer. The dispatcher continuously
drains its persistent receiver even when idle, so ordinary input events cannot
accumulate merely because no caller is active. A nonwaiting capacity-one permit
covers queued and active work; other requests get Busy. Canceled queued requests
write nothing. The later service job queue can retry Busy.

Before each write, request a fence from the existing reader. It polls buffered
reads until an explicit nonblocking Empty result, up to 4096 bytes and 100 ms
(continuous traffic returns Busy), publishes
those records, and returns the minimum eligible event sequence. An incomplete
old line excludes its eventual event too; a line already in discard mode has
already emitted its error and excludes no extra event. Drain preceding events
before writing. Retry Pending is not evidence of an empty buffer. The serial
adapter reports Empty only on WouldBlock; the in-memory test adapter disables
cooperative yielding when checking its buffer. At the first accepted write, take
a second sequence watermark including any incomplete line. Framing and that
write boundary execute under the same split-stream lock, so the reader cannot
publish bytes between the snapshot and write acceptance. A response must satisfy
both boundaries. These boundaries filter
terminal eligibility only: boot, corruption, lag, closure and interference remain
safety events even below the response watermark. Already-ready safety events
take priority over polling the write. Bytes still arriving
in the kernel between boundaries cannot be physically timestamped by this API.

The dispatcher owns the request and permit after admission; caller cancellation
is observed through its response sender. Cancellation while Awaiting leaves a
late-reply tombstone. Cancellation during Writing
requires a subsequent boot after cancellation; an already buffered boot is not
sufficient. Monitor's write guard prevents further writes after partial writes.
No response is returned until the full write succeeds. While Writing, select on
both the write and received events so a boot cancels the write immediately.
Persist full-write success and each consumed terminal before another suspension.
Drop the write future before stamping its cancellation watermark. Boot recovery
then clears both engine uncertainty and monitor write poison under exclusive
ownership, enters Uninitialized, and requires a new explicit version exchange.
The single admission deadline covers fence, drain, write and response waits.
Fence/drain work also has finite byte/event budgets under continuous traffic.

Boot/receive corruption/lag/interference invalidate initialization. A harmless
timed-out or canceled fully written exchange may retire on its own eligible late
terminal. Observable interference or lost ordering requires a later boot; a
terminal alone never repairs it. Initialization is an explicit version exchange;
V is never used to clear a late-reply tombstone. Recovery after known partial
write cancellation requires a boot sequence later than the cancellation watermark.

Correlated h/l/m use one J/N/X/Z terminal; status d returns an observation; v
returns version. j is rejected (J can be its echo). t is rejected by default;
an explicit policy may declare one or two terminal records for a verified HAT
profile. Two-terminal mode consumes both before completing, including tombstone
retirement, so a repeated transaction cannot leak its second terminal into the
next transaction. No firmware policy is inferred solely from a version label.

For observable same-bus interference, H is a command; L has bit16 set for
commands and clear for DALI-2 events. Event payload decoding belongs to P4. M is
treated conservatively as a forward command. Equal-payload monitored copies of
the sent frame are permitted as echoes; they do not complete the exchange.
An unattributed blank-line collision poisons correlation, rather than claiming
that it belongs to bus zero. Same-bus unsolicited J/N ambiguity remains an ATX
wire limitation, as documented in P1.

The internal subscriber is the existing bounded broadcast receiver. RecvError::
Lagged is checked before any buffered packet is eligible, and before each new
write. On lag, close admission until reset; public event consumers remain
independent. There is no internal unbounded packet forwarding queue.

Tests must demonstrate: buffered and split-old-line terminals; async L events;
other-bus terminals; echoes; failures/cancellation during partial writes; boot
during partial writes; late-response retirement; interference→external terminal
→late original terminal cannot allow a new write; j refusal; two-terminal t
completion and timeout; explicit version initialization; Busy writes nothing;
lag/malformed/blank-line failures; close and actual PTY command/event exchange.
