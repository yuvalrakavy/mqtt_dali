# Inventory contract (v2 development)

The bridge is the authority for its saved physical inventory. Names are public
identities; short addresses and group addresses are separate locations. Qualified
targets use the canonical codec in `src/targets.rs`, for example
`Bus1.Outputs.Kitchen` and `Bus1.Groups.Downstairs`. Literal dots/percent signs and
non-ASCII bytes are escaped; names are never silently normalized.

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
Device State/events will carry generation tags, and consumers must accept only
the generation in their current complete inventory. Retired State uses BridgeKit
contextual retirement/removal; each consumer owns cleanup of its own retained
Desired/Config. The adopted runtime enforces original request contexts and
retires old work/cache entries. Physical output/input State remains a later
capability slice; metadata service success does not establish HAT connectivity.

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
contracts; v2 daemon/hardware/web integration is still pending. The operator-owned
directory and ordinary local Unix filesystem are assumed; physical power-loss
behavior is not tested.
