# P5b: assign one unaddressed output without changing existing assignments

## Problem, shape and scope

P5a discovers existing short addresses. A newly installed fixture without an
address cannot answer those probes. The approved Add unaddressed workflow must
give it an unused address while preserving saved/missing fixtures and already
addressed hardware. The legacy iterator and independent emulator already cover
Part 102 INITIALISE/RANDOMISE/search/PROGRAM/VERIFY. Reuse their established
protocol shape through the managed owner; do not route through the legacy driver.

Three shapes were considered: (1) program then save, which can leave an untracked
hardware assignment after a save failure; (2) a new durable commissioning job
database, which duplicates existing persistence for this one-device operation;
(3) save the new address reservation in the existing inventory before programming.
Use (3). Saved inventory already reserves missing hardware without claiming
presence. The reservation survives cancellation/restart; ordinary readback and
later restore/remove workflows resolve it. This is inventory, not a UART journal.
Do not add a background retry/recovery framework.

T3: persistent identity creation before physical address mutation. Existing mission
and implementation approval applies. Implement one fixture per bounded request,
matching P5a's incremental scan shape. The upcoming browser repeats the step and
can stop between fixtures. Full reassign, lost-address restoration and input-module
addressing remain separate following increments; no public all-devices mode in
this command. The full bus job/progress UI is not claimed here.

## Operation and public contract

Add `ServiceHandle::assign_next_output(bus, expected_revision, context, deadline)`
and WithReply controller command `assign_next_output` with exactly `bus` and
`expected_revision`. No target, destination address or addressing mode parameter.
Require Ready inventory, enabled bus 0..3, current controller generation and
revision at admission and before reservation. Same shared permit through search,
settlement, durable reservation, programming, verification and final settlement.
No gap where another physical/config operation can enter.

Return a report with outcome `assigned`, `reserved_but_unconfirmed`,
`no_unaddressed_output` or `no_free_address`, bus, optional address/target,
confirmed inventory revision and nullable reason.
Only `assigned` proves the programming verification completed; it does not claim
current light level or manufacture fixture State. Preserve original request
generation, trace and exactly one command reply across the reservation's runtime
generation change. Requests with malformed fields, stale authority or disabled
buses perform no addressing operation.

MQTT uses one 18 s deadline inside the existing 20 s SDK operation bound. Search
gets at most 10 s clipped to the original deadline, programming at most 4 s
clipped to its remainder; no per-query renewal. Save uses the original deadline.
Caller-supplied shorter bounds remain authoritative. Existing settlement keeps
its fixed cleanup ceiling and actual join/teardown ownership.

Repeated-command completion is currently intentionally Unverified in the canonical
daemon. Add `--uart-repeat-terminals unverified|1|2`, default `unverified`, parsed
before resources start. Map to existing RepeatPolicy; typed services already pass
that policy via with_uart. Unverified commissioning refuses before sending its
sequence, rather than failing halfway through INITIALISE. This selects a known
deployment/emulator profile, never infers a profile from firmware or time. Old
read/control behavior and UART quiet recovery remain unchanged. Physical policy
validation remains outstanding; examples explicitly identify emulator profiles.

## Search and destination selection

First consider unused inventory output addresses in ascending order; saved missing
addresses remain occupied. Query physical STATUS at each candidate. A valid byte
or receive collision occupies that slot even if it has no saved object; skip it.
Only a clean NoResponse permits using the address. Transmit collision, transport
failure, stamp loss or deadline refuses the operation. At most 64 slot probes;
if all are reserved/occupied, return no_free_address without address mutation.
Input addresses are independent and do not consume output slots.

Terminate prior output addressing mode; INITIALISE unaddressed (A5FF) and
RANDOMISE (A700) use the selected repeat policy. Await an actual async 100 ms
randomisation timer under the original search budget. Search the inclusive
24-bit range 0..FFFFFF by COMPARE and all three search-address bytes. Initial
COMPARE at FFFFFF establishes whether anything participates; clean NoResponse
returns no_unaddressed_output, with no saved reservation. Midpoint receive
collisions mean eligible candidates exist in that range; transmit collisions and
unexpected non-FF values fail. At exact selection a receive collision refuses
unique programming; the next explicit request may randomise again. No unbounded
collision retry loop.

After convergence, explicitly select all three bytes of the chosen random address,
require exact COMPARE YES and QUERY SHORT ADDRESS FF (still unaddressed). Keep
that candidate and its original UART/bus stamp as private temporary evidence.
No persistent physical identity is inferred from a random address. Bounds include
both 000000 and FFFFFF; no off-by-one sentinel outside 24 bits.

