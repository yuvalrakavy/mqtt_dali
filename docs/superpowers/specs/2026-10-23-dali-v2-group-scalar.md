# Hardware group Desired and observed State

## Problem and existing evidence

The accepted native group operation provides commands and protected web controls,
but normal BridgeKit Desired/State still rejects saved group targets. Logical
split-channel RGB equipment needs three independently bindable outputs or groups;
MQTT consumers should use the same PowerState/Brightness convention for either.
This slice adds base scalar group bindings. DT8 group colour and Store/PanelScript
remain separate. No configuration format, public identity or topic grammar changes.

Current source has one validated gear Intent, a concrete group owner operation,
full saved-roster raw aggregation and native group selection. The proxy already
owns strict complete-map validation, contextual publication, fair bounded All
reads and a snapshot watcher. Group reports distinguish actual uniform/mixed/
empty/partial state and preserve original membership/mapping authority. Reuse
those mechanisms. Memory recall found the shared BridgeKit/generic-consumer
decisions; current pinned SDK and bridge source govern actual behavior.

Three shapes were considered: infer groups from saved membership and independent
cached levels; send scalar writes as individually addressed fanout; or adapt the
accepted native operation and retain its validated actual observation. Choose the
last. The first loses actual membership evidence and the second changes native
hardware-group semantics. Retaining one observation per saved group is sufficient;
no new transport, worker, polling task or generic request framework is needed.

## MQTT contract

For target Bus1.Groups.Kitchen, Desired supports only boolean PowerState and
finite normalized Brightness0..1, per-property or atomic target map. Use existing
Intent validation, including contradictory-pair rejection. Empty/null/wrong-type,
unknown/read-only and all colour fields reject the complete map before mutation.
Writes require the original group's target-generation. The executing driver
captures the current revision once and passes it to the existing strict typed
Set. A revision race rejects; never fetch a new token or silently retry.

Refresh of a group uses the native read. Omitted generation captures current
authority once; supplied generation must match. Set sends exactly one group
OFF/MAX/DAPC and reads actual full-roster evidence, just like the accepted command.
No averaging, inferred membership, requested-value echo or automatic retry.

Group State has PowerState, Brightness, PowerMixed, BrightnessMixed, Available,
Quality, Coverage, KnownMembers and MembershipMatches. Coverage is always
saved_inventory: native writes can affect other physical members too.

| Native aggregate | Available / Quality | Values |
|---|---|---|
| uniform | true / current | Actual boolean and normalized level; mixed flags false |
| mixed, complete nonempty | true / current | Each disagreeing scalar is null with its own mixed flag true; uniform power can coexist with mixed brightness |
| empty | false / empty | Scalar/mixed values null; KnownMembers0 |
| partial or unavailable | false / corresponding quality | Scalar/mixed values null; fresh KnownMembers is a lower bound |
| invalidated or not sampled | false / stale or unobserved | All scalar/mixed/count/comparison values null |

All nullable fields are published explicitly so BridgeKit's merged retained State
cannot preserve a misleading old level/power/comparison. Complete membership may
still yield MembershipMatches even if level evidence is partial. Available means
complete nonempty group evidence, not uniformity or absence of fixture faults;
consumers check each scalar for null/mixed. Individual-output semantics stay intact.

Scalar writes publish accepted actual State before successful return. Complete
mixed evidence is valid actual state, including an ignored/clamped write. Incomplete,
empty or unconfirmed post-write evidence publishes its truthful quality but returns
uncertainty, following the existing individual-output convention. Native command
Report outcomes remain unchanged. Read can return partial/empty observed State.

## Observation ownership and invalidation

Extend Snapshot with group observations keyed by saved group target. Retain the
accepted report through an Arc plus original context/stamp/stale flag, avoiding
deep copies of a full roster on each ordinary snapshot. Pure Reports/Members/
Values gain equality for exact final acceptance; public typed methods still
return Report. A private Reply carries either the accepted Observation or the
original unconfirmed Report. The owner accepts once and caches the same
observation before waking the caller. Existing native command/typed wrappers
extract Report; scalar operations retain Observation with its original stamp
and context. Failed/fallback reports never install current cache evidence.

Retention checks the continuing group token and enabled bus. Any root revision
change permanently stales retained group evidence: its full roster and member
names/tokens were captured under that revision. No latest metadata restamp. Hat
stamp/health loss and shutdown permanently stale group values with the existing
snapshot publication points, just as individual actual observations are retired.

Common output mutations must retire group observations on their bus before a
possible native send: individual/base/colour writes (including Blink/restore),
group writes, native scene recall, group membership programming, output address
clear/restoration/reassignment/assignment. Inputs, read-only queries and
nonactivating scene editing do not cause this cut. A small exhaustive Work
predicate identifies these concrete cases. The owner checks pending physical
work at the top of its existing loop, publishes stale groups before polling the
HAT helper, and leaves them stale through settlement. No group read can reinstall
evidence while another request holds the single permit. A successful group Set
installs its own new sample only after final acceptance; sibling groups remain
stale. No input Notice/measurement or other-bus group changes.

The final proxy acceptance compares the original returned Observation with the
exact cached nonstale observation (including its original physical stamp), group
token/revision and current lifecycle,
after the final await and within the original deadline. A held ready result
cannot resurrect values after a later write, metadata change or bus outage.
The existing snapshot watcher publishes group projections on its SDK worker;
only BridgeKit owns publication deduplication. No helper-private skip cache.

