# P5i1: explicit full address-space reassignment

## Intent, baseline and chosen shape

Finish the already-approved bus action for full reassignment of outputs, inputs,
or both. Baseline `237bd17` accepts the protected Scenes slice. Lost-address
restoration follows separately in P5i2 using an identified temporary candidate;
this slice must not claim that workflow complete. No live bus, Store, v1, SDK,
PanelScript or UART recovery change belongs here.

The existing bus page adds unaddressed fixtures and refreshes addressed fixtures.
`service::commissioning` assigns one fixture under a permit, durable reservation,
original deadline and physical stamp. Its search initializes only unaddressed
gear. The independent model already supports output Store-DTR-as-short-address
(opcode 0x80), including MASK, and both INITIALISE selection schemes. Inputs
currently support commissioning specials and current-value queries; standard
input address writes/broadcast/DTR0 must be added to the independent model.

Three shapes were checked against the current code:

1. Initialize All and assign the entire bus inside one runtime command: up to
   64 searches exceed the existing 20-second command ceiling. Carrying a native
   initialization session across jobs would add unnecessary session ownership.
2. Clear short addresses in the selected space, then reuse bounded unaddressed
   assignment: handles used gear, duplicate old addresses and a full 64-slot bus,
   while retaining current per-step progress and cancellation. **Chosen.**
3. Remove addresses only from saved rows: misses unsaved/duplicate fixtures and
   cannot implement a complete rescan of used fixtures.

Do not invent another persistence file, journal, durable commissioning lease or
background queue. `Model::edit(expected, &[Edit])` and `ConfigStore::edit` already
accept atomic edit vectors. Existing RemoveOutput/RemoveInput edits retire
Origins/descendants, prune group members and private scene expectations. Reuse
them rather than introducing a parallel inventory-reset mechanism.

## Explicit effects and wire contract

Add WithReply `reset_address_space`, controller target only (no target field),
with strict `bus`, `space: outputs|inputs`, and `expected_revision`. Original
controller generation is mandatory; HTTP keeps session/CSRF/generation. Validate
Ready routing, configured enabled bus, original revision/token, current ready
HAT/bus, and the normal repeat profile before retiring any names. Refuse while
Blink/Stop restoration claims that bus. No raw address/frame API is added.

One request resets one space. The UI's Both selection freezes both spaces and
submits two explicit steps, advancing only through its own exact accepted
revisions. A failure/cancellation after the first space leaves that partial
result visible and stops. This avoids an opaque long bus operation.

The reset retires every saved device in the selected bus/space, including missing
rows and input instances. Fresh assignment starts with default names and fresh
Origins/tokens even when names or short addresses are reused. It does not infer
continuity from old addresses. Output retirement also removes those rows from
saved group membership and private scene expectations. Keep group and scene slot
names. Native brightness, colour, group bits and scene memory are preserved;
the operation changes short addresses only. Operators may subsequently use the
existing group/scene verification and explicit repair/clear/adoption workflows.
Other buses and the other address space retain their identities/settings.

Return a small typed report: `bus`, `space`, `outcome: cleared |
retired_but_unconfirmed`, `revision`, `retired_devices`, `retired_instances`,
`checked_addresses` (0..64), and optional reason. Counts describe saved retirement
or completed queries, never inferred physical fixture counts. Do not return a
potentially enormous list of target strings: public identity history and the
frozen UI preview already supply those. Existing 32 KiB job budget suffices.

After durability, any incomplete physical operation returns the committed
revision with retired_but_unconfirmed. A configuration failure/uncertainty uses
existing error/frozen routing and never starts physical clearing. No rollback,
automatic retry or claim of full assignment follows an uncertain result.

## Save-before-clear and physical ownership

Use `service::reassignment` plus a concrete `physical::Work::Reassignment`.
The request retains original context/deadline, one permit, optional original SDK
CommandCompletion, selected bus/space, original physical stamp and committed
report. The public local helper uses the usual nonwaiting admission and 18-second
absolute budget. Proxy and PreparedCommand share strict parsing/typed dispatch.

At owner admission, capture a ready physical stamp directly from Hat. Add a small
checked Hat helper that checks the same readiness/busy/closing/evidence/profile
conditions as physical enqueue; it returns the existing Stamp, not a new epoch
type. This reset needs no exploratory UART read before saving. The configured
output repeat policy must be known even if the saved output list is empty;
unsaved addressed physical gear may still be present. Inputs reuse the established
pair of ordinary 24-bit exchanges under one 100 ms host deadline, so do not
require an unverified 16-bit j/t profile for an input-only reset.