The [ATX manual](https://atxled.com/pdf/AL-DALI-HAT-I.pdf) documents `t` sending
the pair within 100 ms, without fixing all firmware terminal-count behavior.
The primary [python-dali commissioning sequence](https://github.com/sde1000/python-dali/blob/master/dali/sequences.py)
confirms unaddressed selection, the 100 ms randomisation wait, occupied-address
checks and post-program verification. Its command definitions supply the literal
Part 102 special frames. Existing emulator code is independent and already
models their effects; tests must also use literal peers and boundary vectors.

## Reservation and programming lifecycle

Search is one physical Work phase. After actual helper join/settlement, the owner
checks cancellation/deadline, original controller revision/generation and the
captured physical stamp. With a candidate it saves one fresh default-named output
at the free destination through ConfigStore and the shared accept_save handler.
Reuse P5a's first-unused LightN naming without manufacturing an Observation.
No old output, group, name or lifetime is replaced. The new inventory item is an
address reservation; it remains unknown until ordinary fixture evidence exists.

Pass the original CommandCompletion only to that reservation activation. There is
one durable activation per operation. The operation's reservation is confirmed
only after both durable save and runtime activation succeed. A disk commit with
failed activation keeps existing frozen/pending recovery and cannot program or
claim a confirmed reservation report. Confirmed noncommit cannot program hardware;
uncertain disk/activation stays frozen under existing explicit recovery. If a save
commits but the caller cancels, keep the reservation and skip further mutation.
If saving fails after entering addressing mode, best-effort termination below
still applies; no inferred rollback of hardware or configuration.

After a confirmed save, requeue the same owned request/permit as a programming
phase on the existing Hat owner, rather than awaiting a second hardware loop
inside the disk-save handler. Its internal route uses the new reserved output's
generation with the original trace; it never admits a public retired context.
Require the original search stamp at both enqueue and hardware start. A new
stamp must not accidentally refresh stale candidate authority. Metadata/physical
admission remains occupied throughout the phase handoff.

Before PROGRAM, recheck that the destination still has a clean NoResponse, select
the captured random address explicitly, require exact COMPARE YES and QUERY SHORT
ADDRESS FF. Then PROGRAM the reserved address, VERIFY SHORT ADDRESS YES and QUERY
SHORT ADDRESS equal to `(address<<1)|1`. Withdraw/Terminate the selection and
require a fresh valid STATUS at the new address before reporting assigned.
No level, group, scene or persistent light setting is changed. Other addressed
outputs and the input address space remain unchanged.

After a confirmed reservation, physical failure, caller cancellation observed
before program admission, or inability to continue returns a structured
`reserved_but_unconfirmed` report with the reserved target/address, committed
revision and reason. This is an ordinary WithReply value describing a completed
attempt, not a claim of assignment success; the UI must inspect outcome. If the
caller has gone it may lose that report, but inventory still contains the row.
Do not reduce this outcome to the existing string-only ServiceError mapping.
Disk/CAS uncertainty before confirmed reservation instead returns the existing
ServiceError::Uncertain, with routing frozen and no claim of a committed
reservation/revision; inspect/recover under the current contract. Never delete a
confirmed reservation or retry on another address. A transmitted program may
have taken effect. The
latest complete inventory remains the authority after a lost reply/restart.
Physical failure alone does not freeze durable routing. Only existing disk/CAS
uncertainty does. A later ordinary query is not proof of which physical fixture
an address represents; do not invent replacement continuity.

Terminate inside the active helper on normal assignment/no-candidate completion.
A helper detecting an error may also attempt it when the transaction owner is
ready and its original remaining budget permits. Never bypass unresolved
terminals or resynchronization. A successful search deliberately leaves mode
active for the disk/program handoff. If the owner abandons that candidate after
the search helper has joined (failed save, cancelled caller or shutdown), do not
create another helper solely for cleanup. Mode may remain until the standard
15-minute timer or the next operation's initial Terminate, as on crash/deadline/
link loss. This changes no short address by itself, does not prevent ordinary
light commands, and needs no power cycle, journal or teardown/retry subsystem.
Native input events keep using the continuous reader.

## Files, tests and acceptance

Add pure Part 102 commissioning frame/search helpers under dali and an owned
service commissioning module. Extend the concrete physical Work/Sample/Completion
with search/program phases and a captured-stamp requirement, using existing
helper lifecycle and permit transfer. Owner performs only the durable middle
step and requeues the continuation; proxy adds strict controller parsing and
original completion; daemon exposes the existing policy. No shared SDK changes.

Fail first through real MQTT + the independent PTY emulator: saved output 0,
physically addressed output 1 and a new unaddressed fixture. Assign the new fixture
to address 2, preserve 0/1, save a default name, receive one original-token/trace
reply and leave input module 0 intact. Repeat with no unaddressed fixture must
not change revision. Save/reopen and independent-consumer evidence must preserve
existing identities and expose the new reservation even after a lost response.

Adversarial coverage: every 24-bit search boundary/empty range; literal opcode and
repeat vectors; both repeat terminal profiles and unverified refusal before write;
all reserved/physically occupied slots; missing saved fixtures remain reserved;
receive versus transmit collision at range/exact selection; invalid reply bytes;
malformed/stale requests; cancellation/deadline before and during search/save/
program, native button events while held and permit retention across handoff;
save noncommit/uncertainty and bus/stamp loss while saving preventing PROGRAM;
post-program query failure retains the saved reservation without success/State;
managed stop joins/settles its active phase. Forced conflicts use literal peers,
not a new general fault framework. Tests touch no real broker/UART/fixtures.

Run meaningful mutation controls, affected ownership/save controls, restored
native suite, scoped strict Clippy, Pi compile, waits, format/whitespace and
doctests. Update operator/service/CLI/support/progress docs, then freeze and run
both source reviews. Record actual hardware and same-family limitations.

Self-review round 1 checked command selectors, existing repeat refusal, available
addresses and ConfigStore activation. Round 2 traced cancellation/uncertain save,
new-generation command completion, captured-stamp handoff, inclusive search bounds,
temporary mode cleanup and unknown current State. Both sequential design gates
approved before production edits, after clarifying structured unconfirmed
reservations and limiting cleanup to active helpers. Read-only same-family
review limitations and absent physical evidence remain explicit.
