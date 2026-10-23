# Managed basic input feedback

The feature/feedback foundation is accepted at8dccb7f. Operators and generic MQTT
consumers still cannot inspect or activate a supported wall-panel indicator
through the managed bridge. This T2 slice connects that foundation to strict
commands and a compact protected dialog. It does not assume every input has LEDs
or introduce a second hardware owner. The approved mission authorizes continuing;
no user decision is outstanding.

## Grounded shape and boundaries

Read `input/settings.rs`, the shared `physical::Work` lifecycle, Hat admission/
settlement, owner dispatch/acceptance, strict proxy operations and frozen Preact
input dialog. New feedback work belongs in that same physical owner, with a
concrete `InputFeedback` variant and boxed fallback reports. Reusing the static
input-settings action would conflate indicator activation with event settings,
exclude device-level features and wrongly cut button observations. A separate
owner or retained hardware shadow is unnecessary.

Use saved Input module or InputInstance targets. Device-level and instance-level
feedback remain features of those existing identities, not synthetic fixtures.
Native instance Type or device Status brackets each probe. Query complete fresh
feature lists and capability before Active; make no support inference from HAT N
or a silent query. Original target generation/revision, physical stamp, caller
deadline and command completion survive every queue/await. No bus-off deferral,
UART journal, rollback or retry is introduced.

The prior checked primary references govern all frames: feature Type/Next8E/8F,
feedback Activate10/Stop11/Capability4F/Active4E, deviceFE/FC and instance00..1F/
20..3F selectors. The foundation guide records IEC103/332 and Sunricher evidence.
The native profile implements visible on/off only; support for other real devices
comes from their actual capability response. No brightness/colour/timing/audio
settings, group feedback, natural stimulus, scalar Desired property or Store/
PanelScript change in this slice. Scalar bindings remain later integration work.

## API, routing and evidence

- `read_input_feedback` is Read-class, takes an individual saved module/instance
  target and no parameters. Reads may omit generation; actual returned evidence
  uses the resolved current token and original trace.
- `set_input_feedback` is WithReply, with strict `expected_revision:u64` and
  `active:bool`. Require the original target generation and current revision at
  submit, owner admission and final acceptance. Reject other targets, missing/
  retired tokens, disabled buses, nulls, unknown fields and non-boolean values.
  Both commands work through typed ServiceHandle and the shared MQTT/HTTP parser.
- Report operation/outcome, target/generation, inventory revision, bus/address,
  optional instance/reported_type, before/current `feedback`, reason and
  configuration_changed=false. Device scope uses instance=null/reported_type=null.
  Values carry `features:Vec<u8>|null`, `capability:u8|null`,
  `supported:bool|null`, `active:bool|null`, complete/partial quality.
  Preserve unknown capability bits as raw data, not invented semantics.
- Start each probe with expected instance Type or a numeric device Status. A
  missing/collided presence query is unavailable; a different instance Type is
  type_mismatch. Then capture Type8E; FF permits at most66 adjacent Next replies,
  endingFE, strictly ascending feature32..96. Stop early on bad/missing replies;
  use the existing pure decoder. Keep the same address/selector and bus permit.
  Unknown/missing/malformed enumeration is null/partial, never an empty list.
- A complete list without32 is complete known unsupported: supported=false,
  capability/active=null and outcome=unsupported. With32, read raw Capability4F.
  A missing/collided capability stays null/partial. Recognized visible or audible
  support permits Active4E: FF means true; NoResponse may mean false only after
  the closing presence/type proof succeeds; any other byte/collision is unknown.
  A zero capability is known unsupported. An unfamiliar-only nonzero capability
  remains unknown/partial. Additional unknown bits alongside recognized support
  remain preserved and do not change the standardized basic activation command.
- Close with a fresh Type/Status proof. Never expose a false value inferred from
  silence if closing presence/type failed. Complete supported active evidence
  yields current. Partial optional evidence is diagnostic, never mutation authority.
- Set preflights a fresh complete supported probe before sending one ordinary
  addressed Activate/Stop. An already-equal active value still sends the explicit
  command (activation can restart hardware feedback timing). Read a new complete
  probe afterward and require requested actual state before outcome=set. Ignored
  writes, missing/changed evidence or possible failed sends are unconfirmed.
  Known unsupported preflight sends no mutation and reports unsupported.

## Existing ownership and lifecycle

`input::feedback` owns route/report/probe/execute/accept/finish and handle methods;
split a private route/probe helper only if it improves readability. Add one
Message/Work/Run/Sample/Completion variant, enqueue method and owner dispatch/
settlement/rejection plumbing. Requests hold the original bus permit, deadline,
context and optional MQTT completion. Bound methods to8s within the caller's
original deadline; use existing v2-service-request/physical wait rows and register
only the new local async wrapper name as needed.

Mark a write request started at physical admission for conservative fallback
reporting, without cutting input notices/values. Feedback does not alter event
configuration; existing received button activity remains usable and is neither
discarded nor synthesized. Work::mutating still classifies possible sends after
epoch failure as uncertain. Shared cancellation/link cleanup retains permit/reply
ownership. Reports are not cached in input State and do not edit saved config.
Final acceptance resolves the original action against the latest Ready snapshot,
checks deadline/stamp/staleness/token and rejects renamed/reused locations.
No restamping old evidence with a latest token. Errors after started writes clear
current evidence and return an unconfirmed fallback under original authority.

