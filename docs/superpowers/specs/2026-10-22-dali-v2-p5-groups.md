# P5e: output group assignment, adoption and repair

## Problem, existing infrastructure and selected shape

Approved design section 11 requires group slots 0..15, individual and pattern
assignment, verification including unnamed slots, explicit first-adoption review,
and hardware-to-config adoption distinct from repair. The current inventory
already stores named groups/members and supports AddGroup, RenameGroup and
GroupMembers edits. Its ledger preserves group Origin and rotates membership
bindings. The legacy driver and independent emulator implement group queries and
repeated add/remove commands. The managed v2 service has no group operations yet.

Use one fixture per bounded operation through concrete physical::Work, with
probe/save/program phases following the established commissioning owner. A new
bus-wide transaction/job database would duplicate ownership and make partial
physical results harder to explain. Programming before durability would lose the
desired configuration on save failure. Save desired assignment before changing
hardware, then verify; retain the same permit and captured physical stamp. No
persistent UART journal, new recovery policy, automatic retry or rollback.

T3: persistent group intent and physical settings. Standing mission approval
applies. This slice supplies per-fixture primitives; the future UI/job layer
orchestrates bus verification and frozen pattern previews (Add/Replace), stops
between fixtures, and does not silently refresh a failed revision precondition.
It does not claim the full browser/batch workflow or group level/color control.
Design budget: two grounded self-review passes and two sequential read-only gates.

## Pure protocol and captured evidence

