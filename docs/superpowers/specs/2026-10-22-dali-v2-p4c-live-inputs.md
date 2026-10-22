# P4c3: live input state and native MQTT events

Tier T3: externally binding state/event semantics and inventory generation
cutover. P4a stays closed. This implements the approved input architecture using
the existing single UART reader, HAT owner, saved Inputs/Instances and BridgeKit
worker. No new outage-retry policy, SDK extension or Store change. Budget about
45 minutes for implementation and focused verification; surface a real blocker
promptly rather than extending the work silently.

## Problem, evidence and chosen shape

The codec and named input metadata exist, but Hat::event still ignores L data.
Snapshot and the MQTT helper currently publish only output observations. An
existing emulator Control::InputEvent already injects literal monitored L frames.
Connect those paths. A second raw reader would compete for HAT replies; forwarding
only raw frames would require each consumer to reimplement DALI. Reuse the
owned HAT helper and LinkContext-owned reporter helpers.

Add service/input.rs and focused tests, export input Observation/Notice and add
Snapshot.inputs plus ServiceHandle::subscribe_inputs. The internal Router keeps
current enabled input-instance routes and original generation contexts.
Snapshot.input_listeners exposes installed route boundaries for diagnostics and
deterministic normal-cutover tests; listening is not proof of device presence.
Resolve
device/instance exactly; resolve device/type only when the saved module has one
matching instance. Ambiguous or unassociated sources do not acquire a guessed
target. Instance/type and group sources remain unassociated until their missing
identity/membership evidence is implemented. Preserve raw unknown typed data.

## Mapping cutover using existing receive fences

The earlier service spec explicitly requires old queued raw frames never acquire
a renamed/replacement endpoint's new token. This matters in ordinary inline
renaming, not just transport failures. Monitor::receive_fence_until already drains
readable bytes and returns minimum_sequence excluding a partial line begun before
the boundary (ReadMode::process adds two for an incomplete line). Reuse it; do
not alter UART recovery or discard/reset the parser.

New/changed input routes start pending. Unchanged target/token routes retain their
boundary and observations. Add a crate-private Transactions receive-event-boundary
method using existing exclusive transaction admission, Ready checks, original
deadline and monitor fence. Add one HAT helper kind to obtain that boundary with
a 500 ms ceiling, using the existing owned job/join and paced retry timer. Output
work retains existing priority; metadata save/activation never waits for UART.
No input boundary work is created when no enabled input routes need it.

The job captures exact target/token routes plus UART/bus stamps. Install a result
only for still-matching routes with the same live stamp. Frames below the minimum
sequence, pending routes, stale stamps, disabled buses and unresolved identities
cannot publish a named action. Boot/lag/reopen invalidates the stamps and needs a
fresh boundary before input publication. No event is re-tokened after enqueue.
This conservative cutover may discard input actions during mapping activation;
they are never replayed as another mapping's actions. Existing unaffected routes
continue normally. Busy/failed boundary retries use the existing paced owner;
neither metadata nor healthy output operation is made dependent on input readiness.

## Current state and action wire contract

Instances use State/<controller>/<qualified-instance>. All values are observed;
Desired stays unsupported. Publish Available and Quality (unobserved, current,
partial, unsupported or stale). Saved metadata alone never implies availability.
Recognized Pressed/Released use Available=true, Quality=current and Boolean
Status. Other recognized button actions use Available=true, Quality=partial and
Status=null. Unknown native data uses Available=false, Quality=unsupported and
null type-specific fields. Stale/unobserved also use Available=false and nulls.
Button Status is nullable: true only from native Pressed, false only from native
Released, otherwise null. Native action-only events do not invent a held/released
state or leave a previous press presented as fresh. Absolute inputs expose
PositionRaw and light sensors LightLevelRaw, both native 10-bit event fields;
no guessed Boolean, normalization, full-resolution value or lux conversion.
Occupancy exposes Occupied and Movement. Unsupported data retains
EventInformation and has Available false/Quality unsupported. Stale/unobserved
values publish null for type-specific fields, preventing merged State from
presenting an old button press as current.

Every resolved native event publishes one non-retained Event/<controller> object:
kind=dali_input, target, instance_type, frame and information; recognized buttons
also have action (released, pressed, short_press, double_press, long_press_start,
long_press_repeat, long_press_stop, free, stuck). Preserve these native actions;
there is no new gesture recognizer. Sensor events also carry position_raw,
light_level_raw or decoded occupancy occupied/movement/repeated/sensor fields.
Unknown events carry their raw data
without a fabricated button action. Consumers must filter the intended action.

