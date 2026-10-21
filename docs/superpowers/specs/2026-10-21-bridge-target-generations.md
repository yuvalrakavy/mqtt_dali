# Target generations for mutable physical inventories

P3c2 implementation design within the approved DALI v2 architecture. T3: stale
retained intent must never operate replacement hardware. No second MQTT runtime.
The existing driver defaults and unfenced wire behavior remain unchanged.
The subsequent [command completion correction](2026-10-21-bridge-command-completion.md)
preserves only an exact active command's final completion through its own
successful inventory CAS; ordinary replacement keeps full old-work retirement.
This first opt-in fencing implementation supports fixed Bridge instances,
including managed startup, which is DALI's topology. Dynamic Host factories keep
their existing unfenced construction-window behavior; their description must
reject Fenced explicitly before worker launch or command-policy installation,
never downgrade it. Dynamic fencing is unsupported in this change. Context still
propagates through shared factory/lifecycle paths for optional driver methods.

## Verified paths and chosen shape

At a3db09e RequestMeta carries expiry/conflict only. Desired property retractions
skip request_meta; Config and Command do not preserve it. StateValue carries no
opaque context. Work::Apply groups by target. Worker::configure caches equality
by target/value; Worker::checked may resolve Satisfied before apply. The session's
built-in assume writes Reports directly. Reports removes current state on remove
but keeps broker read-back/bases separately. LinkContext constructs Reporter from
Reports alone. DriverInfo has public struct literals in consumers.

Passing a token only to apply would leave those acceptance/cache paths open.
Changing target spelling or adding another MQTT session would break the approved
contract. Use one opt-in, generation-aware runtime path: preserve context from
the original packet, validate before semantic acceptance, and scope observations
and cached knowledge to their original generation. An authoritative complete
generation table makes runtime validation nonblocking and independent of I/O.

## Public APIs and compatibility

Add RequestContext with optional target_generation: String. The runtime treats
it as opaque. MQTT v5 target-generation must occur exactly once when present and
contain 1..128 bytes in 0x21..0x7e. Duplicate, empty or malformed values are
malformed even on empty retractions. Unfenced requests may omit it. Deadline and
conflict remain their existing fields and retain their original semantics.
Use a separate wire::request_context decoder: Desired request_meta delegates to
it; Config/Command/read-back decode context without inheriting Desired conflict
validation or expiry semantics. Config remains non-expiring.

Add a provided DeviceDriver::request_policy(), default Unfenced. Fenced carries
a complete TargetGenerations value: an instance generation and a map of target
names to generations. Generations obey the same string bound, targets the normal
topic-segment rule. The driver must never reuse a retired generation for a target.
The instance generation covers commands without a target. Description-only DALI
changes rotate the instance generation, leaving unchanged item generations alone.

Capture policy with info exactly once on the existing description path. Preserve
its actual guarantees: synchronous Bridge::start describes inline under the
caller's control; Bridge::run describes off-thread raced with stop signals, with
no new claim of a finite description bound; managed ready is inside the existing
service startup/watchdog bound; factory description keeps its existing bound.
Use a private DriverDescription wrapper through launch/lifecycle; do not add a
required field to DriverInfo or a second description call. Install the initial
table before the instance can execute or publish State.

Provide apply_with_context, execute_with_context and configure_with_context,
whose defaults call the existing methods. Provide validate_request(target,
context, kind), default allow, for additional pure driver validation. Its kind
distinguishes Desired values, Config and command. Run it inside the ordinary
operation clock/deadline before any cached acceptance or outage resolution; it
must be quick and perform no I/O. Runtime generation validation is independent
of this hook. DALI additionally checks its shared service's admission phase.

Provide read_state_with_context returning contextual TargetState records; the
default wraps existing read_state results with no generation. Add Reporter state
and event methods accepting RequestContext. Existing methods use absent context.
In fenced mode, missing or mismatched context cannot enter current observed
State or caches. Do not stamp the current token onto a late unversioned result.
Probe still proves the device link; it does not retoken observations.
Its default delegates to read_state_with_context(Scope::Any).await.map(drop),
whose default still reaches legacy read_state. This supports contextual-only
reads without a false Unsupported probe violation. Proof applies only to
surviving original-context unproven entries.
Also provide remove_with_context and remove_property_with_context. In fenced
mode their absent/retired context has no effect: an old helper cannot erase fresh
state. Existing removal methods use absent context. These ordinary removals are
distinct from the authoritative exact-old cleanup performed by table replacement.
For events, a string target selects that target's table entry; no target selects
the instance generation. Invalid target types are a driver contract violation.
DALI includes __inventory explicitly in its target table with the instance
generation, so its State passes the normal target lookup.

Existing DeviceDriver implementations need no new required methods. Internal
mailbox/wire support structs gain context where necessary; this does not promise
source compatibility for their exhaustive struct literals. Update in-tree tools
and fixtures. Default unfenced conformance remains the compatibility proof.

## Admission, dispatch and retractions

