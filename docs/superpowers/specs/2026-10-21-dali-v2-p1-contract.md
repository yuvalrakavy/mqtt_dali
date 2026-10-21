# P1 UART transport contract

Recovery amendment, 2026-10-22: the owner's
[quiet-period policy](2026-10-22-dali-v2-quiet-recovery.md) supersedes general
Boot-only recovery below. Complete-write/receive uncertainty can be retired by
explicit measured quiet recovery, followed by initialization. Known partial
write poison remains Boot-only. No persistent UART journal is used.

Implements the already approved transport architecture, independently of the
legacy `dali_atx` path. The v2 library owns the new modules; P3 connects them to
the canonical daemon. P2 will exercise the same serial path through a PTY.

Implementation split: **P1a** is the typed wire layer, continuous monitor and raw
serial/PTY adapter. Its `write` receipt reports only bytes accepted by the kernel;
it exposes all received traffic without consuming responses. **P1b**, still
pending, adds the receive fence, transaction state machine and firmware policy
below. P1a is not evidence that command acknowledgement matching is complete.

Source: [ATX I2/I4 manual, pages 6–9](https://atxled.com/pdf/AL-DALI-HAT-I.pdf),
retrieved 2026-10-21. The manual labels firmware v14 and also describes M frames
for firmware 18+. Its version-field descriptions disagree in places. No actual
I4 transcript or firmware compatibility test has yet been obtained.

## Framing and types

- 19200 baud, 8N1, raw Unix serial, no flow control. One nonblocking serial
  implementation serves the Pi UART and Mac/Linux PTY; no GPIO dependency.
- A single reader lives for the whole connected link and parses newline-delimited
  ASCII, accepting LF and CRLF. Preserve bus index and frame width. Bus 0 has no
  prefix; bus 1–3 use their digit prefix. Accept explicit 0 when reading.
- H/J/L/M are monitored 16/8/24/25-bit frames, respectively. B/V carry separate
  boot/version fields; preserve firmware text rather than interpreting decimal
  labels as hex versions. D carries bus status and reported channel count. P
  carries raw power/switch data. X/Z collision records retain their optional
  code. N is the terminal no-backward-response indication, not a successful
  device acknowledgement. An empty line is an unattributed receive collision
  per the manual, never implicitly assigned to a pending bus.
- Invalid lengths, invalid ASCII/hex and unknown headers become explicit
  diagnostics. Bound line storage to 64 bytes and partial-line lifetime to 1 s;
  discard only that malformed line through its next newline, never idle traffic.

## Command ownership and matching

- One pending UART command globally, even across buses. Queue admission is
  bounded and nonblocking; overload is an explicit error. Commands carry a total
  response deadline; a stalled write also has a deadline. No automatic retry in
  this layer: callers decide whether the operation is safe to retry.
- Typed outgoing packets: h/t (16 bit, single/repeat), j (8 bit), l (24 bit),
  m (25 bit). Encode exact width with uppercase hex payload and lowercase header.
  v requests version. d requests a bus-status observation.
- For single h/l/m packets, same-bus J/N/X/Z are eligible terminal records.
  H/L/M, other buses, D/P, and unrelated V never complete them. All monitored
  traffic, including a J used as a response, remains visible to subscribers.
  A successful send with N means the HAT finished waiting for a backward frame;
  the service must use a subsequent query to confirm observed device state.
- A status query returns the first same-bus D observation; this is a status
  snapshot, not proof that this particular query caused it (D is also proactive).
  A version query requires V; boot B invalidates an outstanding command and is
  also delivered as an event.
- If a different 16/24/25-bit command is observed on the same bus during an
  exchange, return an explicit interference result and poison admission until a
  trusted boot/reset boundary. No terminal record clears this state: it could
  belong to the external command. L input events must be classified separately
  from 24-bit commands before interference detection relies on that width.
  A monitor echo equal to the sent packet is permitted, but never an acknowledgement.
- j and t can be encoded but correlated execution is refused until an explicit
  firmware policy establishes echoed-J handling and repeated-command terminal
  cardinality. An unverified policy must not silently become a default.
- On cancellation/expiry before any write, skip the queued request. On caller
  cancellation after a complete write or a response timeout, retain the
  outstanding slot until its eligible late terminal arrives. Reject subsequent
  submissions with Desynchronized meanwhile. Do not let a late J/N satisfy a
  newer command. A boot record can retire the slot; a V alone is not a safe
  ordering barrier. A stuck slot is visible as link degradation, not an infinite
  caller wait. Reconnection policy belongs to P3 and must not silently reset
  this ambiguity while the HAT might still have an outstanding command.
- Deadlines begin at admission. Desynchronization rejects already queued as well
  as new requests. Partial/failed/canceled writes poison admission. Boot during
  a partial write cancels it; remaining bytes must not resume after reboot.
- Before writing, consume prior decoded traffic and establish a reader-order
  fence, including partial old lines. Track Queued/Writing/Awaiting states;
  no apparent terminal can report success until the full write completes.
  Bytes already in the kernel cannot be timestamped reliably by this UART API;
  this remaining attribution limitation must be stated and tested where visible.
- Framing corruption during an exchange poisons matching because it may have
  hidden a terminal or an external command. Internal packet overflow closes
  admission out of band before any buffered terminal can restore it.
- The ATX wire has no transaction identifier. An unsolicited J/N on the same
  bus can be indistinguishable from a backward response, and equal-payload
  external commands can be indistinguishable from echoes. Detect observable
  interference; document this multi-master limitation rather than claiming
  perfect attribution. Physical validation is required before asserting support
  for firmware-specific response ordering or echo behavior.

## Tasks, bounds and events

- Reader and dispatcher/writer are separate supervised tasks. The reader never
  waits for request handling or event subscribers. Bounded internal packet queue
  overflow fails the link explicitly instead of silently dropping packets.
- Public events use a bounded broadcast buffer. Lag is explicit (`Lagged`), and
  consumers must invalidate inferred state/resynchronize after it. This mechanism
  does not promise durable event replay.
- All writes, responses, admission and worker teardown have specified deadlines
  or nonwaiting paths, recorded in the wait registry. Dropping the final handle
  ends the link; cancellation/close aborts and joins the async reader without
  waiting on a synchronous UART syscall.
- Boot rejects active and queued requests and requires explicit initialization
  before admission resumes. Reopening an fd alone never clears a known poisoned
  session. Open/configure runs off async workers with a bounded wait; a late
  result is dropped without spawning a reader.
- Per-bus DTR/DT8 sequence exclusion and operations spanning multiple UART
  exchanges belong to the service/capability scheduler in P4/P5. Transport
  serialization alone is not a multi-command DALI lock.

## Required tests

Independent hand-written wire vectors for all widths/buses and version decimal
text; chunked/multiple/CRLF lines; malformed/oversized/trickling line recovery;
idle L delivery; interleaved other-bus records, monitor echoes, D and L around
J/N; X/Z propagation; write/response deadlines; canceled callers and late replies;
observable external-master interference; boot during exchange; bounded queue
overflow/explicit subscriber lag; one reader, supervised EOF/failure, clean close;
Unix PTY raw mode and end-to-end command/event traffic. No live HAT or broker.