Carry original target generation and authoritative absent trace context from
reception through the queue to Reporter::event_with_context. Source state uses
the same context. SDK retirement is authoritative for already queued old actions.
One bounded service broadcast (64 notices) separates actions from watch snapshots;
snapshot coalescing must not lose two presses. The HAT's transient notice batch
is capped at monitor EVENT_CAPACITY (128), dropping oldest with an explicit count.
The worker reports lag/loss as a structured diagnostic, never reconstructs actions
from retained State. Device-link helper destruction drops its receiver; creating
a new receiver does not reconstruct past actions. Broker disconnect is different:
the existing SDK can retain bounded queued events and deliver them after broker
reconnect. Preserve that SDK policy; do not promise broker-offline events are
discarded, add another outbox, or extend SDK scope. Only the SDK worker calls
Reporter. Actions are non-retained and never reconstructed from State.

Input target refresh returns the latest observed projection, with explicit
unknown/stale quality; this boundary performs no physical value query. Aggregate
reads include these projections alongside existing output readbacks. Input module
targets remain inventory metadata only. Hardware discovery, full-value polling,
event configuration/feedback and semantic emulator controls are subsequent work.

## Implementation and proof

Change service.rs, owner.rs, hat.rs, proxy.rs and proxy/state.rs; add input.rs and
its tests plus real PTY/FakeBroker integration tests. Add the narrow receive
boundary method to atx/transactions/engine.rs, reusing monitor fences. Add tests
first for routing ambiguity, native state/action semantics and mapping cutover;
use independent literal L vectors and the existing emulator/raw PTY controls.

Prove actual MQTT pressed/released state and all native actions, two successive
actions without watch coalescing loss, raw sensor fields/no fake lux, unknown
starts/nulls, source commands ignored, events while an output command is active,
current source traces absent, input refresh with no extra UART request, rename
fencing including an old partial line, nullable Pressed-to-ShortPress/unknown
transitions and no reconstruction from State on reconnect. Keep the tests
proportional to these normal feature paths; no new UART fault campaign.

Run focused tests and meaningful controls, native full/restored suite, strict
scoped Clippy, Pi compilation, format/whitespace/wait registry. Freeze source for
sequential reviews. Update input/operator/contributor guides, support matrix and
progress with implemented and still-pending behavior. No shared skill change is
needed for using existing SDK events/fences; document local event semantics here.

## Self-review

Pass one traced the existing L parser, HAT receive draining and actual helper
ownership, metadata save/activation, projection helper, SDK event API and emulator
control entry point. Pass two checked opposite cases: unknown versus false;
explicit actions versus current state; ambiguous type sources; old partial lines
versus post-boundary frames; unchanged routes versus renamed ones; old fence
completion after another edit; and disconnected action queues versus retained
observations. The stamped per-route boundary and explicit null properties above
address these cases without changing P4a recovery. Scope claims are local emulator
coverage, not physical DALI certification.

## Review and implementation evidence

Both sequential source-grounded design reviews completed. The first identified
that broker reconnect retains bounded SDK events and required an explicit nullable
state/availability table. Both amendments are above; the second reviewer approved
the design. Neither review executed tests, and both use the same model family.

The implementation follows the listed ownership and data paths. Three API-skeleton
tests failed before behavior existed. Ten new tests now include the actual PTY
emulator/FakeBroker and an independent literal UART HAT that withholds an output
acknowledgement while emitting input events. The held-All regression observes a
new button state before releasing the old read. Input listeners expose boundary
readiness as a library diagnostic without claiming device presence.

All eleven focused behavioral controls and six affected existing owned-helper
controls fail at their intended assertions; the restored gate passes 324 native
tests, strict scoped Clippy, Pi library/all four binaries, formatting, whitespace,
wait registry and doctests (zero). Initial integration diagnosed/fixed only the
test peer's retained-slave capture lifetime. The initial full gate found wait
classification/tag placement, fixed through the established local-method rule;
there is one new actual wait, the LinkContext-owned event broadcast receiver.
No production broker/bus, SDK code or Store state was touched. Both sequential
read-only source reviews approved frozen `c0951fb` with no material finding after
the parent's complete diff review. They inspected cutover, context propagation,
nullable merged State, worker ownership, aggregate ordering and test sensitivity.
Neither ran tests or controls; both use the same model family. Physical-device
validation remains outstanding. The held-acknowledgement and partial-line tests
proved useful and should remain the pattern for future capability integration.
