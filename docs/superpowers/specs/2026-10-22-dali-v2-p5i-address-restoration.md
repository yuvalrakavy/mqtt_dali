# P5i2: recover a saved fixture after address loss

## Superseding owner clarification

On 2026-10-22 the owner clarified that the fixture may retain its new short
address, provided that it replaces the previous saved address and frees that
reservation. Preserve the saved fixture's name, identity and bindings; the old
numeric address is not its identity. This supersedes the earlier identified
recovery design that programmed the hardware back to its original address.

The primary workflow is find, identify, adopt the new address. Existing assignment
and discovery creates an ordinary temporary saved row. After Blink or input
activity identifies it, one atomic configuration change removes that temporary
identity and moves the original saved identity to its already working address.
No second native address change follows the save.

The already implemented sole-unaddressed shortcut can still reuse the original
unused address, including on a bus with all 64 saved slots reserved. It explicitly
associates that sole fixture, refuses none/multiple, and does not constrain the
normal workflow to preserve the old address. No persistent journal/lease.

Baseline: accepted P5i1 `8fc2635`. Current `Edit::ReaddressOutputs` already carries
Origin and desired group membership to another address; private scene intent is
keyed by Origin. Ledger replay permits same-space address continuity and checks
continuing input parent lifetimes. Add analogous `ReaddressInput` carrying the
original module and all saved child Origins/names/types/descriptions together.
Removing the original and renaming a candidate would replace its lifetime and
is rejected.

## Shared command contract

Both strict WithReply commands require the saved output/input-module target,
original target generation and exact expected_revision. Controller/group/scene/
instance targets, raw addresses, unknown fields and stale authority are refused.

`restore_identified_address` also requires candidate and candidate_generation.
The addresses must differ, with the same enabled bus and address space. It means
adopt the candidate's existing address into the original saved object.
`restore_unaddressed_address` explicitly associates the only participating
unaddressed fixture and reuses the saved address without a config edit.

Reuse MQTT/HTTP parser, HTTP authentication/CSRF, original request context,
optional SDK completion, nonwaiting permit and original 18-second ceiling.
Stop continuing Blink claims first. Only native unique addressing requires a
known output repeat-terminal profile; identified recovery performs queries only.

Report: operation, outcome, target, original target_generation, current_generation,
bus, previous_address, resulting address, revision, optional candidate/generation/
source_address, configuration_changed, reason. Outcomes: restored,
no_unaddressed_fixture, multiple_unaddressed_fixtures, unconfirmed,
configuration_updated_but_unconfirmed. Keep the actual committed mapping/root
in a post-save partial. Reports fit ordinary 32 KiB jobs. Restoration evidence
does not imply current light/button State.

## Identified recovery

1. Fresh Status proves the old address absent and source candidate present.
   Unknown/collision/error refuses mutation. Retain original stamp/deadline.
2. For inputs query complete count 0..32 and every type. Every saved original
   instance number/type must exist and match; extra physical instances are allowed.
3. Recheck original/candidate Origins, tokens and root at probe acceptance.
4. Save one ConfigStore edit vector: Remove candidate, Readdress original to
   candidate address. Candidate references/children retire normally. Original
   output group references move to the new address; its private scene rows retain
   Origin. Original input module/children retain lifetimes and rotate mapping
   tokens together. Other rows retain their identities. The old reservation
   becomes free in this same atomic change.
5. Activate via normal SDK generation update with the original completion. There
   is no post-save DTR/SetShortAddress/PROGRAM or second physical work item.
   Save failure/uncertainty follows existing ConfigStore activation rules.
   Cancellation cannot start further native work. Accept the own committed root
   and resulting mapping token, rather than requiring the now-rotated old token.
   A confirmed save must remain reported as saved if refresh later fails.

The reply uses the original request generation. Its current_generation describes
the resulting mapping. Generic consumers replay continuing original Origin and
retired candidate Origin to retain their device object and bindings. Never infer
continuity from reused text/address alone.

## Sole-unaddressed shortcut

One native operation, no config edit: fresh absence at the saved address;
Terminate/Initialize unaddressed/Randomise with existing repeat and settling;
inclusive 24-bit search plus exact Compare and QueryShortAddress=MASK; Withdraw
selected, search FFFFFF and Compare to refuse a remainder; reinitialize without
Randomise to reset withdrawal while retaining selected random address; query
complete compatible input topology via FD broadcast-unaddressed only after sole
proof; reuse Program/Verify/QueryShortAddress/Withdraw/Terminate/fresh Status.
Recheck candidate and empty destination before programming. Retain initial stamp
and original deadline through final acceptance. None/multiple/collision never
program; possible partial writes are unconfirmed without rollback/retry.

