# P6c: light controls, current values and native input activity

## Problem and selected shape

The approved bus screen needs an on/off control and current input values, plus
live activity to identify the switch just operated. P6a has authenticated
inventory/status snapshots; P6b has owned command jobs. Neither exposes light
apply/read or native input actions to the browser. T3: preserve physical evidence
and generation boundaries at the new HTTP/SSE consumer. Standing approval covers
these features. Budget: about ten minutes design/review, 25–40 implementation and
verification, warm builds and new controls only, then one restored suite.

Reuse the same scalar intents, final acceptance and observation projections as
MQTT. Building a separate web state cache would duplicate stale-value policy;
calling the driver's public execute/apply outside its SDK worker would violate
publication/completion ownership. The existing typed handles and pure projection
helpers remove both needs. No SDK, persisted schema, target/MQTT grammar, UART
recovery or additional device capability change.

Grounded seams in frozen P6b `d2f6828`:

* `proxy::state::intent` validates the entire base/colour property map before
  either mutation; `colour::configured` reads the saved representation.
* `DaliProxy::output_budget` supplies plain 2 s, selected/known-colour 6 s and
  colour/combined write 8 s bounds. Input read is 4 s. Typed handles retain one
  service permit and original deadline through actual readback.
* `state::accepted` compares the final snapshot, exact output observation,
  current lifecycle and generation. `read_input_observation` similarly checks
  latest input mapping/cache, stale flag, deadline and bus health after awaits.
* `state::projections` includes input projections and uses current lifecycle,
  mapping, independent colour quality and native/query semantics. It exposes no
  private physical stamps or free-text reasons.
* `Notice::current(snapshot)` checks source generation, installed listener,
  enabled bus, phase and exact receive stamp. `Notice::value()` alone is not an
  acceptance check. Existing input broadcast is bounded to 64; Lagged means
  actions were lost and cannot be reconstructed from current state.
* SSE is pull-based and rechecks the session before emission. P6b job records
  already have bounded results and lifecycle ownership, but no change signal.

## HTTP jobs and observed snapshot

Extend the existing strict `/api/jobs` envelope with HTTP-only operations:

| command | target | params | budget |
|---|---|---|---|
| `apply_output` | current individual output | `{"values":{"PowerState":true,"Brightness":0.4}}` with optional selected-family colour scalars | existing 2/6/8 s policy |
| `read_output` | current individual output | `{}` | existing 2/6 s policy |
| `read_input` | current individual input instance | `{}` | 4 s |

Generation is mandatory and remains the client's original selection. These
operations do not change inventory and have no expected_revision parameter.
Unknown params/properties, group/module/controller targets, absent generation,
invalid scalar values and colour fields outside the configured representation
refuse before creating a job. Use the existing command envelope authentication,
Origin/CSRF, post-body session recheck and bounded nonwaiting admission. No MQTT
commands of these names are added; MQTT continues using Desired/read scope.

Preparation captures a separate concrete prepared action (existing command,
output intent/read, input read), validates via the existing resolvers and fixes
its deadline once. Reuse the parser for scalar intents; never update the captured
generation from later inventory. Execution uses typed ServiceHandle calls, then
the same latest-snapshot acceptance as MQTT. Extract the existing pure input
post-await acceptance into a shared helper rather than maintaining two policies.
No Reporter or driver apply/execute call from HTTP. Existing service snapshot
watchers publish physical observations through the SDK worker as before.

Successful read/apply job result is `{ "target": "...", "generation": "...",
"values": { ... } }`, containing accepted actual scalar values/quality rather
than the requested intent. A newer mapping or stale physical evidence before
completion fails safely under existing service semantics. Partial effects are
not rolled back or retried. Unsupported/missing physical values retain existing
Available/Quality semantics, including separate colour quality.

Add `states` to `/api/snapshot` and every SSE snapshot, an array of the same
target/generation/values records from the pure shared snapshot projection. It is
a complete replacement, not a partial merge: omitted base values and explicit
nulls mean unknown/unavailable and cannot preserve an older light/input value.
Initial unobserved items remain visible with truthful availability/quality; no
discovery report is turned into current State. Keep inventory/status fields.
No private errors, file paths, internal stamps, Settings or trace metadata.

Increase the HTTP whole-snapshot ceiling from 1 to 4 MiB to accommodate the
existing <=512 KiB public inventory plus actual per-item scalar/capability
projections. Do not change MQTT's inventory budget. Refuse oversized encoding
as a whole (503 for GET; terminate an unrepresentable snapshot stream); never
truncate, omit arbitrary fixtures or silently paginate a complete snapshot.
Verify representative dense valid inventories against this budget during
implementation; document an actual limitation instead of claiming every possible
valid configuration necessarily fits. Eight streams remain the unchanged cap.

## SSE actions and job updates

Keep initial/latest `snapshot`. Add:

* `jobs`: complete `{ "jobs": [...] }` from the bounded current job book, emitted
  initially and after job admission/completion. A watch counter coalesces changes;
  do not add a job-event replay queue. At most 32 existing bounded records.
* `input`: a native `Notice::value()` plus its original `generation`, emitted
  only after checking the latest snapshot with the exact existing
  `Notice::current` predicate. Expose that predicate crate-privately; leave MQTT's
  event payload unchanged. Do not infer actions from `states` or query results.
* `input_events_lost`: `{ "dropped": n }` on broadcast Lagged, followed by a
  current snapshot. This warns identification clients that activity was missed;
  it does not synthesize missing presses or replay old values.

