# Bounded DALI identity snapshots (approved direction, implementation delta)

Tier T3: durable lifetime and request-continuity semantics. Owner approved replacing
unbounded rename history, keeping current limits. This supersedes full-history
requirements in the P8 foundation spec; no shared Rhai budget increase.

## Contract

Reuse the existing private Origin {target-at-creation, creation-revision}, scoped
by inventory_epoch. Names remain public names and MQTT targets. No new UUID,
consumer registry, acknowledgement protocol, timer or journal. A current identity
record contains Origin, current Projection, parent_origin for input children, and intent_revision (the last change
that revoked earlier intent). Initial intent_revision equals creation revision.
Ordinary renames/address moves preserve it. Membership/type/representation changes
and explicit same-target/same-location binding revocation advance it. Changing
away and back does not restore it. Input children retain their parent Origin.

Wire inventory schema2 publishes identity_snapshot: [identity records] and current
buses/items, without identity_ledger. Existing target-generation stays
<epoch>.<binding_revision>. Snapshot size depends on configured items/members,
not past edits. Missing items retire their Origins; recreation has a new Origin.
No tombstones are needed: continuing consumers retain their old accepted snapshot
and reconcile it against the complete new snapshot. Older inventories are ignored,
equal revision must be identical, and epoch adoption stays explicit.

Independent consumers validate shape, unique Origins/targets/locations, Origin
kind/creation revision <= intent_revision <= binding_revision <= document revision,
group references, input parents and exact current projection agreement before
any writes. Against their accepted snapshot, continuing Origins must keep kind,
parent Origin and nondecreasing intent/binding revisions. An unchanged binding
requires identical Projection and intent_revision. A changed projection must have
a binding revision newer than the consumer's accepted document revision. An
incompatible current contract must also have a newer intent_revision. New Origins
must be newer than the prior accepted document revision (retired Origins cannot
be resurrected). Intent follows a continuing Origin only if its captured binding
revision >= current intent_revision. State requires the exact current generation.

## Persistence and legacy upgrade

Continue reading existing config schema2 and legacy Ledger {baseline,batches}.
Extend Ledger with optional checkpoint {revision, records}; checkpoint forbids a
nonempty legacy baseline. Replay initializes from validated checkpoint or legacy
baseline, then applies the bounded tail. Tracked records carry intent_revision.
After a legacy replay or edit, if tail exceeds 32 batches or 32KiB serialized,
checkpoint the current records and clear baseline/tail. Thus recent history is
optional diagnostic information, never required by MQTT consumers; it may be
empty immediately after compaction. A large single atomic change becomes a
checkpoint directly. The checkpoint is bounded by current inventory; overall
4MiB config /512KiB State limits stay. No automatic epoch reset.

Model construction normalizes its derived ledger in memory but preserves its raw
Inventory as the writer comparison basis. Edit proposals serialize the normalized
ledger; subsequent ordinary save
persists the compact form using the existing atomic config writer. Preserve epoch,
revision/high-water, every Origin, binding revision, intent barrier and private
scene references. Existing v1 config/binary remain untouched. Malformed or null
ledger/checkpoint never falls back to fresh identities. Old v2 readers reject the
new wire schema explicitly; v2 is not released yet. The example consumer keeps
schema1 verification for old fixture/upgrade coverage, but normal bridge output,
new acceptance tests and Store integration use schema2.

## Consumers and pending requests

Web row keys read current identity_snapshot directly; support old ledger fixtures
for compatibility and verify stable keys across long offline edits/restart.
Store applies current snapshots by Origin to existing ObjectIds/Home bindings;
does not need rename history. Update initial/renamed literal Store fixtures.

