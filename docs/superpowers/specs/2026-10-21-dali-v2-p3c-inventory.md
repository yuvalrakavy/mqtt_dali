# P3c inventory reconciliation and request retirement

Implementation-design draft within the approved architecture. Public identities
remain names and canonical qualified MQTT targets. The work is split into the
bridge inventory/reference-consumer contract first, then a minimal generic
BridgeKit metadata extension before connecting any mutable hardware mapping.

## Current code and failure to prevent

P0 Inventory already has schema 2, a random inventory_epoch and monotonic revision,
but only outputs/groups and startup no-replace persistence. P3a supplies the
qualified target codec. BridgeKit State supports JSON objects, Reporter::remove
retires State, and its maximum MQTT packet is 1 MiB. RequestMeta currently carries
only expiry/conflict; StateValue, command/config mailboxes and DeviceDriver drop
other request metadata. Property-copy suppression compares only value/time.

An old retained Desired for Bus1.Outputs.Light1 must never control a different
fixture later given Light1, even after reconnect, coalescing, re-holding or a
rename back to a previous name. Clearing State or asking the consumer to clear
Desired cannot enforce that invariant. The bridge must check the request version
atomically with resolving its current hardware mapping.

## Snapshot, names and durable transitions

State/{controller}/__inventory carries one observed property Inventory, an object:
wire schema 1, inventory_epoch, revision, controller, buses and complete current
items, an epoch baseline identity projection, plus ordered durable identity
transition batches. This reserved metadata target
is not a physical device. Each item includes its canonical target, physical
location separately, capability metadata and binding_revision. Missing hardware
remains an inventory item; only an explicit removal retires it. Inputs/instances
and scene metadata extend the same typed shape in P4, never opaque unknown items.

A request fence is the opaque string `{inventory_epoch}.{binding_revision}`.
It versions a mapping, not public identity: names remain the only public address.
All baseline imported items start at binding_revision 1. Creating/reusing a name,
renaming, verified readdressing or changing a group's effective membership
advances that item's binding_revision to the new saved inventory revision.
Ancestor renames enumerate descendant transitions and rotate their fences.
Replacing an output at the same short address rotates every affected group's
fence even if its saved numeric member list is unchanged. Derive dependencies
from lifetime continuity, not equality of numeric addresses or rename tokens.
Unchanged descriptive edits do not rotate a binding fence. Missing/returning
hardware does not rotate it. Counters never wrap; an exhausted counter refuses
the mutation. Scene-semantic changes will rotate the scene's fence in P5.

Durable transitions include ordered creation, rename/readdress/change and explicit
removal. Each records the committed revision, before/after canonical target and
fence, and before/after physical location where relevant. A rename/readdress
preserves the logical lifetime; removal plus creation starts another lifetime.
Record intermediate renames and reuse, including a rename back, so an offline
consumer can follow the exact chain without inferring lifetime from an address.
Current items, transitions and revision commit together. No transient MQTT event
is required to reconstruct a missed change.

Freeze the epoch baseline at revision 1: a compact list of the initial canonical
targets, binding revisions and scoped physical locations. History is a list of
batches with strictly increasing committed revisions greater than 1. Description
revisions may leave gaps. Each edge has optional from/to identity projections,
at least one present: create has only to, remove only from, and a continuity
change has both. Every from must exactly match the projection before its batch;
sources and destinations must each be unique. Vacate all sources before placing
destinations, permitting an atomic name swap. A removal plus creation at the same
name in one batch is a new lifetime with the new revision fence. Intermediate
steps within a single atomic save are collapsed to before/after edges; a source
cannot refer to another edge's newly created intermediate state.

To projections use binding_revision equal to their batch revision; baseline
projections use 1. Replaying all batches from the baseline must yield exactly the
identity projection of current items, with no address/name collisions. These
checks are mandatory on both producer and independent consumer, before applying
any change. Reject impossible chains, duplicate sources/destinations, inconsistent
current items and an equal revision with different contents. Ordinary replay of
the same equal revision is a no-op. A different epoch is an explicit reset.

Group identity projections also record effective members by their lifetime
origin: the canonical output target and binding revision at its baseline/create
edge. Continuity edges preserve that origin through rename/readdress; removal
plus creation produces another origin. These are references into named history,
not a second public device address. Validate member origins against active
same-bus outputs after each atomic batch, and against current numeric membership
after full replay. This detects missing group dependency transitions while
avoiding group fence churn for an output merely renamed or readdressed in place.