Freeze the selected rows, construct existing removal edits, publish Writing,
await the real ConfigStore result under the original cancellation/deadline, and
use the normal accept_save/activation path with original CommandCompletion.
Thus all retired targets/generations become unavailable before a clear frame
can be sent. Empty removal vectors remain valid no-ops: hardware can contain
addressed gear absent from the saved list, so physical clearing still runs.

After confirmed activation, retain the committed report/revision and recheck
caller/deadline/original physical evidence before enqueue. Work::required_stamp
must carry that same original stamp; never substitute a newer one after saving.
Post-save resolution checks the own committed revision/Ready phase and selected
bus, not the now-retired controller token against the new table. Original context
still owns the reply and trace. Keep the permit through execution, settlement
and final acceptance, using the current physical owner and cancellation paths.

Physical plan for outputs: broadcast DTR0=255 (0xA3FF), then repeated broadcast
SetShortAddress (0xFF80). For inputs: broadcast DTR0=255 (0xC130FF), then the
repeated standard device SetShortAddress (0xFFFE14). Both ordinary inputs and
already-unaddressed devices hear the broadcast. Keep each DTR/write sequence
together under the same permit. No INITIALISE/RANDOMISE session is left open.

After sends, query short-address Status at every 0..63 address in the selected
space. Cleared requires all 64 queries to return NoResponse, current original
physical evidence and a current own committed mapping at final acceptance.
Any value, collision, timeout or stale stamp is unconfirmed; preserve the number
of completed address checks. HAT N is a transport result, not proof that gear
accepted a command. Absence checks do not claim any number/type of fixtures.
Subsequent assignment establishes fresh addressed presence in the usual way.

Pure checked address-frame builders belong beside the existing DALI codecs.
Support short/broadcast destinations and checked optional short addresses so
P5i2 can reuse the same normal address-write primitive. Raw packet vectors must
be independently asserted. The emulator implementation must remain independent
of these builders and of bridge Inventory: extend per-input DTR0, standard
device/broadcast addressing and repeated SetShortAddress acceptance. Preserve
current input value latches, events and independent commissioning behavior.

## Protected web workflow

Bus actions contains Bus settings and Reassign addresses. Choose outputs, inputs
or both, then review an immutable preview identifying controller/bus, selected
spaces and saved device/instance names/counts. Explain that old device names and
their saved memberships/scene expectations are removed and fresh devices receive
default names. Explain that native group/scene memory and live output remain.
Only explicit confirmation submits a command; opening/browsing changes nothing.

After confirmation, show progress on the existing bus device list. Process each
selected space in turn: reset, then assign/discover/refresh that space before
starting the next one. Only a confirmed cleared result permits assignment.
Reuse existing assign_next_output/input and discover_address steps until
no_unaddressed_* or 64 verified assignments. At capacity, report the space full
without claiming that no unaddressed fixture exists; a completed capacity-limited
space may proceed to the other selected space. A premature no_free_address,
unconfirmed result, external revision, replaced/renamed bus, disconnect or
cancellation stops later submissions. Completed retirement/assignments remain
visible. Do not declare reassignment complete merely because addresses cleared.

After assignment, refresh addressed devices and current values in the selected
space only. Do not call Controller.scan wholesale: it intentionally scans both
spaces and could adopt unselected input/output gear. Compose the existing typed
commands per space; keep the current ordinary Add/Refresh behavior intact.

Freeze initial Inventory/Origins, bus name, expected root, selection and Client
Views epoch. Advance the expected root only through own responses and exact SSE
snapshots, allowing valid skipped reserved revisions. Do not use metadata()'s
implicit latest-root injection. Each operation has its own lifetime flag bound
to the actual route/tab, not only the BusPage's number key: leaving the bus/tab
or logout cancels later steps even if another operation resets global flags.
Confirmation and progress/cancellation fit 390×844 and desktop viewports.

## Parts and required evidence

