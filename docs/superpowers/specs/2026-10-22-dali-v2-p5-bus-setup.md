# P5c: configure buses on a fresh installation

## Problem and selected shape

Confirmed in current code: `Inventory::empty` starts with no buses; `Edit` and
`MetadataEdit` provide rename but no creation/enable operation. Discovery and
assignment require a saved enabled bus. `Owner::run` passes enabled bus numbers
to `Hat::new` once; later model sync updates only input routes. This blocks the
approved initial commissioning workflow without hand-editing JSON.

Use the existing durable metadata owner, not a second configuration writer.
Alternatives are automatically creating all four buses (assumes connected ports
and changes fresh/migrated inventories), or restart after metadata edits (leaves
the browser workflow dependent on a process restart). Explicit setup plus live
poll-list sync preserves the current configuration contract. T3 because bus
renames change child bindings and enabling changes live routing. Standing mission
approval applies. This is a backend prerequisite for the approved UI, not a new
UI design. Design budget: two grounded self-review passes and two sequential
read-only gates; no implementation delegation or unrelated recovery work.

## Contract and durable semantics

Add `Edit::ConfigureBus { bus, name, enabled }` and matching `MetadataEdit`.
Upsert by physical bus index 0..3. A new bus has the supplied name/enabled flag,
no description and empty outputs/groups/inputs. Updating an existing bus changes
only name/enabled; description, saved addresses, input types, group memberships,
color selection and all child Origins remain intact. Existing ledger logic
rotates renamed child targets/bindings. Enable-only edits preserve child binding
tokens and Origins; the controller revision/token rotates on a real change.
An exact repeat is a no-op, with no revision/token rotation. Existing structure
validation rejects duplicate bus names, invalid names and out-of-range indexes.
No bus deletion, address assignment, device creation or hardware write occurs.

Controller WithReply command `configure_bus` accepts exactly `bus`, `name`,
`enabled`, `expected_revision`, all required, and no target. Integers/strings/
booleans are strict Serde types; unknown fields fail. Use existing metadata
deadline (no new wait), revision/controller-generation admission, global permit,
durable save, generation CAS, inventory publication and original MQTT command
completion. Metadata remains usable without UART or a healthy physical bus.
Confirmed save/CAS uncertainty follows existing freeze/recover; no new retry.

## Live HAT and cache synchronization

Synchronize enabled physical bus numbers with the committed model before polling
public frames and before returning a metadata result. Preserve the current
internal bus-0 health fallback when no bus is enabled; this does not create a
saved bus or authorize any fixture operation. Added/enabled buses schedule an
immediate health poll, without restarting the UART. Removed/disabled numbers stop
ordinary polling (except that fallback). An unchanged enabled set is a no-op.

Track the actual enabled set separately from the internal bus-0 fallback, so
enabling bus 0 from an empty configuration is still an enable transition.
On an enabled-set change invalidate affected cached evidence and increment its
existing observed-bus epoch. Cancel/discard any owned health/input-boundary helper
using the current join/discard machinery so an older helper result cannot restore
removed evidence. Do not abort physical work: the common permit excludes metadata
edits until physical work has settled. Keep the existing transaction cancellation,
late-terminal and quiet-period behavior unchanged. Re-enable requires new input
receive boundaries. Never infer device presence from enabling.

Health remains the existing best-effort HAT observation stream: an old public
status packet may arrive after a configuration edit and update bus health. This
slice does not add receive cutoffs or a new health freshness/admission framework.
Such a packet cannot revive old fixture values or input actions: those use the
retired cache/listener lifetime, and actual reads/writes query the fixture. A
rare bus outage may drop an ordinary command, as accepted by the owner. A new
nonzero bus with no observed status still waits for ordinary health polling.

Saved output/input observations and input listeners are retained only for enabled
buses. Disabled inputs have no live Router route; queued native events cannot
become actions on a disabled route. Re-enable must not revive old fixture State.
Every Notice carries its private captured physical stamp. The event helper takes
the ServiceHandle and, immediately before reporting, checks current Ready/Writing phase,
enabled bus, matching target generation, active input listener and matching stamp
in the latest input observation. This suppresses notices queued before disable
even after re-enable without token rotation. A newer event/query in the same
listener lifetime does not suppress legitimate ordered button actions. Events
already handed to the SDK retain its established delivery policy.
Writing still uses the active pre-commit model: an unrelated metadata save must
not discard an already queued valid action. Uncertain/stopping/closed phases
remain rejected; committed disabling retires the cache/listener before reply.
Input projection already checks enabled buses; output route resolution already
refuses disabled buses. Retiring cached values on disable is independent of
preserving identity tokens and saved hardware metadata.

The owner must sync the HAT after successful activation/recovery and update its
snapshot hardware/input caches before replying to the metadata caller. It also
keeps the existing top-of-loop sync. Inventory publication describes saved
configuration, not physical availability. Failure before commit leaves live bus
selection unchanged; uncertainty uses only the currently activated model until
existing explicit recovery adopts a confirmed candidate.

## Implementation and verification

1. Fail-first real MQTT test starting with an empty inventory: configure bus 2,
   original correlation/token reply, persisted empty named bus, subsequent real
   addressed discovery on bus 2 without restart. Private FakeBroker and PTY only.
2. Add edit/metadata/proxy operation and native model tests for all 0..3 indexes,
   strict validation, no-op, rename and enabled-only lifetime continuity, saved
   child metadata preservation and independent consumer continuity.
3. Add enabled-bus sync, cache/listener retirement and post-activation snapshot
   refresh. PTY tests demonstrate fresh nonzero-bus health, disable rejection,
   no replayed input action (hold the event helper across disable/re-enable),
   re-enable fresh boundary/readback, existing bus work still available, and
   an in-flight old health helper cannot restore its discarded result. Public
   asynchronous health packets retain their normal best-effort behavior.
4. Exercise held/failed/uncertain saves through existing hooks; before confirmed
   activation no live selection change, global permit remains held, recovery
   adopts configuration without programming devices. Existing metadata completion
   and uncertainty controls run as affected guards.
5. New behavioral mutation controls remove live sync, lose saved children, revive
   disabled caches, accept stale authority and lose original completion. Confirm
   each fails at its declared behavioral assertion, then restored native suite,
   strict scoped Clippy, Pi library/all four binaries, doctests, changed-file
   rustfmt, wait registry and diff review. No real UART, broker or Store touched.
6. Ship operator/API documentation, running guide/catalog pointers and progress
   evidence. Freeze implementation commit and run two sequential source reviews;
   disclose same-family review and absent physical HAT validation.

Self-review pass 1 checked the empty/startup/edit/proxy entry points and actual
dynamic polling dependency. Pass 2 checked disabled sibling paths, cache retention,
input boundaries, helper-result races, uncertainty/recovery and no-op revisions.
No additional owner decision is needed within the approved mission.
