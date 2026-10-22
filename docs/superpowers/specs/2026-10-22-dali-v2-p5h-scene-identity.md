# P5h2a: durable named scene slots

## Purpose and approved boundary

Continue the approved native scene architecture after the codec/model increment.
A named native slot needs the same durable identity, rename continuity and
consumer fencing as outputs/groups before managed scene editing can refer to it.
Add that metadata boundary first; stored per-fixture contents and physical jobs
remain P5h2b, and the approved Scenes interface remains P5h3. This increment does
not operate lights, populate hardware slots, claim verification or expose an
active-scene property. Existing architecture and continuation approval apply.

Use the existing Inventory/identity ledger, ConfigStore owner and shared command
parser. Separate scene identity files or a browser-owned catalogue would create
another authority; assigning public UUIDs would contradict the approved named
identity. Slots belong under each bus's Scenes address-space singleton, without
another Buses layer. The mission retains T3 discipline. Budget: about ten minutes
design/reviews and 25–40 minutes implementation/verification.

## Grounding and readers

| Fact | Current declaration/call path | Consequence |
|---|---|---|
| Scenes namespace already exists | `targets.rs`, Namespace::Scenes/Target::item | Reuse qualified target encoding |
| Config Bus enumerates outputs/groups/inputs | `config.rs`, Bus and validate_structure; legacy into_current | Add optional scene list and explicit empty import/constructor defaults |
| Inventory has four Location variants | `inventory.rs`, Location/Edit; `inventory/ledger.rs` catalog/project/shape/replay/match_current | Add Scene to the existing complete identity machinery |
| Metadata owner already saves and replaces generations | `service.rs`, MetadataEdit::into_edit; routing::admit; owner save/accept path | Reuse durability, original completion, tracing and uncertainty behavior |
| MQTT/HTTP use shared metadata parsing | `service/proxy.rs` info/parse_metadata and web job submission | Add commands here, not a second web save path |
| Consumer validates history and current objects separately | `tests/support/inventory_consumer.rs`, valid_records/current catalogue | Extend both historical and current scene checks |
| Current browser row list selects outputs/groups/inputs explicitly | `web/src/model.ts`, rows | Metadata may be carried without rendering it as a device; Scenes UI remains pending |

The Location census reaches inventory, service routing/proxy/output/input/
discovery/owner, related tests and the independent consumer. Inspect each use;
only exhaustive matches need another arm. Output/input resolvers must continue
refusing the new address space rather than treating scene slots as short
addresses. The compiler catches struct literal/default changes; JSON fixtures
remain useful for fail-first valid/invalid input coverage.

## Persisted and public representation

Add `config::Scene { slot: u8, name: String, description: Option<String> }` and
`Bus.scenes: Vec<Scene>`, defaulting empty and omitted when empty. Keep persisted
schema version 2 and public inventory schema 1: old documents stay valid and
their absent scene list is not synthesized into objects. A nonempty list is an
additive development contract extension; older strict consumers need the updated
Scene Location reader. Scene history remains after removal, so an older reader
may remain incompatible even when the current scene list becomes empty. Keep
the updated reader; never erase history or reset an epoch automatically to make
a downgrade work. The separate v1 binary/config remain unaffected.

Within a bus, slots are unique and 0–15, and scene names are unique in Scenes.
Use existing component-name/description rules, preserving exact names. Other
address spaces may use the same name or numeric address. Reject malformed arrays,
null scene lists, missing fields, invalid slots/names and duplicate names/slots.
Existing 4 MiB file/512 KiB complete-State limits remain unchanged and preflight
the full proposal; do not prune history or manufacture a new epoch.

Add `Location::Scene { bus, slot }`. Scene projections have no members, input
instance type or colour representation. Public bus scenes gain `target` and
`binding_revision`, just like other named items. Target example:
`Bus1.Scenes.Evening`. No colour/level settings or fixture children are introduced
in this slice, and later Store scene objects remain opaque.

Add Edit::AddScene/RenameScene/RemoveScene/SceneDescription through the current
immutable editor. Creation begins a fresh Origin; rename (including bus rename)
continues it and rotates its binding token. Removing and recreating a slot/name
begins a distinct lifetime. Description-only changes advance inventory revision
without replacing scene identity or its mapping token. Other outputs/groups/
inputs keep their Origins, tokens and observations. All scene metadata survives
durable save/reopen and complete retained inventory replay.

## Shared metadata operations

