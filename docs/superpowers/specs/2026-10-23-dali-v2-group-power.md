# Native group power and dimming

## Problem, evidence and shape

Saved groups already have names, identities, desired membership, native mask
read/adopt/repair and browser assignment. They cannot yet control ordinary power
or level. The approved architecture explicitly requires group control and
uniform/mixed/unknown aggregation from fixture samples. This is a common lighting
operation and also a prerequisite for split-channel logical colour equipment.

Current source: `dali::gear::Intent::frame(Destination::Group)` already creates
checked OFF, RECALL MAX and DAPC frames; independent gear vectors cover slots0/15.
`dali::groups::query` reads membership only at an individual short address.
The emulator already applies native group frames using physical membership.
`service::output` deliberately accepts only individual outputs. `groups` owns
membership settings, not levels. `scenes` demonstrates native group/broadcast
mutation, identification exclusion and conservative bus output invalidation.
No new transport, scheduler, timer, persisted configuration or group query codec
is needed. Verify these references during implementation rather than infer a
group reply can establish a level.

Shapes considered: extend the single-output observation/route to a heterogeneous
group type; fan out single-output writes; or add a concrete group-output operation
under the existing physical owner. Choose the last. It preserves single-output
API semantics and uses one native group command, with synchronized hardware
destination semantics. Fanout loses those semantics and would silently replace
the user's hardware group. A generic operation framework is unnecessary.

## Scope and truthful observed values

First slice is base power/brightness only. DT8 group colours and scalar Desired
bindings follow after this vertical operation; no selected group colour format
or fabricated RGB fixture is added here. PanelScript and Store remain P8.

Add a pure captured-evidence aggregate first, then owned MQTT/HTTP/typed commands
and compact controls on the selected Groups screen. The capture scope is every
saved output on the selected enabled bus, bounded by the existing64 addresses.
Reports explicitly say coverage=`saved_inventory`; they never certify that no
untracked or unaddressed physical gear exists. Native commands affect actual
hardware membership, including untracked fixtures, just like native scene recall.
The operator's Verify/Repair flow remains the way to fix membership drift.

For each saved fixture, read both native membership bytes. Preserve missing,
collision and full-byte values (FF is valid membership). If its actual mask
includes the group, query actual level/status at that short address. Never query
level/status at the group destination. Record saved-member expectation separately
from actual membership, including unexpected actual members and missing expected
members. Do not overwrite saved assignments as a side effect of a level read.

The aggregate must not average levels or echo an intent. Known complete members
can yield uniform power despite mixed positive levels. PowerState is a boolean
only if every observed actual member has current level/status and agrees on
on/off; Brightness is numeric only if their ordinary native level codes agree.
Mixed indicators are separate for power and brightness. An empty confirmed set
is `empty`, never off; incomplete membership or member samples make the aggregate
`partial`/`unavailable` with no usable aggregate value or state-derived toggle.
Explicit On/Off/level commands still use normal valid authority. Keep per-fixture raw
diagnostics and observed disagreement rather than converting MASK/silence into
off. Captures are sequential samples, not an atomic bus snapshot or fade promise.

## Native ownership and interfaces

`read_group_output` is Read-class, no parameters, saved group target; generation
may be omitted but a supplied one must match. `set_group_output` is WithReply,
strict current expected_revision plus the existing validated nonempty
power/brightness pair. No null/wrong-type/unknown or contradictory property is
accepted. Typed methods use the same route and checked gear Intent. HTTP uses
the existing authenticated jobs/parser/CSRF/session and immutable admission.

One new concrete Work variant retains the original permit, request context,
completion, deadline and physical stamp through settlement. Limit the request to
15 seconds within its original caller deadline. No independent member operation
can run between the native write and its capture. Resolve against saved group
and enabled bus at submission, owner admission and final acceptance. A write
requires exact original group authority and revision; member diagnostics retain
their captured mapping, never a latest-token restamp. Reject writes while this
bus is identifying, as native scene recall already does.

After full syntax/authority preflight, send exactly one native group OFF/MAX/DAPC
frame, no repeated setting and no individual fanout. Ordinary N means `sent`, not
that every fixture reached the requested value. Capture actual membership/member
levels afterward. Report sent with truthful complete/mixed/partial actual data;
possible failed sends or invalid final evidence are `unconfirmed`. No retry or
rollback. A read reports observed evidence only. Expired original authority can
never produce current success. Unexpected backward data/collision on the write
is uncertain, not confirmation.

Before possible mutation, and again on settlement including canceled/failed work,
invalidate prior cached actual output/colour samples on that bus, following the
existing scene recall lifecycle. This first slice does not promote new member
readings into the single-output cache; ordinary readbacks repopulate it. Keep
input values/Notices and all other buses unchanged. Reports never save config.

## Compact protected interaction

Keep group assignment on its existing screen. The selected saved group's title
gets Refresh, an actual power toggle when a uniform power value exists, explicit
On/Off choices when mixed/unknown, and a compact level control. Controls always
name the hardware group; no implication that only checked saved fixtures will
be affected. Show actual mixed/unavailable state and membership differences with
a link to the existing verification flow. A sent result is described as sent;
sampled values remain actual. Freeze group/location/token/revision/connection
epoch/dialog lifetime and clear actionable local evidence before I/O. Closing
or navigating retires local results; it cannot undo a command already sent.

