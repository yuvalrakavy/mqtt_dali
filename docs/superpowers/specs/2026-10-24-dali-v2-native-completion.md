# Common DT1 emergency support and retained output type capabilities

Approved design for the next substantial P4 milestone. TierT2: new public commands and
observations. Approved architecture§8 and continuous implementation apply.
Following the owner's timing feedback, implement this whole family end-to-end
with one final regression/review cycle, not separate codec/model/service/UI
acceptance gates. Preserve meaningful fail-first and focused checks between
steps. Fresh source-grounded design review APPROVES after two P2 corrections: ordinary
Read must acquire type evidence, and pending timeout must retain pending state.
Review is read-only/source-only/same-family; implementation and physical checks
remain separate. The owner has authorized continuous implementation.

## Problem and choice

DT1 is the only wholly pending native family in the approved capability table
after Part253. Operators need battery/fault/test information, configuration and
explicit function/duration tests without a long-lived bus operation. Existing
DT7 settings/capture work also lacks retained relay capability information.

Use the current physical owner with a distinct typed Emergency work/report,
following OutputSettings preflight/confirmation and Memory original-authority
patterns. Reuse the existing clock/model and protected UI patterns. A separate
worker or per-test journal would add unnecessary lifetime/recovery policy;
shoehorning emergency fields into common settings would muddle its report.
Choose explicit captures and request/observe test operations: no backend poller,
new connection, retry, persisted test identity or claim of certified testing.
Keep existing scalar MQTT and configuration shapes compatible.

Grounding: service/output/settings{.rs,/execute.rs,/switching.rs}, service/memory.rs,
service/physical.rs, owner.rs, proxy.rs; output/colour.rs retains type evidence
only when color sampling is selected. Ordinary read_output currently does not
populate it. Add the bounded type-only read below; proxy/state.rs projects it.
Emulator model.rs advance_to, session.rs existing clock tick, scenario.rs optional
profiles; emulator-web/src/app.tsx cards/editor. Read those complete enclosing
paths before implementation. Canonical Store memory lookup found generic fleet/
binding guidance but no DT1/DT7 contract decision. Shared-component PropertyId
binding remains relevant for later P8, not a schema change here. No
Store/schema/v1/SDK changes in this milestone.

## Native facts and contract

Primary source python-dali gear/emergency.py (IEC62386-202:2009 commands), plus
current Beckhoff DALI202 variables/query documentation. Enable DT1 withC101
before every extended operation. Config/controlE0..F0 repeat the addressed
command under the known HAT policy; queriesF1..FD are plain. Do not insert a
second enable between the two repeated copies.

Prove individual output presence and DT1 afresh before extended traffic. Response1
to QUERY DEVICE TYPE proves DT1;FF means multiple types. For NEXT DEVICE TYPE
use a strictly increasing valid prefix,FE termination. A valid1 proves support;
a decisive higher type proves absence; silence/collision/malformed evidence is
unknown, never permission to write. Reuse a small pure interpretation helper for
existing output samples and fresh native probing; do not rewrite DT8 discovery.

Public operations, individual saved output only:
- read_emergency: Read class, no params. Same original prepared revision,
  generation, trace, stamp and deadline acceptance as current explicit reads.
- configure_emergency: WithReply, strict patch and expected_revision. Fresh
  presence/type/features/requested fields before any setting, known repeat
  profile before scratch traffic, actual readback afterward.
- start_emergency_test: WithReply, test:function|duration and expected_revision.
- stop_emergency_test: WithReply, expected_revision; cancels both pending and
  running native tests. No implicit clearing of prior result-valid flags.

Start/stop return promptly with request_sent plus fresh observed native status.
A native test can be pending or already active; don't claim a new correlated
test or completion from an acknowledgement. Subsequent explicit read_emergency
returns progress and last native valid-result flags. A duration test does not
hold the physical owner for hours. UI labels distinguish request/job completion
from the fixture's test. No automatic replay after reconnect or browser reopen.

One existing physical permit, bounded15s operation lifetime; original parse-time
revision/generation through queue/run/settlement/reply. Read has no cache cuts.
Configuration/start/stop use existing output-bus invalidation and Blink conflict
guards, preserving unrelated input activity and saved inventory. Follow current
partial/unconfirmed outcome policy, not a new rollback/recovery framework.

Use the existing report envelope: operation/outcome, original target/generation/
revision/bus/address, support:bool|null, before(optional), observed(optional),
raw readings, reason, configuration_changed:false. Observed values contain raw
byte bitmaps plus nullable named booleans, nullable settings in the public units
below, and counters with raw/value/unit/at_least. Null means no usable reading;
raw no-response/collision/invalid evidence remains in the report. Read outcomes
are observed; native writes are applied/unconfirmed or request_sent as specified.
Absent/unrecognized type reports support:false/null and no invented values.
Read quality describes evidence coverage, not healthy-battery certification.