## Protected web interaction

Add **Feedback** to input module and instance row menus. The compact existing
Modal shows controller/bus/name/address and applicable instance, visible/audible
support and a single On/Off toggle when complete support exists, plus Refresh and
Close. Display actual state after verified readback, without optimistic toggle
success. Unsupported and unknown evidence explain why the toggle is unavailable;
do not invent brightness/colour controls or protocol jargon in the operator flow.

`FeedbackScope` freezes original row kind/location/generation, root/revision,
connection epoch and dialog lifetime. Validate result operation/target/token/
revision/bus/address/instance/type/config flag/outcome, feature-list shape,
capability/support/quality/active consistency and requested actual state. Only
complete known supported evidence can submit a Set. Recheck before I/O and after
every await. Explicit Close/dismissal revokes synchronously; layout cleanup does
the same on navigation/unmount. Late reads cannot revive or mutate a closed form.
Existing SSE snapshot authority is unchanged; HTTP reports are local evidence.

## Implementation and verification

1. Fail-first service tests, then implement bounded read/set with existing owner
   plumbing. Independent literal scripts cover complete singleton/multiple/no
   features, missing/bad end/order/range/collision, raw unknown bits, missing/zero
   capability, active false versus collision/non-FF, closing presence/type failure,
   unsupported/partial preflight with exact no-write counts and ignored writes.
2. Real service/emulator device and instance toggles, returned actual values,
   preserved saved bytes/identity/input measurement and continuing native Notice.
   Strict MQTT roundtrip/original generation/one reply; stale target/revision,
   unknown target/output/disabled bus, caller deadline and final epoch checks.
   Mid-checkpoint and self-review of all changed code/comments.
3. Frontend fail-first frozen scope/no-submit/actual-match tests, including module
   versus instance, malformed claimed-complete evidence, supported/unsupported/
   partial states, retired generation/revision/epoch, cancellation and dirty state.
   Real phone/desktop menus/read/toggle/reopen/restore at both scopes and a known
   unsupported input. Hold actual entry replies across dismissal/navigation;
   inspect screenshots/scrolling and actual model/inventory, not only success text.
4. Serial controls for missing capability/presence proof, collision-as-false,
   mutation before full preflight, ignored readback, original final authority and
   client no-submit/actual-match/lifecycle guards. Fix tests before freezing.
5. Freeze and sequential native/owner and UI/API source lenses; full native once,
   strict Clippy/Pi/format/wait/asset/whitespace/affected control plans, frontend
   and actual browser. Update operator/API/emulator/contributor guides, support
   matrix, CLAUDE and progress with observed checks and remaining scalar/settings
   boundaries. No shared SDK/memory/skill/deployment/publication or live actions.

## Two self-review rounds

Round1 traced all required Work lifecycle sites and the module/instance physical
row shapes. New concrete work fits existing bounded ownership; no broad refactor
is needed. Round2 checked silent Active versus absent/colliding devices, feature
enumeration adjacency, exact original authority, preserving input activity,
same-value activation, incomplete final evidence and module versus instance UI
guards. Correction: reuse of the input-settings writer would incorrectly revoke
queued button events, so feedback has a separate Work variant under the same
owner and no input-value/Notice cuts. Unknown capability bits alone do not prove
an operable base feedback type. Same-family review limitation remains.

## Design verdict and implementation checkpoint

The combined implementer/reviewer design gate APPROVE, source-only on
GPT6Astra/high. No source/runtime reviewer capability from another model family
is claimed. The initial real MQTT test fails with no such command, then passes
with managed registration/dispatch. The five initial native tests pass, including23
literal read/write cases and existing input neighbors. They verify preserved
current input evidence and a continuing button Notice across module/instance
toggles, exact no-write preflight, ignored/same-value actions, epoch rejection and
original caller expiry. The shared single permit also serializes metadata edits;
no extra generation-race mechanism was introduced.

All69 frontend cases pass. New-module client guards are measured by the selected
fault controls rather than a contrived missing-import baseline. One initial
fixture changed only a parent token for an instance; correcting it to the selected
instance token matches the producer's actual row authority. This was a test setup
correction, not a weakened production guard. Backend and UI diff/comment review
confirmed no input-cache/Notice cuts or saved configuration changes.

Actual phone/desktop acceptance passes952 owned commands in86.45s, including
module, button and absolute-instance toggles, native scope/readback, reopen and
restore, unsupported input, retained inventory/settings/values and held entry
reads across dismissal/navigation. All six feedback screenshots were inspected.
Artifacts: `/tmp/dali-browser-463f979e4e9f52771009d4789c440e96`; log:
`/tmp/dali-managed-feedback-browser.log`. No subsequent production UI mechanism
change was made while adding final literal assertions/control messages.

The sixth native case also passes: saved module/instance targets on a disabled
bus reject reads and activation and leave the independent indicator inactive.
All six pass in6.680s execution. Nine serial intentional faults each produce the
declared failing assertion (124s total): mixed-support preflight, collision,
closing presence, ignored write, final epoch, preserving button Notice, client
unsupported preflight, actual matching and post-await authority. The runner
restores each mutation; no readers/builds shared its mutable tree. Wait registry
generation is separate from final validation.