Store retained Desired recovery must not infer an old request's lifetime from its
old topic alone. Add an optional Kit bridge_target_identity() hook, default () for
legacy drivers, captured as target-identity in outgoing request properties. DALI
uses the canonical epoch/Origin identity. Its historical request resolver locates
the current Origin by that captured identity, checks original generation's epoch
and binding revision against intent_revision/current binding, and preserves the
original payload/conflict/expiry. Identity is captured in the existing memory-only intent entry and in the
broker-retained request when originally published. Restart recovery covers those
already-published retained leases, including restart between topology commit and
retarget publication. Preserve current behavior for unpublished memory-only
requests. Adoption must carry validated target-identity into its rebuilt properties,
not drop it while reconstructing conflict/generation. Requests without proved identity cannot
be retargeted from an obsolete mapping; current exact-generation requests work.
Bridge continues enforcing target-generation; identity is consumer bookkeeping.

## Implementation / evidence

1. Test-first bridge checkpoint/barrier tests and 1200 production Model renames;
   verify bounded State/config and untouched IDs/intent semantics offline/restart.
2. Producer: IdentityRecord/checkpoint validation, legacy replay barriers,
   normalization, bounded persisted tail and wire schema2 snapshot.
3. Independent consumer schema2 validation/reconciliation; preserve old schema1
   mutation fixtures explicitly as legacy cases, not as current producer output.
   Tests: rename-back, address/name swaps, member/type/color away-and-back,
   same-address replacement/removal, input ancestry, malformed/equal/stale rollback.
4. Web direct row keys, native-snapshot UI regressions, rebuild committed assets.
5. Update Store foundation spec/fixtures and Kit identity metadata before current
   inventory integration. Prove actual structured snapshot over lane MQTT with
   existing map10000 limit; no shared limit changes.
6. Targeted affected native/frontend suites, meaningful guard controls, scoped
   build/lint, fresh review, docs/support/progress. Full browser workflow once if
   row-key behavior changes. Avoid unrelated Store formatting/template rebuilds.

Source enumeration: inventory::{Model,state_bytes}; inventory/ledger::{Tracked,
baseline,replay,match_current}; inventory/edit::change; config::Inventory;
config_store/worker normalization; tests/support/inventory_consumer; web/model
rowOrigins/rows; Store Dali on_state (currently no-op TDD stub), Std BridgeTarget
request properties, RetainedIntent context/retarget hooks. No physical UART changes.

Review focus: stable identities after bounded retention, barrier loss on legacy
upgrade/away-and-back, input parent replacement, restart-safe Desired continuity,
malformed snapshot rollback, compatibility and bounded work/storage. Reviewers
read current code; report only concrete correctness gaps with proposed fixes.

Design gate: two source-only GPT-family reviews (A Astra/high, B Sol/high),
reports /tmp/dali-compact-identity-design-{a,b}.md. All concrete findings addressed
above: raw writer basis, retained-vs-memory durability, adoption metadata, explicit
parent Origin. No cross-family review available. Owner implementation go already given.


## Implementation verification (2026-10-24)

Frozen implementation787c011 plus confirming correction.868/868 native,
125/125 frontend, actual linked phone/desktop TCP browser1/1 (172.294s),
strict v2 production Clippy, all-bin Pi check and five meaningful negative
controls pass locally. Existing whole-legacy strict Clippy/formatter issues are
outside this slice; changed Rust files are formatted. No physical validation.

Depth/test source review found a schema2-to-newer-schema1 downgrade in the example
consumer and missing fresh-intent-after-barrier coverage. Add behavioral RED and
refuse downgrade; keep explicit old-wire tests on schema1; prove fresh intent
survives ordinary moves and the conditional over-revocation mutant fails. The
confirming delta read approves both corrections without an unresolved serious
defect; report /tmp/dali-compact-confirm.md. Reports /tmp/dali-compact-code-depth.md and
/tmp/dali-compact-test-review.md; these are same-GPT-family source reviews, not an
independently executed mutation campaign. Actual guard mutations were executed
by the parent through the registered negative-control driver.

Store's optional identity hook and adoption propagation pass8 focused tests;
its actual MQTT callback parses the reviewed compact fixture at the unchanged
runtime map10000 cap (1/1). Store tree creation/migration remain unfinished P8
work; the snapshot correction does not claim those completed.
