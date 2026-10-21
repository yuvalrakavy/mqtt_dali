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
wire example. The new daemon does not publish it yet; BridgeKit integration is
still in progress.

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

Missing hardware stays in saved inventory. Only explicit removal retires it.
Device State/events will carry generation tags, and consumers must accept only
the generation in their current complete inventory. Retired State uses BridgeKit
`Reporter::remove`; each consumer owns cleanup of its own retained Desired/Config.
Runtime request/context enforcement and cache invalidation are the next contract
slice; inventory generation tags alone do not yet provide that enforcement.

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

Close has a caller deadline. A false result means filesystem ownership may still
be held; the managed runtime must take its incomplete-stop path. A true result
means the file owner ended even if client handles remain. These are library
contracts; v2 daemon/hardware/web integration is still pending. The operator-owned
directory and ordinary local Unix filesystem are assumed; physical power-loss
behavior is not tested.