Retain transitions for the entire inventory epoch. No automatic pruning, no
consumer-ack dependency and no hidden persistent public UUID. Bound the serialized
complete State document at 512 KiB (including its wrapper), and config at the
existing 4 MiB. Refuse a proposed inventory mutation before committing if either
bound would be exceeded; preserve the prior file and report the capacity reason.
Do not silently discard reconciliation history or roll the epoch. The resulting
capacity limit must be documented. A deliberate new epoch is an explicit reset;
existing consumers must not silently rebind across it.

Preflight all encoded targets and the complete State size before creating a v2
import/new file, before hardware/routing admission on existing v2, and before
every mutation. This tightens P0's syntax-only checks: existing/imported names
are never truncated or silently normalized. An oversized existing v2 remains
untouched and reports exact offending paths and encoded/document byte counts.
Document offline repair: with the daemon stopped, correct a not-yet-imported v1
copy, or back up/remove the unusable v2 and explicitly rebuild/reimport a new
epoch; consumer reset/adoption is required. Do not pretend an unsafe truncated
or partial inventory is available. Test individually valid targets whose combined
document exceeds the bound, long Unicode names and large descriptions.

A fresh consumer builds from current items. A same-epoch returning consumer
ignores older/equal revisions, applies transitions after its accepted revision in
order, then reconciles against the complete snapshot. Rename/readdress changes
the existing device record in place. Removal retires the device; logical Home
objects remain and expose an unresolved binding. A reused name creates another
device record. Validate the entire document and transition chain before changing
any consumer state. Partial or invalid documents preserve the last good model.
Different epochs surface a reset requiring explicit consumer adoption policy;
Store bootstrap/migration policy belongs to P8 rather than an implicit match.

Persist snapshot changes through a single serialized owner under a caller bound,
using private temporary files, fsync, atomic replacement and directory sync.
Use expected revision preconditions and report a successful change only after
durable save. P0 startup still uses no-replace publication; v1 is never written.
An existing schema-2 document without new fields is an unambiguous baseline:
binding revisions default to 1 and history to empty; validate and publish the
enriched snapshot without replacing the file until a real mutation.

The daemon writer holds a nonblocking exclusive flock on an adjacent private
regular lock file for its entire lifetime; use no-follow/close-on-exec, never
unlink the locked inode. A second daemon writer fails before admission. Worker
requests own their filesystem slot and a reference to that lock until actual
completion, even if an async caller times out or is canceled. Reads of complete
atomic documents remain possible; they do not grant another writer ownership.

Separate mapping outcomes: not committed, committed/durable, or uncertain. A
timeout or post-replacement directory-sync error is uncertain. Freeze conflicting
mutation/routing admission, retain the pending worker, and resolve only after it
actually ends plus bounded reload and successful file/directory durability checks.
Never restore the old in-memory mapping merely because a caller timed out.

Persist a private revision_high_water field (default current revision for older
schema-2 files; when present it must be at least current revision). Before every
public revision commit, including descriptive edits, atomically save/fsync the old
inventory with high_water reserved through the proposed revision. Only after
that reservation is confirmed durable may the worker replace it with the new
mapping/revision, retaining that high-water value. The reservation is excluded
from the MQTT snapshot and does not advance its public revision. This prevents
reuse of a possibly committed mapping revision after an uncertain second save or
restart. Failed reservation is resolved before attempting any mapping save.
After recovery allocate above max(active revision, high-water); gaps are legal.
Capacity preflight covers both reservation and proposed documents before writing
either. Tests cover timeouts before/after replacement, directory-sync failure,
late completion, revision burning and restart under the lifetime lock.

Persist identity_ledger as an optional baseline+batches field. Derive current
binding revisions by replay instead of duplicating a current projection in the
file or adding fields to every legacy-shaped output/group. Only absence means
the P0 baseline; a present malformed/incomplete ledger never falls back. Full
replay must equal current configured targets/locations. Initialize the explicit
ledger before the first real mutation. The wire carries the resulting per-item
binding revisions and the complete explicit baseline/history.

## Generic request metadata and atomic routing

Add optional MQTT v5 user property target-generation. Missing remains compatible
with existing drivers. If present it must occur exactly once and contain 1..128
visible ASCII bytes; reject malformed or duplicate forms before mailbox posting.
The generic runtime treats its value as opaque. DALI requires the exact current
fence for every mutation; absence or mismatch is a rejected stale request, never
a fallback to address matching. Controller-scoped mutations use the current
epoch/revision fence. Refresh/read-only requests need no fence.