| Part | Files/analogue | Required proof |
|---|---|---|
| Frames and independent physical model | dali addressing helpers; emulator/model.rs,input.rs | Literal output/input clear and addressed vectors, both repeat profiles, partial/late pair refusal, 64 fixtures, duplicates and unaddressed mixes; preserve other space/bus, values, groups/scenes |
| Atomic retirement | Existing Model/ConfigStore edit vectors and contract tests | Whole selected space/descendants retire in one saved revision, private rows/members prune, unselected Origins unchanged, default-name/address reuse gets fresh tokens, empty saved list still permits hardware reset |
| Owned reset | New service/reassignment, owner integration, physical work and Hat checked stamp helper | No clear before durability/activation; save failure/uncertainty; held save and original stamp; bus-cycle/late final rejection; original deadline/permit/completion/context; truthful partial report; no new recovery policy |
| Shared MQTT/HTTP | proxy Operation/PreparedCommand and jobs | Strict scope/params/tokens/revision; independent consumer sees retirement before native clear, old retained intent cannot affect reused names; auth/CSRF; no dependence on Store |
| Responsive bus action | app/Devices and small commissioning helper | Frozen two-space review, own exact revision/epoch advancement, cancelled/unconfirmed step stops, no implicit rebasing; all three scope selections |
| Real browser | Existing private broker/PTy browser harness | Cancel preview sends nothing; outputs-only, inputs-only and Both reset/assign with disk/native identity assertions, preserved live/groups/scenes, new default names/retirement, phone/desktop layout and readable dialogs; keep prior workflows passing |
| Acceptance and docs | fault controls, guides/matrix/progress/CLAUDE | Intended RED per guard; restored scoped lint/Pi/wait/format/plans and assets; frozen source gates/full native/browser; honest physical and restore limits |

Use existing save hooks and the isolated HAT stamp test harness. Do not invent a
runtime hook or a recovery journal for tests. Trace each result sink and budget
before exposing aggregate replies. New async local helper names/waits use the
existing registry. Documentation serves operators separately from contributors;
no Store catalog or global/user-memory write is needed.

## Grounding, open verification and review record

Protocol facts were cross-checked with the primary implementation
[python-dali gear commands](https://github.com/sde1000/python-dali/blob/master/dali/gear/general.py)
and [device commands/addressing](https://github.com/sde1000/python-dali/blob/master/dali/device/general.py),
[device broadcast encoding](https://github.com/sde1000/python-dali/blob/master/dali/address.py),
plus the [ATX I2/I4 manual](https://atxled.com/pdf/AL-DALI-HAT-I.pdf).
Verify the exact DTR0/address semantics and literal frames during implementation;
physical firmware pair timing and real fixture behavior remain unverified.

Second self-review checked the normal full-capacity case: the existing search
returns NoCapacity before testing unaddressed gear when all saved slots are full.
The UI therefore does not demand a NoCandidate reply after 64 verified assignments.
It also scopes refresh to the selected space instead of reusing the both-space
ordinary scan wrapper. These are normal installation cases, not new recovery
machinery. Count-only reset replies reuse the existing ordinary job budget.

Self-review checked the actual vector-edit API, per-space retirement/pruning,
shared generation replacement, no-op saved reset, old desired rejection, trace
and completion ownership, initial/final stamp checks, disabled/no-HAT refusal,
unverified profile, two-space partial cancellation, reply budget and reuse of
one-at-a-time assignment. Bridge-runtime memory reinforces separate maintained
v1 and generic consumer support; its historical implementation dates were checked
against this checkout. The owner already approved this workflow; no new product
decision is needed. Finish both fresh design lenses before production edits.


Both fresh design lenses APPROVE against baseline237bd17: implementer and
correctness. No evidenced design blocker; exact protocol semantics remain
literal implementation checks. Same-family reviews; cross-family unavailable.
Inline implementation follows the selected shape. Early local evidence: six
service tests, independent literal peer across both spaces/profiles and failure
cases, four new codec/model tests, six frontend tests, and the real browser
workflow pass. This is not the final acceptance verdict.


## Acceptance

Accepted locally at `2c39885`, production `dbc4f84`. Both sequential source
lenses APPROVE and extended approval through the test-only missing-fixture/
other-bus/permit-diagnostic delta. All 14 current controls have intended RED
evidence. Restored 8/8 checks pass; final acceptance passes 595 native tests,
32 frontend tests, doc tests and phone/desktop functional browser with 697owned
commands. Logs: `/tmp/dali-reassignment-restored-final` and
`/tmp/dali-reassignment-acceptance`. Physical HAT timing remains unverified;
P5i2 lost-address restoration remains separate. See the phase ledger for exact
scope, correction history and artifact paths. No new Store/global skill or
memory entry is needed: bridge CLAUDE and operator/contributor guides carry the
procedure. Review coverage is same-family only.