| Command | Target | Parameters besides command/target |
|---|---|---|
| create_scene | None | bus, slot, name, optional description, expected_revision |
| rename | Existing scene | name, expected_revision |
| describe | Existing scene | description (string or explicit null), expected_revision |
| remove_from_list | Existing scene | expected_revision |

Creation requires the original controller generation. Targeted changes require
the original scene generation and expected complete inventory revision. Validate
at normal proxy preflight and again in owner admission; never stamp a newer
token onto queued work. Advertise create_scene as WithReply and reuse the
existing strict Rename/Describe/Removal parsing where appropriate. The existing
targetless describe behavior stays valid; targeted describe accepts Scene only
in this increment. Missing description is refused; null explicitly clears it.

Extend MetadataEdit with CreateScene/RenameScene/DescribeScene/RemoveScene and
map to the existing editor. CreateScene's optional description is metadata, not
hardware configuration. Fenced original-context command completion follows the
existing save/retirement path. HTTP uses that same typed command path with the
current session/CSRF/job limits, deadline and sanitized result behavior. No new
endpoint, background task, timer, wait, retry or UART policy is needed.

Removing from the saved list forgets the identity only: it does not issue native
Remove-from-scene to any fixture. Physical clear remains a separately reviewed
P5h2b operation. Creating or naming a slot likewise does not claim that native
contents exist. Read/recall/edit/capture/copy/clear scene commands remain
unadvertised until their managed paths are implemented.

## Parts and verification

| Part | Files | Behavioral proof/fault sensitivity |
|---|---|---|
| Config/validation/old-file compatibility | config.rs and config/tests | Valid scene JSON first fails before support; invalid/duplicate slots/names, malformed lists, old import and omitted-empty serialization |
| Identity/editor/public State | inventory.rs and inventory/{edit,ledger}.rs; new scene tests | Birth/rename/bus rename/description/removal/recreation, durable replay, unchanged unrelated bindings and complete metadata |
| Independent consumer | tests/support/inventory_consumer.rs and inventory_contract tests | Same consumer object ID on continuity, distinct ID on recreation; reject malformed Scene projection/history/current slot or omitted current Scene objects |
| MQTT owner path | service.rs, service/{routing,proxy}.rs and service scene cases | Real FakeBroker create/rename/describe/remove/recreate, strict payloads, original-token replies/traces, stale token/revision refused, no physical traffic |
| HTTP facade | web job scene test module/index | Authenticated real job saves scene metadata through owner and returns updated authoritative inventory; no separate save logic |
| Compatibility/blast radius | Necessary Bus literals and exhaustive Location arms | Existing output/input operations reject Scene; full native suite and Pi four-bin compile |
| Docs/catalog | inventory contract, MQTT/service/web guides, support matrix, CLAUDE and progress | State exactly metadata support; operator scene controls remain pending |

Add failing tests for the core valid Scene JSON and real command acceptance
before implementation. Add focused controls for range validation, projection/
consumer completeness, continuity and original-target admission, proving intended
behavioral failure rather than compilation failure. Do not duplicate the owner's
entire existing filesystem/retirement test net for each new metadata variant;
exercise the real shared path and any new Scene-specific decision.

Finish with targeted tests/controls, restored native regressions, scoped strict
Clippy, Pi library/four-bin compile, doctests, changed-file formatting, wait
validation, affected control plans and final diff review. Two sequential design
and source reviews cover buildability/correctness, reader completeness and the
parts ledger; disclose same-family/source-only review limits. No browser source
changes are planned; any later user-facing Scene controls belong to P5h3 and its
real functional phone/desktop pass.

## Self-review

Pre-implementation gates: implementer/buildability review APPROVED (40 s), then
correctness/reader-completeness review APPROVED (15 s). Both are same-family,
source-only reviews; no cross-family review or runtime verification is implied.
The continuity approval and current Go authorize this bounded metadata slice.

Round one traces creation through ConfigStore, retained State, consumer replay
and original-token command completion. Round two checks siblings: optional old
files versus malformed new lists; duplicate names across versus within address
spaces; bus rename; description versus binding identity; removal versus hardware
clear; recreation versus continuity; original Scene versus controller generation;
current metadata versus history; input/output resolvers versus new Scene slots.
No new product policy is inferred for stored contents or mixed HA presets.
Documentation/skill coverage is part of the phase; no Store schema, Store memory
or skill modification is needed for this bridge-only increment.
