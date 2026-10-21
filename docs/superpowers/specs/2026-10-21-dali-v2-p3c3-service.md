# P3c3: DALI service ownership and routing transitions

Implements the already approved architecture. No new user-owned product choice
is needed. The shared managed-service and generation boundaries are committed
at tracing-init `273062f05c1f0df05cb35f19bad458f2691d8dea`. This contract grounds
the next implementation in the actual ConfigStore, UART and runtime APIs.

## Verified infrastructure and constraints

- `ConfigStore::open/edit/recover/close` owns a runtime blocking worker, private
  lifetime lock, reserved revision high water and recognized recovery candidates.
  Its snapshot may become Ready before service routing is safe to expose.
- `Model` supplies complete Inventory State, projections, epoch and revision.
  A target token is `epoch.binding_revision`; controller and `__inventory` use
  `epoch.revision`. Names remain public identities, with no additional ID.
- BridgeKit fixed `Bridge` requires an explicit instance. The daemon will require
  `--instance` and verify it matches the loaded/migrated controller name. This
  avoids inventing a second controller identity or doing filesystem I/O before
  the runtime's signal/logging startup. New inventory uses that instance name.
  Check-only migration keeps its existing CLI compatibility.
- `ServiceContext::ready(proxy)` transfers the proxy before MQTT/device startup.
  A Reporter first becomes available through the proxy's `connect` LinkContext.
  Attach that Reporter to the service before attempting HAT connection; this
  handshake is independent of broker availability. Configuration mutations stay
  unavailable during the brief pre-attachment phase. Read-only inventory remains
  accessible. Reconnect attachment is idempotent within this fixed runtime.
- `Transactions` owns the continuous monitor and exclusive HAT exchanges. It
  exposes typed events/status and bounded initialize/execute/close operations.
  Do not create another UART reader or expose a raw monitor to frontends.
- `Serial::open` currently uses a detached OS thread. Before managed UART
  integration, move that opener to runtime-owned blocking work so successful
  managed teardown cannot ignore a still-running resource opener. Keep one
  outstanding opener: an elapsed caller deadline cannot start another opener
  while the first still owns blocking work. A timed-out opener is terminal for
  that UART owner until its actual completion.
- Retained State replay order is not an inventory barrier. Runtime model replay
  sorts target names, so `Bus1...` can precede `__inventory`; sealed/retired output
  can interleave too. Consumers must handle state arriving before its matching
  complete inventory without materializing an unproven device.

## Implementation slices

P3c3a first adds the inventory/routing owner and proxy attachment boundary. It
does not pretend to implement missing hardware capabilities. Follow with P3c3b
UART ownership/readback scheduling and P3c3c canonical daemon/CLI/process tests.
P4 adds the capability families, P5 commissioning jobs, P6/P7 HTTP/browser, and P8
actual Store persistence migration. Do not label P3 or the full bridge finished
after P3c3a alone.

Files expected in P3c3a: `Cargo.toml`/lock, `src/lib.rs`, new `src/service.rs` and
`src/service/{routing,proxy,tests}.rs` as useful, `src/config_store.rs` only for a
test-scoped hook adapter, independent consumer fixture/tests, inventory guide,
wait registry, control table/ledger and progress record. A small pure adapter
from Model projections to BridgeKit generations belongs to the service boundary.
Keep legacy binary/modules untouched.

Pin all tracing-init workspace dependencies to the same reviewed revision,
including BridgeKit and test tools. Local Cargo cache population may fetch that
commit from the local shared checkout for offline development. Record that it
is not fetchable from upstream until publication; do not claim a portable fresh
clone/release before that is resolved. No publication is authorized by this
contract slice.

## Single service owner and frontend access

Construct `DaliService` without resources. Its managed `run` opens ConfigStore
under startup/cancellation bounds, validates controller identity, builds initial
routing authority, hands off a resource-free proxy, then owns all work until
cancellation. P3c3b adds HAT resources inside that same runtime.