Add `dali::groups` with a typed 16-bit Mask, validated existing GroupAddress and
literal frame builders. Query low/high membership with C0/C1; bit 0 is group 0,
bit 15 is group 15. Both FF bytes mean mask FFFF, not MASK/unknown. Add/remove
use 60+slot / 70+slot, sent twice through existing ATX t and known repeat policy.
Bound every slot and short address before encoding. Plan single-bit changes or
full-mask differences (removals then additions), never alias slot 16 to another
opcode. The [primary python-dali implementation](https://github.com/sde1000/python-dali/blob/master/dali/gear/general.py)
independently specifies those queries and send-twice settings; no source is copied.

A Probe retains low/high optional bytes, quality complete/partial/missing/collision
and stale flag. A full mask exists only for two valid bytes. Never merge a missing
half with zero or a previous query. Queries are captured sequential evidence, not
an atomic snapshot across other DALI masters. They do not publish light State or
infer power, color, presence or current group availability after persistence awaits.

## Public operations

Add strict WithReply controller command `create_group` with required bus, group,
name and expected_revision. Map matching MetadataEdit to existing AddGroup with
empty members/description. Require current controller token and saved bus. Slots
0..15 and names use existing validation; occupied slots are rejected. Existing
group rename remains `rename` with its item token. Creation does not touch hardware.

Add typed ServiceHandle methods and these individual-output commands:

| Command | Required params besides output target | Result and authority |
|---|---|---|
| `read_groups` | none | Read class; optional original generation, validated if present. Captured mask, saved expected mask and drift, with unresolved evidence retained. |
| `assign_group` | group, member(bool), expected_revision | WithReply, current output token; selected group must already be saved. Save that one desired membership and enforce only that bit. |
| `adopt_groups` | reviewed_mask(u16), expected_revision | WithReply, current output token; fresh probe must equal reviewed mask. Save this fixture's complete hardware memberships, creating names for newly observed slots, without hardware writes. |
| `repair_groups` | reviewed_mask(u16), expected_revision | WithReply, current output token; fresh probe must equal reviewed mask. Enforce all 16 saved membership bits, including removing unmanaged bits, without metadata changes. |

Reject unknown fields, non-integers/negative/out-of-range masks/slots, non-output
targets and stale revision/item authority. Read accepts no params. Assignment,
adoption and repair use the same output route resolution and original trace
propagation. Read replies keep original MQTT metadata, including absence; reports
carry the actually resolved target binding generation and inventory revision.
`read_groups` is this proxy's first Read-class command: declare its class separately
from the existing WithReply list and dispatch it before requiring CommandCompletion.
The SDK does not provide that capability for Read execution. Include a real MQTT
read without generation or trace headers; its captured result succeeds while the
reply's absent metadata remains absent. No SDK extension is needed.

The serialized report fields are `operation` (`read`, `assign`, `adopt`, `repair`),
`outcome` (`observed`, `verified`, `adopted`, `unconfirmed`), `target`,
`target_generation`, `bus`, `address`, confirmed `revision`,
`configuration_changed` (bool), `expected_mask` (u16), optional `probe`, optional
`drift_mask` (u16), optional `reason`, and nullable assignment `group`/`member`.
Probe fields are `low`/`high` (nullable u8), `mask` (nullable u16), `quality`
(`complete`, `partial`, `missing`, `collision`) and `stale` (bool). Nulls remain
explicit. Drift exists only for a complete probe and equals mask XOR expected.
Single-bit verification proves only that selected
assignment; other hardware differences remain visible in drift. Whole-mask repair
requires exact final mask. Adoption means confirmed saved metadata, not current
physical evidence after disk waits. Missing/incomplete probe cannot authorize a
mutation. Failed admission/pre-save transport returns existing ServiceError.
Confirmed desired save followed by failed/canceled/stale programming returns
structured unconfirmed when a reply is still deliverable. A failed repair after
entering its program phase also reports unconfirmed against the existing saved
revision. Never report a preparation probe as post-write verification.

## Desired state and safe first adoption

Assignment changes one saved group's member list, preserving its other members,
every other group, fixture identities and descriptions. It programs only that
group's bit; existing unnamed/unselected hardware groups remain untouched. Saving
an unchanged desired bit is a no-op revision, but actual hardware may still need
that one-bit repair and must be read/verified. Group bindings rotate only when the
saved membership changes; the selected output binding remains unchanged.

Adoption compares all 16 fresh observed bits with the reviewed mask, then builds
one atomic existing edit batch. For each saved group, add/remove only this output;
for each observed true unsaved slot, create a group containing this output.
Choose unused GroupN names using existing unused_name logic. Preserve all other
fixtures and group Origins. A repeated adoption is a no-op. No empty slots are
invented and no physical add/remove commands are sent. After save, mark captured
probe stale if its physical stamp no longer holds; metadata can still be adopted.

Repair derives desired mask from all current saved groups. Both its preparation
probe and the fresh pre-program read must still match reviewed_mask. A changed
mask refuses settings writes with a review-again reason. This is the explicit
operation that can clear unnamed hardware assignments; initial discovery/read
and ordinary single-bit assignment never do that. No human-approval token or
new persistent job object is introduced.

## Concrete ownership and failure boundaries

Implement `service::groups` Request/Route/Observation/Completion and concrete
probe/program plans; extend physical::Work/Run/Sample/Completion and Hat enqueue.
Keep replies and the global permit in the owner, not spawned helpers. Route
through existing Transactions, same actor/bus stamp, final drain and settlement.
Known repeat profile is required for assign/repair before probing or saving;
read/adopt work with default Unverified. Firmware/emulator never infer that policy.

Read gets a 2 s original MQTT deadline; mutations 18 s inside the existing SDK
20 s bound. Probe phase caps at 3 s and program at 4 s, clipped once to the
original deadline; no query renews it. Use existing fixed settlement/teardown
ceiling and actual owned joins. At most 16 settings frames and bounded queries.

After a complete assignment probe, recheck original output authority/revision,
phase, receiver/deadline and physical stamp; save desired membership via ConfigStore
and accept_save using original CommandCompletion. Only confirmed activation can
advance to program. The continuation carries its original physical stamp and
confirmed model revision, never a freshly stamped substitute. Re-read before
programming, change only needed bits and always read both bytes afterwards.
Repair uses the same probe/program handoff without a save. Adoption stops after
confirmed metadata activation. Owner handles group completions before snapshot
publication; sync routes as today and perform final evidence validation.

Failed/noncommitted save leaves old desired state and sends no settings. Disk/CAS
uncertainty uses existing global freeze/recover; recovery only adopts saved group
intent and never resumes programming. Confirmed save survives physical failure,
bus cycles, cancellation/restart and partial writes. Cancellation keeps the permit
through actual settlement; no attempt to roll back hardware groups. Native input
events continue through the existing monitor/owner/helper. Changes already made
remain inspectable by a new read, adoption or reviewed repair.

## Implementation and verification

1. Fail-first real MQTT read/assign workflow with private broker and independent
   emulator. Add pure literal frame/mask tests at slots 0/15, addresses 0/63,
   FFFF, empty/full masks and rejected 16/64, plus deterministic mask differences.
2. Expose create_group through existing metadata. Test strict fields, occupied
   slots, durable original reply, rename/no-op and continuing child identities.
3. Add owned group query/execution and concrete physical variants; literal peers
   cover low/high missing/collision, both repeat terminal counts, unknown profile
   preflight and no-op physical verification. Queries never manufacture State.
4. Test assignment preserves every unrelated saved/physical bit; drift stays
   visible. Adoption creates only observed missing slots and preserves all other
   fixtures; repeat adoption is a no-op. Repair requires reviewed masks, clears
   unmanaged slots only explicitly, and rereads exact result. Level/color and
   input traffic remain unchanged. Cover both address spaces at numeric 0.
5. Race/failure seams: held/failed/uncertain save; no settings before confirmation;
   bus epoch loss during save; canceled/expired probe/program with native events
   and permit retention; changed reviewed mask before program; partial settings
   failure and failed/mismatching verification return truthful unconfirmed data.
   Recovery/reopen retains desired state without automatic programming.
6. Independent consumer tests group lifetime/member-Origin changes, current output
   lifetime and intent continuity, new group additions and renamed group bindings.
   Fault controls remove each behavioral guard; require declared assertion RED,
   then restored suite (four test processes), scoped strict Clippy, Pi lib/all
   binaries, doctests, changed-file format/wait checks and diff review.
7. Ship operator/API guide, support/catalog/progress updates, explicit partial
   outcomes and first-adoption rules. Freeze source and run two sequential gates;
   same-family and absent physical hardware evidence remain explicit limits.

Self-review pass 1 follows the current group reducers, typed address builders,
independent emulator opcodes and common physical/save/completion ownership.
Pass 2 checks initial unmanaged bits, one-bit vs whole-mask verification, no-op
revisions, input-event sibling paths, original context after CAS, stale physical
continuations and save/recovery failure arms. No additional owner decision is
needed within the approved group workflow.