Subscribe to snapshot, job watch and input broadcast before producing the initial
values so changes during setup remain visible. Keep all per-stream state bounded
(receiver cursors, dirty flags and at most one pending native notice/loss count).
Use the existing pull stream, no producer task or unbounded pending event list.
Do not prefer snapshot traffic so strongly that a busy input bus starves native
actions; polling between update sources is fair. Stop/expiry/revocation retain
their independent wakeups and session recheck before every emitted event type.
Concretely, prefetch at most one native notice/loss and coalesce snapshot/job
notifications into dirty flags, then rotate among the three ready event kinds.
This gives a pending native notice progress within three event emissions even
when snapshots/jobs remain ready; it also prevents native traffic starving those
updates. Await the existing stop/revocation/expiry and source notifications only
when no event is ready. Initial snapshot comes first. After a loss, schedule a
current snapshot before further native actions, then resume ordinary rotation.
Revalidate each pending native notice against the latest snapshot immediately
before emission, after all awaits; stale mapping/receive lifetimes are dropped.
Passive snapshots, actions, jobs and SSE reconnects never renew idle expiry.

No replay guarantee, SSE event id or Last-Event-ID resume. Reconnect starts with
current snapshot/job records and only newly observed native notices. Job watch
notifications occur after releasing the record mutex; borrow no watch guard
across an await. Stop still drops the stream/subscriptions within the same Server
and actual accepted-IO ownership. No new worker, timer, configuration knob or
failure-recovery state machine.

## Implementation and evidence

Files: `service/proxy.rs` and its scalar/acceptance helpers as needed, a small
crate-private projection export in `service.rs`, Notice predicate visibility,
`web/{handlers,jobs}.rs`, focused tests, wait registry, operator/contributor
guides, support/progress and a new control table. Existing shared command
validation and MQTT completion dispatch remain intact. No Store/PanelScript work.

| Property | Test seam / concrete tool | Detectable counterfactual |
|---|---|---|
| Real power/brightness/DT8 changes with actual accepted values | private HTTP jobs -> independent PTY emulator -> snapshot/result + physical model | echo intent or skip dispatch fails physical/result assertions |
| Strict atomic preflight and original mapping | malformed/foreign-family/mixed maps and stale-token HTTP requests; inspect emulator unchanged | second parser/partial validation mutates hardware or admits stale selection |
| Current output/input state, unknown/stale clearing | real queries/native frames and bus disable/rename; complete HTTP/SSE snapshots | stale values/current availability survive cutover |
| Post-await input/output acceptance remains common | existing held-read tests + new prepared-action tests at final snapshot seam | older observation or mapping can be accepted |
| Native action delivered independently of latest current state | emulator presses/releases/short actions -> actual authenticated SSE | deriving actions from State loses/duplicates the action |
| Native lag and route retirement | same pull-stream constructor used by HTTP, deliberately paused test consumer + real service/emulator events | stale generations emit; dropped events replay as presses or loss is hidden |
| Fair activity progress under ready snapshots/jobs | same stream, keep both other sources ready while a real notice is pending; assert input/loss within three emissions | strict snapshot-first scheduling starves the identification action |
| Job notifications are complete and bounded | real job admission/completion with SSE; coalesced updates resnapshot book | missing signal leaves spinner after completion |
| Session expiry/logout/stop protect every event type | live HTTP stream with jobs/native traffic and short test Auth policy | traffic renews session or emits after revocation |
| Dense snapshot/whole encoding bound | valid Model and shared projections, HTTP serializer limit seam | partial/truncated inventory or private fields appear |

Start with failing HTTP apply/snapshot and native-event regressions, batch fixes,
run focused tests then new/reanchored controls only. Plan-check touched old
control tables; the input acceptance extraction may require a reanchored row.
Restored full native suite (four processes), scoped strict Clippy, Pi four bins,
docs/fmt/wait/diff once. Freeze and run two sequential same-family source gates;
no hardware certification, browser workflow acceptance or live Logmon export
claim. P7 still supplies the approved responsive screens.

Self-review 1 opened actual intent, budget, final output/input acceptance,
projection and Notice/broadcast code; it selects shared pure consumers rather
than a second cache or SDK driver invocation. Self-review 2 checked the sibling
colour/base failure arms, absent/retired generations, post-await receive stamps,
snapshot replacement/null semantics, paused/lagged/reconnected event consumers,
job notification ordering, fair polling under input traffic, source closure and
logout/absolute expiry while other update sources remain active. Dense snapshot
measurement remains an explicit implementation probe, not an assumed guarantee.

Design gate 1 approves (about 50 seconds, same-family/source-only), requesting
the explicit simultaneous-ready fairness regression above. The three-kind cursor
is a local stream selection detail, not a second command scheduler or worker.

Design gate 2 approves (47 seconds, same-family/source-only). Implementation
measures a valid dense model at 522,331 inventory bytes, 1,824 initially
unobserved states and 1,054,621 HTTP bytes, crossing the former 1 MiB ceiling.
The first controls find an await-side loss mutation GREEN because snapshot
readiness leads to the equivalent prefetch conversion first. Consolidate both
through InputEvent::receive; its shared-rule control is behaviorally RED, without
adding a special scheduler seam for the masked branch. Keep the initial
diagnostic log/ledger and use the final 16-row control table for current evidence.