The service owns an immutable Arc<Model> plus a distinct routing phase:
AwaitingRuntime, Ready, Writing, Uncertain, Stopping or Closed. Configuration
readiness and hardware availability are separate facts. Hardware loss cannot
destroy saved inventory or stop the configuration service. Proxy connectivity
means the service is available, independently of HAT connection; otherwise the
runtime would refuse metadata commands during hardware loss. Hardware commands
check hardware availability separately.

Public handles clone owned snapshots; no watch/mutex borrow escapes. Update
notifications use bounded broadcast with explicit lag/resnapshot behavior.
Snapshots include controller/build identity and truthful phase/link facts;
credentials never enter exported inventory or logs.

Mutation requests use nonwaiting bounded admission and one exclusive service
operation permit, retained through the active operation. Each original request
carries an absolute deadline, cancellation/reply state and original context.
Actor dispatch rechecks phase/revision/context. The actor serializes routing
mutations with actual address resolution and hardware scheduling. A permitted
request waiting behind a later hardware job is checked again before dispatch.

The initial public administration methods are safe metadata edits: rename an
output/group/bus and set the controller description, plus explicit recovery.
They use the existing Edit reducer and expected revision. Generic physical
replacement/readdress/member edits remain an internal confirmed-job boundary;
frontends cannot claim hardware success by writing configuration alone. P5 must
perform and verify those physical operations before committing their mapping.

The proxy uses the same typed service requests as future HTTP handlers. Its pure
validation reads only the service-owned snapshot. `__inventory` is read-only.
Until implemented, hardware properties/operations return explicit Unsupported;
no guessed success, assumption, inferred address or permanent held pseudo-job.

## Durable edit and recovery ordering

1. Validate expected revision, edit kind, deadline and service admission. Serialize
   against active hardware operations. Freeze routing while retaining the prior
   confirmed model and original generation table.
2. Await ConfigStore edit under the original absolute deadline. A canceled or
   expired caller before work begins causes no edit. Once saving starts,
   cancellation/deadline is an uncertain outcome unless durability already won;
   it cannot make routing Ready independently of the owner.
3. NotCommitted may restore Ready only when ConfigStore confirms the recognized
   prior public mapping and Ready phase. A private high-water reservation can
   advance without changing public mapping. Any ambiguity remains Uncertain.
4. For confirmed changed mapping, build the complete next table and call
   `reporter.replace_generations(expected, next)`. Require success. Publish the
   complete Inventory through the same Reporter with its instance token. Only
   then expose the new service model/Ready phase. This orders queueing and
   routing; it does not promise broker acknowledgement or retained replay order.
   For a momentary MQTT command committing this same change, capture the runtime's
   exact `CommandCompletion` only inside contextual execute, forward it explicitly
   to this owner, and use `replace_generations_for_command` for this activation.
   That single-use capability preserves only its final original-context reply;
   queued/unrelated retired work still fails. HTTP edits carry no capability and
   use ordinary replacement. Stop or a stale/used capability fails the CAS before
   authority changes. Never preserve every active old command by generation.
5. Uncertain saves remain frozen even if the filesystem worker later finishes or
   a ConfigStore snapshot changes. Explicit recovery waits for recognized,
   durable old/reserved/proposed bytes, then runs the same retirement/publication
   sequence before Ready. Unknown external files are never adopted.
   Recovery's `Committed { changed: false }` reports that recovery itself made
   no new logical edit; the recognized recovered mapping may still differ from
   the prior routing model. Compare full public models/tables, never gate runtime
   retirement or publication on that boolean.
6. Runtime retirement failure is distinct from disk uncertainty: retain the
   durable candidate and stay frozen. Never start another configuration edit
   through half-installed authority. Explicit recovery may retry only the exact
   original expected-table CAS with that same durable candidate. The Reporter
   has no authority getter; do not discover/adopt a conflicting authority or
   use disk recovery alone to restore Ready. A persistent mismatch requires
   restart. A stale owner cannot manufacture success.

