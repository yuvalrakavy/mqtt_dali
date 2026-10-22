# P5f: assign one unaddressed DALI input module

## Problem and selected shape

The managed bridge can discover addressed input modules/instances and receive
common Part 301–304 events, but cannot assign a factory-new input module a short
address. P5b already supplies an owned output search → durable reservation →
verified programming flow. The independent emulator currently models only input
queries/values, with no input random-address commissioning state.

T3: physical addressing and persistent identity. Continue the approved mission.
Extend the concrete commissioning flow with an explicit input/output space, pure
Part 103 frames and an independent emulator extension. Keep its existing global
permit, captured stamp, completion capability, absolute deadlines and partial
reservation result. A second commissioning owner would duplicate durability and
teardown. A generic transport/job engine would expand the change unnecessarily.
No SDK change, UART journal, new recovery policy, automatic retry or rollback.

This slice assigns one unaddressed input module. Full reassign, lost-address
restoration, indication/event configuration and browser/batch orchestration remain
separate. It deliberately saves the new module with no invented instances; the
existing addressed-discovery operation then discovers real instances. The future
bus workflow can perform those two explicit operations in sequence.

Design budget: two grounded self-review passes and two sequential read-only gates,
using available same-family reviewers. Standing authorization covers inline
implementation after the design gates; no new owner decision is required.

## Grounded protocol boundary

