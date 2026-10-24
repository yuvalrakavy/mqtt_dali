# Inventory contract (v2 development)

The bridge is the authority for its saved physical inventory. Names are public
identities; short addresses and group addresses are separate locations. Qualified
targets use the canonical codec in `src/targets.rs`, for example
`Bus1.Outputs.Kitchen` and `Bus1.Groups.Downstairs`. Literal dots/percent signs and
non-ASCII bytes are escaped; names are never silently normalized.

Inputs occupy a separate address space: `Bus1.Inputs.WallPanel` contains named
instances such as `Bus1.Inputs.WallPanel.Instances.EntranceKey`. A bus's optional
`inputs` array contains modules with `short_address`, `name`, optional
`description` and `instances`; each instance has `number`, `name`, optional
`description` and native numeric `instance_type`. Module addresses are 0..63;
instance numbers are 0..31. Unknown types remain metadata. The same short address
may identify an output and an input independently. Missing inputs in an older
v2 file or v1 import means an empty array; an explicit null array is invalid.

Named native scenes occupy `Bus1.Scenes.Evening`. Each bus's optional `scenes`
array contains `slot` (0..15), `name` and optional `description`. Names and slots
are unique within that bus's Scenes namespace, independently of outputs/groups.
An absent array means empty; null is invalid. Scene projections carry their
target and binding revision, with no members, input type or colour representation.
Rename, including bus rename, preserves the lifetime and rotates its binding;
description changes preserve the binding. Removal/recreation starts a fresh
lifetime. History remains after removal, so older readers that do not understand
Scene locations can still be incompatible when the current scene list is empty.
See [scene metadata operations](dali-scenes-contract.md#named-slot-metadata).
This catalogue does not expose per-fixture scene settings or active-scene State.

The pure `inventory::Model` validates configuration and produces the complete
State payload destined for `DALI/State/<controller>/__inventory`. The property
`Inventory` contains wire schema 2, epoch/revision, controller/buses/items, and
`identity_snapshot`: one compact identity record for each current item. This metadata target is not a fixture.
The literal [rename fixture](../tests/fixtures/inventory-v2-rename.json) is the
wire example. The managed `service::DaliService` now publishes it through the
pinned BridgeKit runtime; real FakeBroker integration tests consume that output.
The canonical `mqtt_dali_v2` executable runs the managed v2 service;
`--check-config` remains an explicit offline operation. See
[service development](service-development.md).

Each item has a `binding_revision`. The opaque request generation is
`<inventory_epoch>.<binding_revision>`; it versions the mapping, not the device's
name. A rename or verified address move preserves the device lifetime and rotates
that mapping's version. Replacing a fixture creates another lifetime, even with
identical name/address text. Groups rotate when their effective member lifetimes
change, including replacement at an unchanged numeric address. An output merely
renamed/readdressed within its lifetime leaves group meaning intact.

Input module rename preserves its lifetime and those of its instances, rotating
all affected targets/generations together. Renaming one instance rotates only
that mapping. Module replacement/removal retires all descendants; identical text
and addresses cannot preserve the replaced buttons' lifetimes. Every historical
batch must have a complete, consistent module/instance hierarchy. A continuing
instance must retain the same parent lifetime across a batch. Instance type is
part of its projection, so a type change cannot keep the old generation or carry
old binding intent as though it were merely a rename.

Input address adoption likewise carries the module and every continuing child
Origin to the new short address, rotating their mapping generations together.
The old reservation becomes free. A discovered temporary candidate and its
children retire in the same atomic batch. Existing consumer object IDs and
compatible bindings follow the original Origins, rather than the temporary
row or numeric address. Output adoption uses the existing readdress operation;
desired groups follow the relocated original, and private scene references keep
its Origin. See [address recovery](address-recovery.md).

An output can optionally configure `color_representation`: `temperature`, `xy`
or `rgbwaf`. Omitted or null means the basic power/brightness contract; absent
values are omitted when saved. This is the public component selection, not a
claim of hardware support. It appears in the output's identity projection and
current Inventory. A change preserves the physical lifetime, name/address and
consumer object ID, while rotating that output's binding generation. Group
membership and unrelated bindings stay unchanged. Repeating the selected value
is a no-op; changing away and back does not reuse a generation.

Consumers must drop old observations and retire their old contract intent,
including when a format change and rename occur together. A same-format rename
still permits ordinary intent continuity. Output and group projections may select
color representations independently. Selection is forbidden on input and scene
projections, including past ledger states. Updated readers accept old
documents; older strict readers need updating to accept new-field documents.
Use the service edit/command path so selection and history commit together;
manually changing a saved setting without its ledger is rejected.

Each current identity record contains the immutable private `origin` (target and
revision at creation), current `projection`, and `intent_revision`: the last
change that revoked earlier control intent. Input-instance records also capture
`parent_origin`. Names remain the public addresses; Origin is reconciliation
metadata. Ordinary name/address moves preserve Origin and intent_revision.
Membership, type or representation changes advance the intent barrier, including
changes away and back. Removal/recreation creates a new Origin even with identical
name/address text. Group members reference current output Origins.

Consumers validate all identity records and exact agreement with current items
before applying any change. Match continuing records by Origin to preserve device
IDs and bindings. Require nondecreasing binding/intent revisions, unchanged parent
Origin, and births newer than the last accepted document for newly seen records.
An old intent can follow a continuing Origin only when its captured binding
revision is at least the current intent_revision; State still requires the exact
current generation. Ignore older documents and accept identical equal revisions
as no-ops. Conflicting equal revisions and a different epoch require refusal or
explicit adoption. No consumer acknowledgements or historical replay are needed.

Saved configuration can still read the former baseline/transition ledger. The
bridge replays it once to recover the exact Origins, parent identities and intent
barriers, preserving epoch/revision and private scene references. The derived
ledger checkpoints current identities once its diagnostic tail exceeds32 batches
or32KiB; it then discards that tail. A subsequent ordinary save persists the compact
form through the existing atomic writer. Startup preserves the original file as
its comparison basis. Recent history can be empty after compaction and is never
published on MQTT. An explicit null/malformed ledger or checkpoint is an error.
Schema1 MQTT readers need updating to schema2; v2 has not been released. The
independent example consumer retains explicit schema1 compatibility tests.

For retained-request recovery, a consumer must capture the stable lifetime in its
original request context (Store uses the optional `target-identity` user property).
An obsolete topic alone cannot prove identity after it has been reused. Preserve
original expiry/conflict when retargeting a compatible retained request; reject
unproved requests. The bridge's execution fence remains target-generation.
See the [compact identity design](superpowers/specs/2026-10-24-dali-v2-compact-identity.md)
for the checkpoint and recovery invariants.

Retained State may arrive before its inventory. The independent consumer caches
the latest observation per target/token until a complete accepted inventory proves
that mapping. No unknown State creates a device or changes intent. Current
observations clear when their mapping version changes. An old retraction removes
only its exact pending/current token; it cannot erase fresh state. Ignored older
inventories and malformed/conflicting documents do not drain observations.
The example bounds pending data to 64 entries and 64 KiB including target/token
bytes, with an 8 KiB per-State limit and deterministic oldest-entry eviction.
Eviction leaves state unknown and calls for a read, never an assumed off value.
Button events must not enter this retained replay cache.

Missing hardware stays in saved inventory. Only explicit removal retires it.
Public [Remove from list](remove-from-list.md) removes an output or input-module
subtree through the durable metadata owner. Same-address/name rediscovery creates
a fresh lifetime; removed intent/observations must not migrate into it. Output
removal cleans saved group references but does not program physical groups.
Bus [setup](bus-setup.md) preserves saved children. Enable-only changes preserve
their Origins and binding tokens; a bus rename preserves Origins while changing
descendant targets/bindings. Disabled buses retain reservations but retire live
cached values/listeners; re-enable does not restore old button actions or State.
Addressed [discovery](discovery.md) creates default-named additions through this
same ledger. `AddInputInstance` gives only the new child a fresh Origin; its
continuing parent and siblings retain their identities and binding generations.
Neither a smaller instance count nor missing hardware removes a saved item.
Device State/events will carry generation tags, and consumers must accept only
the generation in their current complete inventory. Retired State uses BridgeKit
contextual retirement/removal; each consumer owns cleanup of its own retained
Desired/Config. The adopted runtime enforces original request contexts and
retires old work/cache entries. Base individual output State is implemented
(see [MQTT outputs](mqtt-outputs.md)); resolved native input observations/actions
use [MQTT inputs](mqtt-inputs.md). Input metadata can be renamed with the existing MQTT `rename`
command or service API. Input Desired remains unsupported; saved metadata does
not establish HAT connectivity.

Limits are deliberate: config at most 4 MiB; the complete serialized Inventory
State (including its wrapper) at most 512 KiB; each encoded target at most 4096
bytes. Both initial/imported inventories and changes are preflighted. Diagnostic history is bounded automatically; current identity records are never
discarded to make room. When capacity is exhausted, retain the old document and
report the offending path/size; no partial snapshot or automatic epoch reset.

If an installation cannot pass preflight, stop the daemon and keep backups.
Before the first import, correct a copy of the v1 configuration and select that
copy explicitly. For an unusable existing v2, back it up and rebuild/reimport a
new epoch deliberately, then explicitly reconcile/adopt it at consumers. Deleting
v2 forces import of the current v1 file; the bridge never edits v1. Do not edit a
live v2 file or remove its writer lock.

`Model::edit` proposes an immutable next configuration; it does not save or verify
hardware. Its caller must complete commissioning verification before applying an
address change. `config_store::ConfigStore` supplies serialized durable saving
with expected-revision checks. It holds an adjacent private `.lock` file for its
lifetime; a second writer is refused. Do not delete a live lock. Reads of complete
configuration documents remain possible.

A change first saves the old inventory with its next revision reserved, then
saves the new mapping. Private `revision_high_water` is absent from MQTT and
prevents reuse of reserved revisions, including after a failed or interrupted
save. A reservation may leave a legal gap in public revisions.

Save outcomes distinguish Committed, NotCommitted and Uncertain. On timeout or
uncertain replacement, the worker keeps its slot/lock and conflicting operations
stay frozen. Caller cancellation does not undo a possible file replacement.
After the worker ends, `recover` reloads only a recognized old/reserved/proposed
document and confirms file/directory durability before admission resumes. An
unexpected edited file or persistent sync failure remains frozen. Startup also
confirms durability before admitting an existing file.

The service has separate routing readiness. It freezes before saving, then
requires successful complete-table runtime retirement and queues the complete
inventory before exposing the new mapping as Ready. Recovery activates the
recognized complete model even when ConfigStore reports `changed: false`.
Runtime-CAS failure keeps the durable candidate frozen; retry uses only the exact
original expected table. A persistent conflict requires restart, not adoption of
an unknown authority. Metadata MQTT commands use a single-use active completion
capability so their own commit can return one original-token success; unrelated
and queued retired commands remain refused.

Close has a caller deadline. A false result means filesystem ownership may still
be held; the managed runtime must take its incomplete-stop path. A true result
means the file owner ended even if client handles remain. These are library
contracts; authenticated web integration is still pending. The operator-owned
directory and ordinary local Unix filesystem are assumed; physical power-loss
behavior is not tested.