Same saved revision/token/address is valid here. This supports 64 saved
reservations without a temporary slot. Inputs retain ordinary 24-bit repeated
pairs with one 100 ms host deadline; outputs use configured HAT repeat profile.
Independent emulator FD query support must not import production codec logic.
PTY evidence is not physical HAT/fixture certification.

## Observations and queued actions

Cut only original/candidate observations at admission and settlement. Address
mapping changes rotate affected generations and invalidate historical routes.
Unique repair intentionally preserves token/stamp, so keep the already tested
per-route revocable Arc<AtomicBool> copied into Notices. A cut permanently revokes
the old flag, installs a fresh one, clears affected receive floors/native sequence
and marks values stale. Notice::current checks its own flag in MQTT helper and
SSE emission. Prompt the existing Hat boundary scheduler to rearm. Fresh activity
or queries never revive old queued presses; other routes remain current. No new
public generation scheme, queue purge, transport epoch or journal. Already
accepted SDK/broker publications follow existing delivery policy.

## Protected Devices workflow

Actions -> Restore lost address opens an inline recovery area. Find one candidate
composes existing assign/discover/current reads in the selected address space.
Identify on the same list using output Blink/Stop or input values/actions. Find
another is another bounded operation. Closing leaves completed candidates as
ordinary rows. No borrowed saved reservation; explain the unique/isolation
shortcut when no test slot exists.

Rows already assigned by an ordinary bus scan are eligible too: the operator
can scan two lost-address fixtures, Blink each new row, and associate each with
its original saved fixture without clearing or assigning their addresses again.
Confirmation names the selected saved row being retired, even if it was named
before opening recovery; never assume that every eligible row is temporary.

Frozen confirmation names original name/previous address, candidate/new resulting
address, temporary identity to retire, and explains that the previous reservation
becomes free while original bindings continue. A blinking selected output offers
Stop blink and adopt address, awaits confirmed prior-level restoration, then
submits. Other candidates offer Adopt identified address. Unique confirmation
separately explains reuse of the previous address.

Freeze both identities, root and connection epoch. Bind lifetime independently
to Devices route/tab/selected object; global cancellation reset cannot resume it.
No latest-root injection. Accept own skipped revisions and unique same-revision
results, reject unrelated streamed roots. After adoption update only the expected
own mapping token/address from exact result plus SSE, then discover/read at the
new address. Refresh failure must not imply the confirmed save was undone.

## Verification and documentation

Fail-first model tests cover input module/child Origin continuity, token rotation,
restart/replay parent identity, atomic candidate removal, old reservation reuse,
and unaffected routes. Service and generic consumer checks cover output group/
scene intent and device-object continuity. Identified native traffic contains
only queries: no address-write frames. Held/failed/uncertain saves, cancellation,
stale candidate authority, original/final probe stamps, permit and original
completion keep existing truthful activation semantics.

Retain unique tests for both spaces/profiles, full reservations, none/multiple,
occupied old address, exact collision, incomplete/incompatible input topology,
deadline and actual paused MQTT/SSE Notice revocation after fresh activity/query.
Strict HTTP/MQTT retain original replies. Phone/desktop browser checks cover new
resulting address, old reservation reuse, Blink stop before adoption, input child
continuity/activity, cancelled/frozen review, partial reports and route/reconnect/
external-root stopping. Existing commissioning/reassignment/scenes remain green.
Run fault controls, restored scoped checks and native/browser acceptance; freeze
source for two review lenses. Disclose same-family-only review and physical gaps.

Update commissioning/recovery operator/API, inventory/service/emulator/browser
guides, matrix, progress, wait registry and bridge CLAUDE. No Store/global skill/
memory writes. Self-review checked existing output readdress and ledger before
selecting this shape; the failure/neighbor pass checked child parent lifetimes,
candidate reference pruning, own token rotation, old-address reuse and refresh
at the resulting address. Owner clarification authorizes this revised behavior.

## Acceptance follow-up: discovery admission

The longer combined browser run exposed an existing interaction: a periodic
state read can hold the shared permit when the next addressed-discovery step
arrives. Nonwaiting discovery returned Busy and stopped full reassignment.
Discovery now queues once for that permit within its original absolute deadline,
then validates the original root/generation against the latest snapshot. It does
not retry an operation or renew authority. Expired/canceled waiters submit no UART
work. Other commands retain their existing admission policy. A fail-first literal
peer test covers contention, cancellation, expiry and a root retired during the
wait; an intentional-fault control restores nonwaiting admission.