Introduce a cloneable RequestContext with optional generation and backward
compatible DeviceDriver provided methods apply_with_context,
execute_with_context and configure_with_context, defaulting to existing methods.
Add an explicit validation hook before semantic acceptance (including already
satisfied outage results, dedupe and configuration acceptance); its default is
allow for existing drivers. DALI validates against its current inventory without
requiring hardware I/O. Atomic dispatch still rechecks on the service, because a
mapping can change between early validation and dispatch. Rejected stale intent
is distinct from a link failure and from a normal outage-conflict result.
Preserve context per state value, command and config through replacement,
replays, conflict checking, unproven/re-held state and reconnect. Group values
for one apply only when target and context match. Property-copy suppression and
configured-value deduplication include context: equal data from another mapping
generation is a distinct request. Never combine an old property and a fresh
property under a single fresh token.

Inventory-fenced mode is an opt-in driver policy; existing drivers retain their
current behavior. For that mode, reject built-in runtime assume (which otherwise
mutates Reports directly), keep __inventory and other metadata read-only, and
disable Config's cached-equal fast path unless it has revalidated the current
generation. Parse metadata on empty retractions as well as nonempty requests.
A fenced Desired retraction may cancel only pending intent carrying the same
generation; an old generation must not cancel a new one's waiting write. Treat
this as canceling that publisher's old request, not as permission to write the
current target. Missing/malformed generation cannot retract fenced intent.

The DALI proxy sends the values/context in one bounded service request. The
service checks current fence and resolves hardware inside the same serialized
operation that schedules the write. Mapping changes cannot interleave between
validation and dispatch. Recheck before dispatch if an admitted operation waited
behind a commissioning job; deadlines/cancellation remain in force. Read-only
queries never turn a missing/mismatched mutation fence into permission to write.

Reporter::remove retires old State targets. Consumers clear only their own old
Desired/Config topics and migrate only their own known intent through proven
same-lifetime rename/readdress edges after accepting the complete inventory.
Removal/recreation and a new epoch never retoken old intent onto a replacement;
a fresh consumer must not adopt arbitrary retained old intent by matching names.
The bridge never clears a consumer's
retained intent. A generic reference consumer and MQTT examples must implement
these rules independently of Store and of the bridge's mutation reducer.

Every device State carries its current generation. A consumer materializes it
only when that generation matches its accepted complete inventory; mismatched
observations cannot revive a removed/reused name. Inventory itself uses its
epoch/revision and cannot be changed through assume. DALI service read results
carry the generation captured at scheduling and are checked again before report;
late results from a retired mapping are discarded. Mapping retirement also clears
the runtime's old assumptions, feedback overrides, configured-value cache,
conflict/readback bases and pending/unproven intent for the retired generation.
MQTT retained readback is filtered against current inventory before it can seed
active state. State retraction plus newly tagged state uses the shared Reporter;
no second publication path. Freeze exact invalidation/report ordering in P3c2
before mutable hardware integration; P3c1 tests the independent consumer filter.

## Files and verification

P3c1: extend src/config.rs with baseline-compatible fields/validation and safe
revisioned replacement; add src/inventory.rs and tests for pure atomic mutation,
snapshot bounds and typed identity transitions; add a separately implemented
reference consumer in tests plus literal inventory fixtures. Initial reducers
cover output/group add, rename, readdress, removal, group membership and bus
rename. Hardware verification and job orchestration remain P5. No HTTP UI yet.

P3c2: in the isolated tracing-init branch, extend wire/driver, mailbox, session
admission and worker dispatch with context; retain old trait behavior by default.
Update schemas only if the generic wire shape changes (the user property does
not change scalar Desired payloads). Add conformance coverage and README/tutorial,
wait registry and skill guidance. Commit/pin the dependency before daemon
integration is final; no publication/deployment is part of this contract slice.

Failing-first/negative-control cases: offline multi-rename plus name/address
reuse preserves only the intended device record; explicit removal vs missing;
same/older/invalid/partial/different-epoch documents; crash/save failure and
concurrent expected-revision conflict; exact size/counter limits; stale retained
Desired after reuse, mixed-generation equal-value/property copies, outage
re-holding, stale Config dedupe and delayed commands. Preserve existing v1 import,
codec and full generic BridgeKit conformance. Run native/armv7 compile, strict
scoped Clippy, wait registry and fresh sequential core reviews. Store migration
and its actual binding preservation tests remain required in P8.

## Design review record

Sequential same-family reviews required baseline preflight, explicit uncertain
save recovery, process-exclusive writer ownership, exact transition replay,
effective group dependency fences, runtime-owned mutation/acceptance coverage,
generation-tagged observations and continuity-only intent migration. These are
incorporated above. The second review approved P3c1 for test-first implementation
with reservation for every public revision, including descriptions. P3c2's exact
runtime cache/report invalidation ordering remains a separate gate before mutable
daemon integration. Reviews were read-only; no claims of hardware validation.