The [ATX I4 manual](https://atxled.com/pdf/AL-DALI-HAT-I.pdf), page 6, documents
single 24-bit `l` frames and automatic repetition only for 16-bit `t` frames.
The [primary Part 103 implementation](https://github.com/sde1000/python-dali/blob/master/dali/device/general.py)
and [commissioning sequence](https://github.com/sde1000/python-dali/blob/master/dali/device/sequences.py)
provide the distinct input opcodes, unaddressed selector, raw short addresses,
send-twice flags and 200 ms randomisation allowance. The ATX driver in that project
also limits its `t` encoding to 16 bits; its response/retry loop is not adopted.
Do not copy Python source into this independently implemented Rust model.

Add `dali::input::commissioning`, using validated input Address and the existing
24-bit random SearchAddress value (or a small shared value extraction if needed).

| Operation | Literal Part 103 frame |
|---|---|
| Terminate | C10000 |
| Initialise unaddressed | C1017F, twice |
| Randomise | C10200, twice; allow 200 ms to settle |
| Compare / Withdraw | C10300 / C10400 |
| Search high/middle/low byte | C105xx / C106xx / C107xx |
| Program / Verify short address | C108aa / C109aa, raw aa=0..63 |
| Query selected short address | C10A00; raw address, FF unaddressed |

These are not the Part 102 shifted/odd short-address parameters. Query addressed
device status through the existing input Address/DeviceQuery codec. Output and
input short-address spaces are independent even at numeric address 0.

For the two send-twice input commands, send two ordinary `l` transactions under
one host deadline chosen before the first frame: min(original operation deadline,
now + 100 ms). Both single-terminal exchanges must complete before it. Keep the
same concrete physical helper and bus stamp; never renew the pair deadline or
use unsupported `repeat:true` with Bits24. If either exchange fails/exceeds the
bound, stop; do not retry or proceed as though initialization/randomisation worked.
This is a bounded host implementation using documented frames, not a certification
of real firmware timing. HAT/fixture timing remains unverified; slow firmware
can refuse rather than silently falling back to an unbounded pair.

The 16-bit repeat profile remains required only for output commissioning. Input
assignment uses two supported single-frame transactions, so `Unverified` does not
invent a 24-bit repeat profile or block these explicit pairs. Transactions/ATX wire
mode policy itself remains unchanged.

Do not introduce quiescent-mode state. The reference sequence uses it to reduce
event collisions; this bounded one-module operation can refuse ambiguous evidence
while continuing native event handling. Exact-candidate collisions or invalid
Compare replies cannot authorize a reservation. A noisy bus can fail explicitly;
adding a persistent silent mode without hardware evidence would increase scope.

## API, saved identity and honest discovery

Add strict controller WithReply command `assign_next_input`, with the same required
`bus` and `expected_revision` fields as `assign_next_output`. Reject extra fields,
item targets, invalid/disabled buses, stale controller generation/revision and
unready UART before physical work. Add the typed ServiceHandle method. Preserve
the original MQTT generation, trace, response topic and correlation across saving.

Reuse commissioning Request/Route/Plan/Observation/Completion with an explicit
`Space` (prefer the existing discovery Space type). Keep the existing output API
as wrappers selecting Outputs. Add report field `space` (`outputs`/`inputs`) and
outcome `no_unaddressed_input`; preserve all existing fields and output outcome
spellings. Input outcomes are `assigned`, `reserved_but_unconfirmed`,
`no_unaddressed_input`, `no_free_address`. Report address/target/revision/reason
retain the existing reservation semantics. This report-field addition is explicit
and tested; it does not alter the maintained v1 binary/wire.

Find a free destination using only saved input reservations and actual addressed
input status responses. Skip saved missing modules and physically occupied input
addresses. Do not test/reserve output addresses for this operation. Search all
24-bit random addresses inclusively, with range-collision handling and strict
exact-candidate/query-short evidence, as the established output flow does.

Before PROGRAM, save one `Edit::AddInput` with unused default `InputN`, selected
short address, no description and an empty instances list. This mints a fresh
module Origin and binding token, preserving every continuing input/output/group
lifetime. Existing P5a validation supports this empty module. Never invent instance
count, types, capabilities, state or availability from a search.

Only confirmed save/activation advances to PROGRAM with the original captured
stamp. Recheck free destination, selected random address and unaddressed status;
program raw short address, verify it, query it back, withdraw/terminate, then
require a fresh addressed input status before reporting `assigned`. Keep the same
global permit through search/save/program and actual settlement. A confirmed
reservation survives failure/cancellation/changed bus; report unconfirmed when
deliverable. Failed/unconfirmed save sends no PROGRAM. Existing freeze/recover
adopts metadata only and never resumes addressing.

After `assigned`, clients can call existing `discover_address` for the returned
input address using the newly confirmed controller revision/generation. It fills
real instance types under the same module Origin. This second operation has its
own ordinary authority and completion. Avoid a second CAS inside the assignment:
the original CommandCompletion capability is single-use. The assignment report
does not claim instance discovery is complete. The future one-button web workflow
will perform the follow-up without inventing types or a new persistence protocol.

Keep existing active-helper cleanup: best-effort Terminate only while that helper
still owns a ready link and its original deadline. Between-phase abandonment uses
the standard initialization timer/next explicit Terminate, without a cleanup job.
Use the existing 18 s MQTT original bound, 10 s search / 4 s program phase caps,
200 ms bounded input randomisation wait, and unchanged actual join/close ceilings.

## Independent emulator

Add optional validated input `random_address` to strict Scenario (default absent);
seed missing values independently in the emulator. Keep existing scenarios valid.
Do not perturb existing output random-address draws merely by seeding inputs;
use a separate deterministically seeded input RNG state with the same independent
emulator generator when necessary.
Input State owns initialization expiry, search address, withdrawn flag and pending
randomisation readiness. It must not import the bridge commissioning codec.

Implement raw C1 special commands independently: unaddressed/all initialization
selection, identical double-command acceptance within 100 ms, 200 ms randomise
readiness, inclusive Compare, raw short Program/Verify/Query and Withdraw/Terminate.
Initialization expires after the standard 15-minute window. Model input repeat
latching separately from output repeat latching; intervening frames invalidate
the appropriate pair. Preserve output DT8 selection semantics for 24-bit traffic
and existing input value/latch behavior. Reuse the emulator's independent seeded
random generator and deterministic clock. Missing/duplicate candidates remain
observable through existing response aggregation, with literal-peer collision
tests supplying precise failure evidence where identical responses cannot
distinguish multiple physical devices.

## Implementation and acceptance checklist

1. Fail-first real MQTT `assign_next_input` against private broker/emulator. Use
   occupied/reserved inputs plus an unaddressed multi-instance module while output
   0 exists. Verify fresh default module, raw programmed address, original reply,
   preserved physical output values/groups/scenes and existing input values/names.
2. Pure literal frames at short addresses 0/63 and random 0/FFFFFF; reject 64 and
   25-bit search values. Independent emulator tests 100 ms repeat/200 ms readiness,
   initialization selection/expiry, Program/Verify/raw query and preserved other
   address-space/value/colour behavior. Test its behavior without bridge codecs.
3. Independent literal UART peer tests random boundaries, exact/range collision
   distinction, invalid/no replies, only two `l` frames for each repeat, one shared
   100 ms pair bound and no resend. Native input events continue while waiting.
   Verify input assignment works with Unverified while output preflight is intact.
4. Strict payload/authority, no candidate/capacity, missing saved reservations,
   physical occupation, no settings before durable save, failed/uncertain/held/
   canceled save and explicit recovery. Changed bus through save cannot re-stamp a
   candidate. Partial programming/final status failure retains confirmed module.
5. Follow-up addressed discovery populates actual instances under continuing module
   identity; old pre-discovery actions cannot acquire new children. Independent
   consumer validates the new module lifetime and later children/intent continuity.
6. Fault controls remove each new family/encoding/pair/readiness/identity guard;
   require named behavioral assertion RED. Re-run affected output commissioning,
   input read/event and activation controls. Restore all source before complete
   native tests (four processes), scoped strict Clippy, Pi all binaries, doctests,
   changed-file format/wait/diff checks.
7. Update commissioning/emulator/API/support/progress docs and contributor catalog;
   distinguish assigned module from discovered instances and physical evidence.
   Freeze source and run two sequential source gates. No physical certification,
   full addressing UI or overall P5 completion claim.

Self-review pass 1 checked current P5b owner/physical flow, empty-module P5a
discovery, raw Part 103 commands and the HAT's documented frame modes. Pass 2
checked single-use completion across the proposed follow-up, output/input address
isolation, 100 ms pair deadline propagation, separate emulator repeat latches,
noise/collision refusal, no quiescent cleanup obligation and post-save failures.