Read captured raw readings plus typed nullable values:
- F1 battery charge0..254 (linear percentage and raw);FFunknown.
- FA mode,FB features,FC failures,FD status are FULL bitmaps, includingFF.
  Preserve simultaneous bits; do not invent one exclusive mode.
- F3 last duration result×120s;F4 emergency runtime hours;F5 total runtime×4h;
  F9 rated duration×120s. FF is saturated lower-bound data, not unknown. Label it.
  Current Beckhoff thresholds forFF are510min/255h/1020h/510min; python's F5
  comment says1016h. Use the current primary definition1020h and retain raw plus
  at_least; don't claim an exact duration.
- F6 emergency level,F7 minimum,F8 maximum: known1..254,FFunknown; limits ordered.
- F2 selectors4/5 function/duration intervals (days/weeks),6execution timeout,
  7prolong30s. Read next delays via selectors0/2 high byte then QUERY DTR1(9C)
  for the matching latched low byte; do not query a second live low value.

Report version/type/support/quality, raw evidence, features, mode/status/failure
booleans, units and unavailable/invalid/unknown fields. No transport silence
becomes false/zero. Existing common output status remains independent. Tests'
last-result display requires native done-and-valid flags; otherwise unavailable.
Native last result is not represented as the result of this request.
Status bit3 has conflicting labels in the two sources (fully charged versus
charger ready). Preserve it as raw status; use F1 for the battery-charge UI.
Do not publish a semantic bit3 label without revision-specific primary proof.
Other test/status/failure labels follow the primary bit tables.

Config fields: emergency_level, prolong_seconds(multiple30,0..7650),
function_test_interval_days(0..255), duration_test_interval_weeks(0..97),
test_execution_timeout_minutes(15 OR whole days1..255), and paired initial
function/duration delay_minutes(multiple15,0..983010). Adjustable level needs
fresh featurebit4 and requested value within known ordered manufacturer limits.
Automatic intervals/delays require fresh featurebit3. Zero interval disables it.

Native testDelayTime is shared write-only staging: stageEA/EB before the EC or
ED setter consuming it. Require the corresponding explicit initial delay when
setting a positive interval, and reject unused delay-only/disabled pairs. A patch
can supply separate first delays for function and duration; stage/consume each
pair separately. Do not expose shared DTR staging to consumers. Native next-delay
readback can advance or trigger an immediate test: compare observed evidence
honestly and return unconfirmed fields when it cannot establish the requested
setting; don't retry or invent proof. UI retains unconfirmed drafts.

Duration interval97 follows both python and Beckhoff's table; one Beckhoff
paragraph says255. Publish this supported limit and preserve out-of-range raw
evidence instead of guessing. Next-delayFFFF means unknown/unimplemented, but
byte255 in interval/timeout/prolong fields can be valid. Keep per-query semantics.
REST, INHIBIT, factory reset, counter/result resets and physical certification
are outside this common-operation slice; no hidden use of them.

## Retained capabilities and controller UI

Project read-only SwitchingSupported and EmergencySupported booleans/null from
current output type evidence, under existing lifecycle/generation rules. Normal
output reads, including ordinary fixtures with no selected color representation,
must acquire that evidence: add a type-only QUERY DEVICE TYPE and, for FF,
strictly increasing NEXT DEVICE TYPE enumeration bounded to nine replies, ending
on FE or a valid type greater than eight. Reuse the existing discovery evidence
when color sampling already occurs; do not probe twice. Retain it in the existing
output observation, separately from color selection/samples so a type-only read
does not enable full color sampling on subsequent reads. Base writes need no
new capability preflight; their next normal read refreshes evidence. No separate
capability cache, configuration field, worker or persistent device-kind inference.
The same original request deadline applies. Type evidence cannot make missing
base readings available or change independently successful base-state quality.
A valid prefix can establish a positive fact without pretending the complete
type list was read. Unknown or stale type evidence stays null. Do not equate
not-DT7 with a proven dimmer, or derive capabilities from names/selected color.

These are observed capabilities, not new persistent identities or an automatic
Home equipment rename/class migration. Keep existing PowerState/Brightness wire
bindings compatible. Controller rows may identify proved relay/emergency types;
an ordinary relay uses its power toggle and does not offer misleading continuous
dimming controls. Scene engineering values on a proved relay say virtual level.
Store class materialization and persistent bindings remain in P8.

Add a compact Emergency lighting action/dialog on individual output rows, with
explicit Read and clear unsupported/unknown states. Status includes battery,
mode/fault flags, pending/running and last valid results. Settings use actual
units and feature-dependent availability. Function/duration/Stop are explicit
buttons with test request/current-status labels and a Refresh status action.
Duration request explains that it uses the battery for the native test period.
Reuse original frozen scope, synchronous retirement, dirty-draft retention and
failure clearing. No HTML touch-panel or Home/preset design changes.