Parse context before processing Desired, Config and Command, including empty
payloads. Each pending/in-flight/unproven/re-held value, config, command and read
caller retains its own context. Never replace a token with the current table's
token. Property-copy suppression and config equality include context. Group an
apply only across the same target and context. Read coalescing must not merge
callers with different contexts where execute/replies depend on that context.

For fenced mutation packets, check the table's exact generation and mutate the
mailbox in one ordered critical section: mailbox lock, then Reports lock. Table
replacement uses the same order. A separate check before posting is insufficient:
G1 must not pass validation, pause across G2 replacement/cleanup, then displace
fresh G2 work. The authoritative post validates while holding mailbox ownership;
no I/O, driver callback, logging or await occurs under either lock. Unknown
targets and missing/mismatched generations are refused as stale; never fall back
to an address. Recheck in the worker before conflict resolution,
Satisfied, cached Config success and actual driver dispatch. An allowed driver
callback cannot override a failed generic check. Read-only refresh and commands
declared Read need no token; a supplied token must still be well-formed.

The built-in assume is refused for fenced instances. Their initial policy is
always known before launch; there is no supported fenced policy-unknown window.
Dynamic construction remains unfenced and preserves its existing request/assume
behavior. A factory returning Fenced fails before its driver can connect.
Metadata mutability is checked by the driver's validation hook before accepting
Desired/Config/commands; DALI's __inventory is read-only. Refresh remains allowed.

A Desired retraction cancels only intent/assumptions from its own generation.
An old generation may cancel its own old pending intent, but cannot settle or
cancel a newer generation's write. Missing/malformed context cannot retract
fenced intent. Config retractions follow the same pending-context rule; a driver
configure(None) still requires the current mapping generation. Fenced input
cleanup must never infer a token from the latest pending value.

Rejected stale intent uses the existing refused Error/reply grammar and preserves
link health. Driver validation errors use existing operation error classes. Do not
reconnect or retry a momentary command merely because its mapping changed.

## Generation replacement and runtime retirement

Reporter::replace_generations(expected, next) compares the complete expected
table, validates next, and atomically installs it with Reports retirement. It is
available only for opted-in fenced instances. No transition back to Unfenced.
The service serializes replacements; expected-table comparison rejects stale
owners. Retain a weak mailbox binding in Reports/Reporter so retirement does not
create an ownership cycle. Acquire mailbox first, then Reports, install the table
and retire queued work before releasing mailbox ownership. Ordinary observation
merges need only Reports. Never acquire mailbox while holding Reports: existing
withdrawal already takes mailbox then Reports, so the reverse order is unsafe.

For each removed/changed target generation, clear its current reports, synthetic
written knowledge, assumptions/unproven values, startup read-back (including the
frozen copy) and broker-loss bases. Clear generation-scoped worker configuration
and conflict caches before they can be used; key cached values by context as an
independent guard. Request bases captured earlier stay with their original
context, so they cannot become bases for a fresh generation.

Prune pending/in-flight/re-held/unproven/config work for the retired generations,
with normal refused replies for commands. Retirement cleanup names the exact old
generation, not "everything unequal to next": that cannot accidentally remove a
newer request if two table replacements and their cleanup interleave. Internal
retirement delivery wakes the worker even during a device outage. It performs no
hardware I/O. A work item already in a driver call cannot be undone; its result
may update runtime knowledge only if its original context is still current.

Observation validation and merge occur under the same Reports lock as table
replacement. Startup broker State read-back retains its packet context and is
accepted only for the matching table entry in fenced mode; the supported fixed
topology installs the table before subscribing. No broker State tagged with a
retired generation becomes a new conflict basis. Unfenced factory construction
does not gain a new quarantine buffer or new preconstruction admission policy.
Filtering applies to both catch_up's direct proving map and decide's direct
current-value calculation, not just fold/Reports. Revalidate the request's
original generation after awaited reads before Satisfied or conflict decisions.

Lifecycle Config installed/replayed bookkeeping carries the accepted original
RequestContext, not target-only sets. A rejected/malformed/stale Config packet
does not change that bookkeeping. Replay-absence synthetic configure(None)
uses the originally installed context; it never manufactures the current token.
Have session routing return validated Config admission information so lifecycle
updates occur after validation. Validation here means wire/table validation and successful mailbox admission;
it does not await driver-hook or hardware acceptance. A later driver rejection
is still an admitted Config, matching existing delivery-level bookkeeping.
Retraction admission distinguishes rejection from "no pending value", so a
failed retraction cannot accidentally settle an assumption.
Retained State Seen records preserve parsed
context, including invalid/absent distinctions, through disabled-instance
re-enable and the direct lifecycle read-back route.

Every fenced State/event publication, request Error and command reply carries
target-generation from its original record, including reconnect replay and
already-sealed batches. A late terminal Error must not settle or cancel a new
mapping's intent at a consumer. Extend internal
snapshot/change/output records, never look up a fresh token at publish time.
Retirement retractions identify their retired generation. Queued old observations
may remain tagged old; consumers filter them against complete accepted inventory.
Status/Active/version retain their current grammar. Existing via/traceparent and
response correlation properties continue through the one publication path.

