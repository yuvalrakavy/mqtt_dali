# Input feedback Desired/State bindings

## Intent and chosen shape

The approved bridge supports hardware indicators through typed/MQTT commands and
protected UI. Add BridgeKit scalar bindings so any MQTT consumer can control an
input module's or instance's indicator without fabricating button measurements.
Tier T2: additive public properties and owner-accepted evidence. Existing user
authorization covers implementation. No emulator, SDK, Store or UI source edits.
Budget: two grounded self-review passes, one design gate, native tests/controls
and sequential final source gates. Tokens/cross-family reviews unavailable.

Chosen: feedback properties on the existing input target, backed by a separate
owner-accepted observation cache. Reuse the native feedback operation; do not
create an LED hardware address or a second device driver. Rejected: conflating
PowerState/input state (wrong semantics), or a separate Feedback child target
(artificial hardware hierarchy and extra binding identity).

## Public contract

Module and instance Desired accepts ONLY `FeedbackActive`, a JSON boolean.
Use existing scalar/atomic Desired topics and the exact original target token.
Reject null/string/number/empty/unknown maps, mixed input-measurement writes,
outputs, groups and disabled buses before native IO. Capture one current revision
for the existing service Set action; do not refresh authority after an await.

State adds disjoint properties:

| Property | Value |
|---|---|
| FeedbackActive | Actual boolean, or null without usable current evidence |
| FeedbackAvailable | Complete supported actual-state evidence, boolean |
| FeedbackSupported | Fresh proven support boolean, otherwise null |
| FeedbackCapabilityRaw | Fresh raw capability byte, otherwise null |
| FeedbackQuality | unobserved, stale, current, unsupported, partial, unavailable or type_mismatch |

Input Available/Quality, measurements and native action notices are independent.
Unsupported is not Off. Preserve unknown capability bits. Even equal Desired
values use the existing explicit native action when the runtime calls apply.
Report actual accepted evidence before completion; success requires outcome set
and requested actual equality. Failed/unconfirmed work cannot retain old Active.
No saved config changes, synthesized button actions, fallback, retry or rollback.

## Owner and snapshot evidence

Grounding: feedback::accept currently consumes an internal Observation and returns
Report after final mapping/deadline/stamp checks; owner publishes its snapshot
before finishing that reply. Group output provides the reusable accepted-cache
pattern; Snapshot has separate output/input/group maps. Input notices have their
own permanent receive cut and must not be touched by indicator operations.

Add Snapshot.feedbacks keyed by existing target. Cache an Arc<Observation> with
original RequestContext, final Report, physical Stamp and staleness. Accept and
publish the same capture before replying. Internal Reply mirrors group output's
Observed/Unconfirmed split; keep all public typed/command Report shapes unchanged.
The proxy's final acceptance requires original deadline, lifecycle, token/revision,
nonstale capture and Arc identity in the latest cache, not report equality alone.

Prune retired targets and disabled buses; stale on any inventory revision change.
At admission of any actual feedback read/set, retire that scope's cached evidence.
Before native progress and on settlement, retire feedback caches on stale HAT/bus
epochs, and on native input address clearing/unique recovery/reassignment even
when saved revision/token does not change. Those address operations cover the
module and all its instances. Ordinary output writes, input value reads/settings
and feedback operations on another scope do not retire unrelated feedback.
Retirement is permanent; an old Arc cannot become current after same-value reads.

## Publication and refresh

Grounding: BridgeKit Reports merges partial PropertyMaps per target and keeps the
last report context. Input projections currently publish one measurement map per
instance; HTTP expects one complete row per target. Reusing the measurement
context for newly sampled feedback would misattribute its trace, and republishing
unchanged feedback last after a button event would overwrite the event's cause.

Merge disjoint measurement/feedback fields into one whole State per target.
Keep the existing watcher, with no publication suppression or second SDK cache.
A bounded Snapshot.input_contexts map records the most recent owner-accepted
original cause and family. Update from the final accepted input observation
(which can be newer native data), current resolved Notices and accepted feedback
captures, including same-value observations. Explicit trace absence replaces old
trace. Native input updates/notices are handled before final feedback acceptance
within one owner turn; this defines aggregate acceptance order when a watch
coalesces snapshots. No timestamp/counter framework or per-property trace promise.

