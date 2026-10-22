# P4c4: current input values through the owned UART service

## Problem and chosen shape

The approved bus UI must show the current state of common inputs before a user
operates them. Today instance refresh projects native-event cache only
(`service/proxy.rs::read_scope`); a quiet or event-disabled instance remains
unobserved. Native events also carry at most ten data bits. A real hardware query
is required, rather than inferring released/off from silence.

T3: a wire contract extension and the shared physical-operation lifecycle.
Standing implementation approval applies. Reuse that lifecycle for output and
input work using a concrete typed enum. A separate input lifecycle would duplicate
settlement/cancellation rules; direct frontend transaction calls would lose owner
lifetime. Neither is needed. No UART recovery, outage policy, retry, shared SDK,
physical input configuration, discovery or Store/UI implementation changes.

Implement one vertical slice: exact pure value decoding, independent emulator
queries, owned service read, and MQTT refresh/aggregate projection. The existing
single permit, helper and settlement ceiling serve both physical operation kinds.

## Protocol and value model

Query saved individual input module/instance addresses with 24-bit commands:
instance type (80), resolution (81), input value (8C), then input value latch (8D)
for the remaining bytes. Confirm reported type equals saved type before claiming
a typed state. Resolution is 1..255 bits: at most 32 wire bytes and 34 queries.
All bytes must answer without collision; a missing/interrupted value is unknown,
never a zero or partly accumulated measurement. Use one original deadline and
captured transport/bus stamp for the whole sequence.

`dali::input::Resolution` validates nonzero u8 and exposes byte count.
`InputValue::decode(resolution, wire_bytes)` requires exactly ceil(bits/8) bytes,
strips low repeated padding bits from the MSB-first sequence, and keeps canonical
big-endian bytes (fixed ceil(bits/8) width). Expose exact uppercase hex, optional
u64 conversion when it fits, and approximate normalized fraction for absolute
inputs. No bigint/JSON integer precision loss: the exact value is always hex.
Reject resolution zero and truncated/extra bytes. Do not reject valid FF bytes as
MASK or treat the first byte as the whole value.

Sources: [general instance variables](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/6113524875.html)
define 1..255-bit resolution; [full value queries](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/5634238347.html)
describe value+latch (that PLC API separately limits its LWORD to 64 bits).
The pinned python-dali `query_input_value` sequence at commit
`4b21c18e79d02d2a97f91d135f0eee450e8c2fb9` provides the MSB-first/low-padding
reference. Literal examples: one-bit FF -> 01; two-bit 55/AA/FF -> 01/02/03;
ten-bit ABC0 -> 02AF. A 255-bit value needs all 32 bytes.

## Observation, native events and publication

Represent observation evidence explicitly as Event {frame,payload} or Query
{reported_type,resolution,value,quality,reason}; never manufacture a native Event
for a query. Query evidence has no EventInformation. The existing Notice stream
remains native-only, including all button gestures and original trace absence.

The input router keeps a per-route accepted native sequence. Capture it at query
admission. If that route accepts any newer native event before query completion,
keep the native observation rather than overwriting it with a latched read. This
is a conservative ordering rule, not a claim of a distributed atomic snapshot.
Queries can still answer when event delivery is disabled or no listener boundary
has completed. Generation, bus and UART acceptance checks remain mandatory.
Never retag a query result after rename/replacement. A valid query clears an old
event value; a subsequent native event clears old query-only metadata, preventing
an older full-resolution sample from appearing current alongside a new event.

Continue existing Available/Quality and type-specific event properties. Add
nullable `ValueSource` (event/query), `ValueResolution` (bits), `InputValueRaw`
(canonical exact hex). Query-specific type projection:

- Part 301: Status Boolean only for resolution 1 and canonical 0/1.
- Part 302: Position normalized 0..1; InputState Boolean only at resolution 1.
- Part 303: Occupied/Movement from canonical two-bit flags, only resolution 2.
- Part 304: exact raw value and resolution; no invented lux conversion.
- Unknown types: preserve raw diagnostic evidence; Available=false,
  Quality=unsupported, no invented semantic property.

`PositionRaw` and `LightLevelRaw` retain their existing event-only ten-bit meaning;
query observations clear them with explicit null. Position/InputState also appear
as explicit null when an event lacks verified resolution. All new fields use null
on absent/stale evidence to clear BridgeKit merged State. Query quality is current
for a complete supported value, unsupported for unknown types or incompatible
type-specific resolution, type_mismatch for changed physical type, partial when
some metadata answered but value did not, unavailable when nothing answered.
Stale lifecycle takes precedence. Unavailable does not mean released.
For query evidence Available=true requires complete, supported, type-matched
current values; partial/unavailable/type_mismatch/unsupported queries are false.
Native action-only partial evidence keeps its existing Available=true semantics.