## DALI service ordering

The service owns routing admission separately from ConfigStore's persistence
snapshot. Before a mapping mutation, freeze service writes and serialize against
hardware jobs. Keep the previous confirmed routing model during the save. On
uncertainty stay frozen, including through late filesystem completion/recovery.

After a confirmed durable save/recovery, install runtime generations and retire
old runtime knowledge, report the complete inventory with its instance generation,
then expose the new service routing model as Ready. Old read responses keep
their old tokens and are dropped. If runtime retirement fails, remain frozen;
do not route through a half-installed inventory. A restart reconstructs the
initial generation table from the durable Model before MQTT admission.

The proxy/service rechecks context atomically with resolving the current address
when scheduling hardware work, and again at dispatch after a queued job. No
generic early check replaces this final service check. Store consumers migrate
only their own intent through the proven named history, as already designed.

## Implementation and verification

1. Context codec and provided driver APIs; private description propagation through
   fixed, managed and factory startup. Unit tests first for malformed/duplicate
   metadata, optional legacy behavior and policy validation/fenced assume.
   Prove a factory returning Fenced is rejected before connect (record the call),
   fixed managed policy installs before admission, and unfenced construction
   behavior remains compatible.
2. Preserve context throughout mailbox, conflict/dedup/re-hold paths. Tests for
   same-value/different-generation copy, mixed-generation grouping, empty stale
   retraction against a new write and cached-config/outage Satisfied bypasses.
   Force the G1 validation/post versus G2 replacement interleaving and prove that
   fresh G2 work survives. Verify contextual target/property removals too.
3. Reports generation tables, contextual observations/read-back/output, retirement
   and cache invalidation. Test lock-order boundaries, exact-old cleanup, stale
   in-flight results, reconnect snapshots and already-sealed publications.
4. Local FakeBroker conformance with a mutable fenced driver: queue across outage,
   rename away/back, remove/recreate same name/address, group-semantic change,
   delayed old State/Config/Desired/retractions/events and valid fresh commands.
   Verify actual driver calls and wire v5 properties, not only runtime maps.
5. Run existing default unfenced kit + black-box conformance, compiled tutorial
   examples, strict Clippy, Pi library check and reproducible negative controls.
   Update runtime spec, README, tutorial and developing-mqtt-bridges guidance in
   the same feature. Actual Store persisted reconciliation remains DALI P8.

Design self-review round 1 followed actual producer/consumer paths: found the
direct assume path, retraction metadata bypass, Config cache and Satisfied path.
Round 2 checked startup/factory timing, old result publication, lock ordering,
frozen read-back and interleaved retirement cleanup. The choices above close
those paths; fresh design review precedes production edits. Power-loss/physical
HAT validation is outside this shared runtime scope.

Sequential same-family design reviews completed. The first requested atomic
admission, contextual removals and lifecycle metadata coverage; these are now
specified. The second approved test-first implementation after clarifying direct
conflict-read filtering, delivery-level Config bookkeeping and contextual default
probe delegation. Cross-family review is unavailable. No production source was
changed before these reviews.

## Implementation verification (2026-10-21)

The shared runtime boundary is implemented on `feat/dali-managed-services`.
The full kit gate passed 194 tests, including its compiled tutorial examples and
wait registry. The existing black-box conformance gate passed all 174 tests with
explicit `RUST_LOG=info` (the inherited warn filter hides INFO assertions).
All-target Clippy with warnings denied, three rustdoc tests and the
`armv7-unknown-linux-musleabihf` library compile check passed.

Sixteen reproducible negative controls in
`mqtt-bridge-kit/docs/generation-controls.toml` each failed at its declared
behavior assertion; compile failures are not counted. The runner restored each
source and recorded its ledger. They cover generation authority, admission/post
ordering, mixed-context grouping/copies, driver validation before cache/conflict
acceptance, direct/proving-read filtering, sealed output metadata, latest-only
Config caching, late state/removal, retirement/CAS, and legacy retry/read/merge
compatibility.

Fresh sequential source reviews approved the result. The first found four
compatibility regressions: duplicate initial policy installation after a failed
thread start, optional-token legacy read filtering, obsolete per-context Config
cache entries, and erased unfenced partial state. All four were reproduced by
failing regressions and fixed before reapproval. The second found no further
blocking issue. Both reviews were source-only and from the same model family;
independent model-family coverage is unavailable.

The admission race test pauses immediately after the authority check and asserts
that fresh intent survives a concurrent table replacement. Its 200 ms scheduling
window is controlled coverage, not an exhaustive interleaving proof; moving the
check outside mailbox ownership failed the intended assertion in its negative
control. No production test seam is compiled for this scheduling hook.

The runtime spec, README and tutorial describe the optional APIs. User-level
bridge-skill guidance records their development-only status. Remote Claude
artifact mirrors were not edited from this session. No release, push, merge,
deployment, live broker/HAT test or downstream dependency adoption is included
in this boundary. DALI service ordering and actual Store persisted migration
remain their respective implementation phases.