Unrelated snapshots retain that aggregate context. Retired evidence cannot
advance it; prune generation changes. If the last family becomes stale, use the
other still-current family's original observation context, otherwise an untraced
current mapping. Every watcher replay and delayed direct scalar report uses the
latest merged State/context, while exact feedback Arc acceptance still protects
its actual value. Commands/Acks keep their own original request cause. Thus an
old feedback completion cannot overwrite a newer input cause. Whole projections
also provide unique HTTP/read-result rows; no duplicate target or schema change.

Target refresh for a module reads feedback (8s). Instance target refresh reads its
value under the existing4s bound, then feedback under8s, one12s total ceiling;
revalidate both against the final snapshot and return actual complete state.
All keeps the current15s rotating cursor. Append feedback tasks for already
sampled scopes only, bounded8s each within the original total. Cold All does not
probe every possible indicator; explicit target refresh/read/set establishes
interest for future All refresh. Include unsupported sampled scopes. This avoids
new unsolicited native queries during startup while enabling subsequent polling.

## Execution and tests

Phase1: accepted feedback cache, invalidation, disjoint/merged projections and
existing command publication. Fail-first native tests for real typed commands,
cache/reply agreement, failed sends, unsupported/unknown vs Off, lifecycle and
address changes. Snapshot constructors need the new empty cache; no persistence.

Phase2: strict proxy Desired and target/All refresh, original context, final
acceptance and publication. Actual ephemeral MQTT + existing independent PTY
model for module/instance isolation, measurement/notices preserved, refresh and
reconnect, original traces/trace absence. Literal peers supply missing/collided/
ignored-write/late-acceptance evidence. Update old targeted input-refresh peers
with explicit unsupported feature replies where their real entry point now
queries feedback; never teach peers invented success. No emulator file changes.

Controls vary validation, exact cache identity, invalidation, late final acceptance
and native actual equality one at a time. Include preexisting return-to-earlier-
value and tracing regressions. Run focused tests during implementation, then the
full native suite, Clippy, Pi, wait validation, affected control plans, generated
assets and whitespace. No new browser pass is claimed without UI source changes.
Docs: input feedback/MQTT guide, contributor guide, capability/progress records
and this as-built record. Root owns shared wait-registry edits; coordinate rows
with the parallel emulator session. No memory/skill writes or live operations.

## Grounded self-review

Pass1 checked feedback accept/finish and owner publication ordering, complete
proxy target/All routing, Snapshot fields, group retained/invalidate helpers,
Hat cached-vs-final stamp predicates and permanent input Notice cuts.
Pass2 checked the other branches: read failure cannot retain old true, unsupported
stays distinct from false, unchanged-generation address clearing still retires
old evidence, module/instance indicators remain separate, and partial State maps
cannot replace whole HTTP rows or overwrite new input traces with old feedback.
Actual SDK Reports::merge confirms unchanged reports also replace aggregate
context. The first gate caught asymmetric replay in the proposed feedback-only
suppression. The revised whole-State/owner-cause shape is approved; a fresh Arc
protects same-value captures without an operation-sequence framework.
Design gate APPROVE: buildability, original authority/trace publication,
stale-evidence scope, refresh fairness and requirement/alternative audit.

## As built and acceptance

Production `1cb2055`, corrected at `1cb11ea`, implements both phases. Native
feedback Report shapes remain unchanged through an internal Reply wrapper.
Owner-accepted Arc captures, scope retirement, whole-target State and aggregate
cause handling follow the approved mechanism. The merge preserves the existing
output/input/group projection order and appends new module rows.

Twelve new native tests cover commands, scalar/atomic writes, malformed maps and
original authority, actual ignored writes, exact-capture/deadline refusal,
unknown/unsupported/Off, revisions/native address clearing, same-value/replayed/
coalesced contexts, sampled-only All refresh and one HTTP row per target.
Fail-first evidence was absent feedback MQTT State. Six named guard-removal
controls are RED, including the strict independent ignored-write peer; old wide
input-budget and group pre-send cuts were reanchored and return their named REDs.

The initial full native run had760pass/5fail: an unintended sorted projection
order and existing tests expecting old query counts/measurement-only fields.
The corrected stable merge and explicit new contract assertions pass targeted
verification. Final full native **765/765** passes across eleven binaries with
the opt-in browser skipped. Strict Clippy, Pi library/four binaries, changed
formatting, all affected control plans, normal wait validation0, asset-byte
reproduction and whitespace pass. Both sequential source reviews and one
confirming correction reader APPROVE; same-family/source-only limits disclosed.
No source suppression cache, operation counter, SDK/config/native-protocol/UI
change or physical/live system operation was introduced.