## Implementation and verification

1. Pure aggregate with fail-first tables for uniform off/on, mixed levels with
   uniform power, mixed power, MASK, collision, missing membership/status,
   actual-vs-saved drift, empty sets and boundary slot15/address63. Use raw
   independent evidence; a caller cannot label incomplete members current.
2. Concrete owned read/set and strict commands. Literal UART and independent
   emulator/PTY/MQTT verify one group frame, actual hardware selection (including
   saved drift), no fanout/group queries/config mutation, other-bus isolation,
   original authority/deadline, final stamp, identification and conservative cache
   invalidation. Mid-checkpoint before adding UI.
3. Protected Groups controls, frozen authority and actual-result guards; client
   tests and real phone/desktop read/On/Off/level/mixed/partial/rename/cancel flows.
   Check operator-visible values against native snapshot and retained config.
4. Serial intentional controls for raw MASK/membership completeness, power-vs-
   brightness aggregation, native destination, ignored final stamp, cache
   invalidation, client original authority and requested-vs-actual display.
5. Freeze; sequential native/owner and UI/contract readers. Full native, strict
   Clippy/Pi, changed formatting/wait/asset/Node/whitespace/control plans and real
   browser. Update guides, support matrix, CLAUDE and progress with actual limits.
   No shared SDK, memory, deployment, publication or live-system mutation.

## Two self-review rounds

Round1 checked existing group vectors/model and avoided duplicating the UART or
reinterpreting a group as a short address. Report coverage is the saved inventory;
it cannot claim an exhaustive bus census. Round2 separated power uniformity from
level uniformity, actual membership from saved intent, command-sent from attained
level, and native bus cache invalidation from input-event cuts. A fixed original
deadline bounds64 saved outputs; unresolved captures remain partial. The same-
family review limitation and physical firmware validation boundary remain.

## Design verification

Combined implementer/reviewer source gate APPROVE. Three clarifications are
incorporated without changing shape: unknown aggregate state does not disable
explicit native writes; completeness covers every frozen saved roster entry,
including unvisited rows; optional-generation reads bind current authority once
at submission and retain that token/revision/member mapping through acceptance.
The pure helper takes the entire roster separately from captured raw samples,
rejects duplicates/unexpected samples, and derives quality itself. A matching
queried prefix cannot stand for an unvisited saved fixture. No distinct-family
or physical review is claimed. Existing native codec/model/invalidation patterns
held; no new framework is warranted by the review.

## Native mid-checkpoint

The five pure aggregate cases first fail at runtime, then pass. The first real
MQTT case fails with no such command before registration. Managed strict read/
Set now use one concrete Work under the existing physical owner. All17 focused
cases pass (11 new pure/native and6 retained input-feedback cases),6.555s
execution. Native literal vectors use bus1, address63 and group15; ordinary
N/silence, collisions, MASK, statusFF, ignored writes and native maximum/level/
OFF are distinguished. Independent physical group membership, including an
untracked member, determines actual command selection. Saved membership remains
unchanged; explicit coverage is the frozen saved roster. Original authority,
disabled bus, final physical stamp, expired ready reply, bus-local cache cuts,
other-bus values and a continuing button Notice are covered.

Initial integration corrections were test-only: BridgeKit replies wrap the
report in `value`, and a literal peer must capture the whole PTY owner to retain
its slave descriptor (Rust disjoint-field capture otherwise drops it). A missing
explicit module import was a compile-only correction. These do not change wire
semantics. Self-review traced submit/admission/settlement, original read token/
revision and full-roster placeholders; no new task/configuration/polling loop
or input activity cut was added. UI and final fault/review acceptance remain.

The separate real Blink-admission test also passes (0.317s): active identification
refuses group writes while read admission remains valid. All77 frontend cases
pass. The client independently checks full member identities/raw evidence and
aggregate consistency, preserving actual values even when a sent command was
ignored. Explicit unknown-state writes remain available without a prior read.
There is no automatic group read on component mount, avoiding contention with
membership batches.

Actual phone/desktop acceptance passes984 owned commands in85.50s, including
native On/Off/level, mixed positive levels with uniform On, missing-address
unknowns, explicit Off without a prior usable value, unchanged saved configuration/
other fixtures, held reads across slot/navigation and actual level restoration.
All four new screenshots were inspected. Artifacts:
`/tmp/dali-browser-affea6b2334b8e30389429b4e3ea432b`; log:
`/tmp/dali-group-output-browser2.log`. The initial browser setup used private
configuration objects as public target projections; correcting setup lookup to
the authenticated public snapshot fixed that fixture error. Production behavior
was unchanged. No subsequent production browser mechanism change is claimed.

All11 serial intentional controls produce their declared REDs in134s total:
prefix completeness, power/level separation, MASK, high membership byte, native
destination, bus cache invalidation, final epoch, Blink exclusion, client full
roster, post-await authority and failed-action evidence clearing. The runner
restores every mutation before frozen readers/builds. API/operator/contributor
guides, support matrix and CLAUDE describe command versus scalar boundaries and
truthful coverage. Final source lenses and broad acceptance follow this freeze.
