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
`Inventory` contains wire schema 1, epoch/revision, controller/buses/items, and
the complete named identity ledger. This metadata target is not a fixture.
The literal [rename fixture](../tests/fixtures/inventory-v1-rename.json) is the
wire example. The managed `service::DaliService` now publishes it through the
pinned BridgeKit runtime; real FakeBroker integration tests consume that output.
The canonical `mqtt_dali` executable runs the managed v2 service;
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
still permits ordinary intent continuity. Colour selection is forbidden on group
or input projections, including past ledger states. Updated readers accept old
documents; older strict readers need updating to accept new-field documents.
Use the service edit/command path so selection and history commit together;
manually changing a saved setting without its ledger is rejected.

The ledger starts with the epoch baseline and retains every atomic transition
batch. Before/after edges prove continuity, creation and removal. Consumers replay
and validate the whole history against current items before accepting a higher
revision. Sources leave together before destinations arrive, allowing name swaps.
Never infer lifetime continuity from an address, a reused name or one transient
MQTT event. An absent ledger in an older schema-2 file means the initial baseline;
an explicitly null, malformed or incomplete ledger is an error.

An offline consumer follows continuity to rename existing device records in
place. Removal/recreation creates another record; old intent must not follow it.
Intent owned by that consumer may follow proven rename/address continuity,
including rename-back, but must not be retokened onto a replacement or another
epoch. Ignore older documents; accept an identical equal revision as a no-op;
reject conflicting equal revisions or rewritten accepted history. A new epoch
requires explicit adoption. The independent example algorithm under
`tests/support/inventory_consumer.rs` exercises these rules without Store code or
the producer's replay/mutation implementation. Actual Store ObjectId/binding
migration remains a separate implementation phase.

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
bytes. Both initial/imported inventories and changes are preflighted. History is
never silently pruned. When capacity is exhausted, retain the old document and
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