## Polling and implementation

Use the existing Scope::Target path for group refresh and include enabled saved
groups in Scope::All's single fair cursor alongside outputs and input instances.
Each group read has at most15s, clipped to the original All15s ceiling; cursor
advances before every await. Full captures can be expensive on a large bus:
incomplete passes retain completed observations, report an error and resume fairly.
No automatic repair, additional poller or deadline renewal. Disabled buses and
unready UART reject before physical admission. Explicit native commands and web
controls also populate the same group State cache through owner acceptance.

Files: service.rs/group_output.rs/owner.rs/hat.rs/physical.rs and existing Snapshot
test literals; a narrow proxy/group module plus proxy.rs/state.rs dispatch and
projection composition; focused native/MQTT tests; MQTT/group/service guides,
support matrix, CLAUDE, controls and progress. Existing browser source needs no
new controls. Check the real browser suite after integrating new background reads.

## Verification and two self-review rounds

Round1 left the document: traced existing apply/read/All/watcher, group completion,
output writes, scene recall, membership program continuation, address operations,
identification and input ownership. The mutation predicate is on physical Work,
not only public MQTT handlers, so typed/web/Blink operations share the cut. Owner
loop ordering must publish the cut before polling changed(), which starts helpers.
Round2 checked failure arms and siblings: successful partial command reports do
not become successful scalar confirmation; pending/canceled writes cannot revive
cache; mixed booleans/levels clear separately; membership/root changes invalidate
full-roster authority; input-only/scene-staging work is excluded; All keeps one
original ceiling and a persistent cursor. Verify these claims in implementation.

Practical fail-first seams: real MQTT/PTY Desired and refresh with independent
actual membership (including untracked gear), strict atomic invalid maps, actual
clamp/ignored values, mixed/partial/empty clearing and generations; owner cache
cuts via typed individual/group/scene/membership/clear and continued input Notice;
proxy read_pause for held replies across bus/mapping/output changes; paused-time
All cursor/deadline fixtures; pure captured-report projections. Both endpoints
and the real browser must agree with native state, never merely with command echo.

Run intentional controls serially before freezing: omitted group dispatch,
individual destination (existing native control), inferred/retained mixed values,
omitted pre-send cut or Work branch, omitted final cache comparison, retokened
revision/disabled-bus authority and renewed All ceiling. Each names a behavioral
oracle. Then full native, strict Clippy/Pi, formatting, independent wait validation,
Node/assets/whitespace/affected plans and real browser; sequential frozen native/
owner and MQTT/contract source lenses. Record same-family review and lack of
physical HAT validation. No shared SDK, memory, live Store, deployment, publication
or push/merge work is included. Design budget: two self-review rounds and one
fresh combined design gate under the existing approved mission.

## Gate and native checkpoint

Fresh combined design source gate APPROVE, with explicit reminders to dispatch
group Apply validation too, distinguish output/input Work and retain Program
continuations, and keep partial/fallback evidence separate. Implementation
self-review caught a proof-loss issue before production sync: comparing only a
public Report could accept an old held result after a bus cycle and a new
identical report. The private Reply now preserves the original Observation/stamp;
the narrow design correction was source-reviewed APPROVE. A real held-result /
bus-cycle / identical-public-report regression covers it. Both reviews are
read-only and same-family. No second implementation or transport was delegated.

The first MQTT runtime baseline fails with no group scalar actual State. Initial
fixture compilation used private configuration fields; it now compares the
fixture's public saved inventory. All7 new native cases and4 existing All/cursor
neighbors pass (11/11,0.990s execution). Coverage includes actual untracked native
members, original token/trace, ten invalid maps/tokens, disabled buses, separate
mixed values and explicit clearing, five literal missing/MASK/empty modes,
ignored/clamped writes, pre-send cuts, unchanged inventory/other-bus groups/input
Notices, individual/group/scene/membership-repair/address-clear mutations, and
original All deadlines/cursor. Native command regressions also pass unchanged.

The initial appended group endpoints sorted before individuals and changed the
existing read order; stable append preserves individual/input order and the
single persistent cursor. A disabled-bus fixture used an already rejected same
Desired property value; a distinct new value exercises admission rather than
SDK deduplication. No production behavior was changed for that fixture.

An ordinary-read regression exposed cache invalidation incorrectly using final
operation acceptance's strict Ready check. Diagnostics proved identical stamps,
healthy hardware and the ordinary Awaiting phase. Cache validity now shares the
existing individual-output predicate, preserving Writing/Awaiting while still
retiring late/reset/quiet/epoch changes. Final acceptance remains strict Ready.
All temporary diagnostics were removed. This fixes a common polling path without
changing UART recovery or adding outage machinery. Strict Clippy passes10.43s;
fault controls, frozen source lenses and full acceptance follow.

All8 serial intentional controls produce their named REDs in149s total: missing
Apply admission dispatch, Report-only stamp comparison, incomplete Set success,
omitted pre-send cut, omitted membership Program cut, wrong input-feedback cut,
ordinary Awaiting invalidation and invented mixed brightness. The runner restores
all mutations before freezing. Guides/CLAUDE/matrix and the wait registry now
cover group scalar bindings, exact observations, nullable values and common
output mutation scope. Independent wait validation remains a separate check.