On shutdown close service admission first, settle/refuse pending callers, stop
UART helpers and close ConfigStore within the original managed component ceiling.
Returning success requires actual owner teardown. A blocked filesystem/opener
must keep runtime destruction incomplete so managed stop returns false at its
parent bound. ServiceContext exposes cancellation, not the parent deadline;
local cleanup budgets are bounded but parent forced cancellation remains the
final ceiling. Do not claim the service reads an unavailable absolute deadline.
Do not wait for MQTT to close and do not detach resource owners.

## Independent consumer replay handling

Extend the independent consumer example to keep observed values separately from
its owned intent. A bounded pending-observation cache can hold the latest State
per target/context until the matching complete inventory arrives. Validate wire
shape, target/token and byte/item limits (including key bytes) before caching.
No unknown observation
creates a device, changes persistent identity, or migrates a request token.

After accepting a whole inventory, apply only observations matching its exact
epoch/binding revision. Clear current observation on mapping-version change;
never carry it to a replacement. Older/malformed/conflicting inventory leaves
both accepted devices and observations unchanged. Old retractions clear only
their own context in both pending and current observations and cannot erase a
newer value. Inventory acceptance distinguishes Applied, Unchanged and Ignored;
an ignored older inventory returning success cannot trigger buffer draining.
New epochs
still require explicit adoption; this cache cannot adopt an epoch implicitly.
Document deterministic bounded eviction as unknown-state loss requiring a read,
not as a default off value or evidence of physical state. Events cannot be
replayed as button actions after a later mapping acceptance.

P4/P5 must additionally freeze the UART receive cutover rule for input events:
queued raw frames must not acquire a replacement's fresh token merely because
they are decoded after an inventory change. The monitor has sequence counters
and receive-fence machinery; verify its draining/partial-line guarantees before
using a cutover fence. This slice does not yet expose decoded input actions.

## Verification and completion boundaries

Use failing tests first for pure table/token mapping and buffered State-before-
Inventory, malformed/older/equal/new-epoch handling, old-context retractions and
bounded cache eviction. Include literal external wire fixtures.

Service tests exercise the real ConfigStore and BridgeKit Reporter boundary:
initial attachment and identity mismatch; delayed save and immediate phase
rejection; same mapping/no-op/description versus rename/replacement/group-meaning
tokens; failure before reservation/after rename/at directory sync; canceled
caller, late completion and explicit recovery; runtime-CAS failure; HAT/broker
outage independence and actual bounded shutdown. Prefer existing filesystem
hooks exposed only to crate tests over another persistence implementation.

Use local ephemeral FakeBroker and PTY emulator only. Verify actual consumed
generation metadata and preserved independent consumer records/intent. Add
guard-removal controls for the service phase boundary, recovery ordering and
consumer buffering/filtering. Source reviews precede production edits; freeze
source during sequential core reviews and negative controls. Run native tests,
scoped strict Clippy, wait registry, Pi compile checks and relevant existing
migration/UART/emulator tests. Document supported versus pending behavior and
sync runtime/consumer guidance and applicable skills in the same phase.

P3c3a completion does not establish physical HAT compatibility, live Logmon
delivery, browser behavior, Store migration, startup process gates or a release.
Those remain explicit later acceptance work.

## Integration review corrections

Two sequential design reviews approved the source-grounded slice. The first
implementation review found three reproduced gaps: self-retirement of a successful
MQTT edit reply, accepting a ready response resumed after its original deadline,
and failing to compare LinkContext's actual instance with persisted identity.
All three were observed failing before fixes. The first required the separately
reviewed shared command-completion extension above; it does not add a wire field.
The latter two are service-local boundary checks. Final verification and code
review records follow when the corrected dependency is adopted.

P3c3a now adopts `259d963fd448ea2512e36a00e2f22850af0bc1f7` consistently in
Cargo.toml/lock. The full native suite passes 201 tests, scoped strict Clippy and
the armv7 library/v2/emulator compile pass, and the wait registry is current.
Nine service/consumer controls fail at their intended assertions. Both sequential
source reviewers approved the corrected boundary; MQTT recovery after canceled
save additionally checks original token/correlation and once-only completion.
The source review and controlled timing tests are not exhaustive concurrency or
physical hardware proof. UART/daemon integration remains the next slice.
