# Individual short-address clearing

The owner approved clearing one fixture while preserving its saved name and
bindings. This extends the approved commissioning workflow. No live bus action
is authorized. Tier T2; one design pass and two bounded source review lenses.

## Shape and interaction

Use the existing restoration physical-owner path, with a distinct Clear action
and native plan. A separate scheduler/request hierarchy would duplicate the same
authority, stamp, permit, cancellation and receive-cut rules. Bulk reassignment
is unsuitable because it retires saved identities and broadcasts address writes.

Output and input-module row menus offer **Clear short address**. A confirmation
names the controller/bus, fixture and address. It explains that the saved name,
bindings and old reservation remain; Add unaddressed can find it again, followed
by identified adoption. Cancellation sends nothing. A matching active Blink
must stop and restore successfully first; another Blink claim refuses clearing.
The dialog freezes root, row identity/generation, connection epoch and lifetime,
using the existing Restoration scope. Navigation/closure permanently cancels
later submissions. No automatic rediscovery or restoration follows clearing.
Explicit Cancel/Close handlers revoke synchronously before changing state;
`useLayoutEffect` cleanup revokes on unmount. Passive effects cannot provide this
guarantee. The browser regression holds a completed Stop admission, pauses passive
timers/animation frames and exercises real Cancel, Close and route navigation.

## API and native behavior

`clear_short_address` is WithReply through MQTT and authenticated HTTP. It takes
an output or input-module target, its original target generation and exactly
`expected_revision`; no raw address, group, instance or controller target.
Expose a typed `ServiceHandle::clear_short_address` wrapper. Reuse the original
18-second command budget, permit, context and completion.

Add Action::ClearShortAddress and Plan::Clear to service::restoration. Resolve
uses the existing saved-fixture, enabled-bus and exact root/token checks. Owner
requires no identification claim and captures the original physical stamp.
Treat Clear as mutating and require a known output repeat-terminal profile;
inputs retain ordinary repeated 24-bit frames under the existing 100 ms pair
ceiling. Cut current observations and queued input notices at admission and
settlement, through the existing permanent per-route revocation mechanism.

The native plan freshly queries presence at the saved short address, then uses
the existing addressed dali::addressing output/input plan with new address None:
DTR0 FF, addressed SetShortAddress repeated correctly, then query the old short
address and require NoResponse. No broadcast SetShortAddress, initialise,
randomise, config save or identity transition. Preserve other fixtures, native
level/colour, group/scene memory and all saved intent. Missing/uncertain presence
refuses writing. Partial transport or final stamp uncertainty reports unconfirmed;
never automatically retry or roll back.

Extend the existing report outcome with `cleared`. `address` and
`previous_address` both describe the unchanged saved reservation; revision and
mapping generations stay unchanged, configuration_changed is false. Success
requires current final physical evidence and the original saved authority. The
browser verifies operation, target, generation, bus, address and exact root,
waits for the matching streamed view, and displays retained-name/bindings guidance.
An unconfirmed reply cannot produce a success notice or trigger follow-up work.

## Verification and documentation

Fail-first service tests cover output/input clearing under both repeat profiles,
unchanged saved bytes/Origins/children/intents, unaffected fixtures, native
settings, rediscovery and subsequent adoption, absent/invalid/stale authorities,
unknown output profile, cancellation and final uncertainty. Literal codec/peer
coverage verifies addressed vectors and original repeat bounds. Strict parser
tests cover shared HTTP/MQTT admission. Frontend tests exercise frozen authority,
confirmed Blink stop, stale/closed/reconnected scopes and unconfirmed outcomes.
Real browser phone/desktop checks clear output/input modules, preserve saved
records, rediscover candidates and recover their original identities. Fault
controls must fail on missing presence, addressed scope, profile and final absence guards and
UI authority or stop confirmation. Run targeted and full native suites, frontend
tests/build/assets, scoped strict Clippy, Pi check and wait/format checks.

Update operator/API recovery, service/browser guides, support matrix and progress.
No Store/schema, global memory/skill, SDK publication or deployment changes.

Self-review round 1 checked the addressed codec and actual physical-owner path.
Round 2 checked unchanged-generation receive revocation, Blink/permit ordering,
strict parsing, cancellation and stale final reports. Existing cut/settlement
and saved-authority checks cover both successful and uncertain native clearing.