## Independent emulator

Optional strict emergency profile on DT1 outputs, old scenarios unchanged.
Store native settings/flags/counters separately from temporary repeat/type-enable
scratch. Model literalC101 and repeatedE3/E4/E5/E9..EF independently of driver
code; reject unenabled/unrepeated setters and unsupported feature settings.
Runtime tests use the existing model clock and bounded per-gear state, not new
tasks. Frozen clock lets browser tests advance minutes instantly. Simulate
pending/running/complete/failed function and duration tests with configured
durations/outcomes, stop cancellation and correct result-valid/failure bits.

Execution timeout is distinct from a completed failed test. Start its clock when
the native request becomes pending. At expiry set failure bit4 (function) or bit5
(duration), while preserving the corresponding pending flag; do not transition
to terminal failure or silently cancel the test. Native timeout zero means15min;
nonzero values are whole days. A later eligible transition may still start the
pending test; explicit Stop cancels pending/running work. Keep this bounded state
inside the existing per-gear clock model. Frozen-clock tests cover immediately
before/at the15-minute threshold, nonzero-day encoding, retained pending with
the failure flag, later start and Stop after timeout. No new timeout worker.

Automatic schedule modeling must be bounded and explicit: at most one coalesced
due occurrence per kind per advance, matching existing large-time-jump policy;
never loop through missed years. No physical battery discharge/charger/output
illumination model or compliance claim. Native settings and counters survive
transport reboot; scenario reset reloads configured profile. Capture saves native
configuration/observations, not active timer/enable/repeat scratch.

The emulator's own UI configures the optional profile and offers battery/failure/
test-outcome controls and live mode/progress display. Retain its separate process,
TCP HAT endpoint, existing JSON editor and scenario capture/import/reset. Add one
ready-to-use maintained emergency fixture scenario; preserve all existing demo
devices/configuration when refreshing its owned processes.

## Execution and verification (one milestone)

1. Pure type interpretation, DT1 frames/ranges/units/bitmaps/saturation and
   setting plans fail-first. Independent literal expected frames, including
   enable/repeated copies, correct query selector/DTR1 and paired delay staging.
2. Independent profile/model plus clock tests for current state, pending/running,
   success/failure/stop, feature refusal, automatic setting consumption/coalescing,
   pending timeout without cancellation, capture/reboot/reset and absence on
   ordinary fixtures. No driver codec imports.
3. One owned native read/config/test path, strict generic MQTT/HTTP/typed API,
   original prepared authority and final checks. Literal UART and independent
   TCP tests for both HAT profiles, unverified-write refusal, MASK/saturation,
   readback/unconfirmed and preservation of unrelated native/config/cache state.
   Focused midpoint check; no full acceptance cycle per substep.
   A fresh ordinary relay and emergency fixture with no color selection must
   acquire retained support through normal Read. Assert one bounded type probe,
   no full color discovery, no duplicate when color is sampled, null on stale/
   malformed evidence, and unchanged ordinary base quality and deadline policy.
4. Retained support fields and protected frozen UI, real phone/desktop/emulator
   test/config/status interactions with clock advance and lost/retired replies.
   Existing relay/DT8/base/input behavior must stay compatible.
5. Documentation/skills phase: emergency guide, MQTT/UI/emulator pointers,
   current support matrix/tracker and controls. No Store memory/skill edits.
   One mutation pass, frozen fresh depth review, one full native/frontend/linked
   browser cycle plus Clippy/Pi/waits/format/assets/plans. Physical validation
   remains pending; no main merge/push/publication/household HAT deployment.

Bounds: at most256 physical outputs across4 buses, one current emergency work
and permit; at most2 schedule deadlines,2 pending-since instants and one active
test per emulator gear; bounded reports/UI clone per read. No history growth, per-test journal,
new listener/connection or worker. Confirm all enclosing owner/action arms and
actual available profile/clock APIs during implementation.

Self-review: new type fields derive only from current evidence; they don't
change the selected public color contract or persistent identity. Timing setters
consume shared native state, hence paired first delays rather than a hidden
arbitrary schedule. Unknown/malformed support prevents extended writes; allFF
semantics vary by query. Source conflicts are recorded rather than copied into
undocumented assumptions. Verify during implementation: physical work/owner
stop and settlement arms, model repeat-enable ordering/capture of base state
without active timer overlays, and UI dirty paired schedule edits. Named test
seams above cover these; no user decision is needed for routine integration.

Primary references:
https://github.com/sde1000/python-dali/blob/master/dali/gear/emergency.py
https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/12783946635.html
https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/12783980939.html