An explicit instance refresh now queries the hardware, then projects the latest
same-generation snapshot after its final await; a newer native event may be that
latest evidence. Preserve the original request context for query results and
authoritative absent trace for unsolicited native events. Any stale/mapping-changed
completion is refused. Nonreply/collision produces explicit unknown State; a
transport error also returns the existing service rejection and invalidates data.

Aggregate reads include enabled input instances in the existing fair cursor and
15 s ceiling, with a 4 s per-input budget selected before admission. Plain output
and configured colour budgets remain unchanged. Explicit typed library reads
honor their caller deadline. The 4 s budget must pass a paced 255-bit literal peer
at 80 ms/reply; reduction to 2 s must fail the completion assertion. No deadline
renewal per latch. Startup inventory-only reads remain hardware-free.

## Implementation and verification

`service/physical.rs` owns a Work enum pairing each typed request/route, shared
Active/Stage and tagged completion/outcome. Preserve existing output API and
behavior; adapt `hat.rs` to common getters/dispatch without a new queue or timer.
The owner accepts typed completion into the existing snapshots and releases the
same permit only after settlement/final validation. Input reads share shutdown,
late-reply, bus stamp and generation handling with tested output work.

`service/input/readback.rs` owns input resolve/query/request/accept helpers;
`service/input.rs` owns event/query projection and sequence ordering. Add pure
codec types under `dali/input`. Emulator uses an independent per-instance optional
profile (resolution and raw wire bytes), preserving existing numeric instance-type
scenario lists. Add 24-bit short/module/instance query decoding and a latch cursor;
device count/type queries answer independently of output address space. Scenario
validation bounds profiles, duplicate instance numbers and byte counts. No shared
production decoder in physical model. Raw event injection does not silently alter
physical query values: a private explicit control changes the input's wire sample.

Test first: literal 1/2/10/16/64/255-bit values, extra/missing bytes and zero
resolution; independent emulator commands and latch restart/collision behavior;
real service/MQTT refresh before any event; button/contact/occupancy/relative-light
projection; wide raw preservation and paced deadline; no synthetic actions;
native event during query wins; missing/collision/type mismatch clears old state;
generation/cancellation/native traffic/permit ownership across an input query;
aggregate cursor includes inputs and rechecks latest snapshot after awaits.
Keep existing native-event tests meaningful after target refresh starts querying.

Run behavioral guard-removal controls for the risky boundaries, then full native
tests, scoped strict Clippy, Pi compile, changed-file format/whitespace, wait
registry and doctests. Update input/emulator/service/operator docs, support matrix,
contributor catalog and progress; source gates review a frozen SHA. Same-family
review and lack of physical HAT/input certification remain explicit limitations.

## Self-review and gate verdict

Round 1 checked actual cache-only refresh, physical ownership and the pure latch
reference. Round 2 checked sibling output paths, aggregate read timing, native
ordering, mutable mappings, merged-null behavior, disabled event streams and
255-bit bounds. Exact raw bytes avoid a new lossy JSON format; event-only fields
retain their existing meaning. No owner product-policy decision remains open.
Design gate: surely yes, two sequential read-only buildability/correctness lenses.

Both sequential design gates approved. Gate 1 requested an explicit distinction
between metadata-only query availability and native action-only partial evidence;
the rule above resolves it. Gate 2 found no material gap. Reviews were same-family
and read-only; neither ran tests or mutated the checkout.

Implementation inspection also identified that monitor sequence numbers restart
after reopening a transport. Reset per-route native sequence tracking when a new
stamp's receive floor is installed, and capture the comparison sequence at actual
physical execution admission after any preceding health/boundary job finishes.
The new transport regression distinguishes a genuine newer release from an old
high sequence counter. This preserves the selected conservative ordering rule.

## Implemented evidence

The quiet-button MQTT refresh failed before production changes. Seventeen new
tests now pass alongside the prior suite: **399 native tests passed**, scoped
strict Clippy/Pi compilation/format/whitespace/wait registry/doctests pass.
The 80 ms paced 255-bit peer completes within 4 s and detects a 2 s regression.
All **12 new controls and 8 affected existing controls** produce their intended
behavioral failures. The independent emulator latch test detects live-data
changes incorrectly altering an already captured sample. Exact wide values,
common types, native precedence, traces, merged-null clearing, missing/collision/
changed type, mapping/caller expiry, cancellation, final validation and shutdown
are covered. The source gates review the frozen implementation next; hardware
certification, input configuration and live systems remain outside this evidence.
