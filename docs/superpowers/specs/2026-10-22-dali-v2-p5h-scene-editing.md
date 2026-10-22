# P5h2b2: private scene contents and nonactivating row operations

## Purpose and boundary

Complete the managed backend required by the approved Scenes screen: persist
desired per-fixture values privately, program and verify a row without activating
it, adopt existing native settings and prepare a capture for review. Existing
stored reads and bus/group recall remain the read/recall boundary. The browser's
reviewed capture, copy, clear, filtered/bulk changes and verify/repair compose
these bounded per-row operations; P5h3 implements those visible workflows.
No native scene contents enter retained Inventory or Store device objects.

Use the same v2 file/ConfigStore and named identity ledger. A second scene file
would split durability and recovery; a browser-only desired copy would disappear
on restart and prevent verification. Keep per-fixture physical jobs bounded,
using the existing service permit and physical owner, with save before program.
Do not introduce a multi-minute bus job, a new UART client, journal or scene
activation inference. Original architecture and continuation authorization apply.

## Shared invariants across readers

| Shared datum | Config/model | Physical owner | MQTT/HTTP | Browser composition |
|---|---|---|---|---|
| Fixture identity | Continuing Origin within the scene's bus | Captured Origin/address and original stamp | Canonical target plus original authority/revision | Captured Origin survives rename, never a reused address |
| Saved row | Private desired setting; absent means unmanaged | Intent to program, never evidence of success | Explicit inspection only; absent from complete Inventory | Saved expectation compared with fresh evidence |
| Native stored row | Not copied into desired implicitly | Complete typed stored report, distinct from actual output | Reviewed evidence for adopt/repair | Review is frozen; incomplete evidence is unresolved |
| MASK | Brightness, colour type and attributes are distinct | Explicitly staged with native semantics | Required structure; null only for attribute MASK | Keep brightness differs from Leave unchanged |
| Revision/token | Private edits advance root; bindings stay stable | Original token/root and same permit across save | Reject stale root or item authority | Batch advances root only through its own accepted results |
| Save/program result | Confirmed durable intent may outlive failed IO | Never activate while editing; verify stored data | Separate saved from verified/unconfirmed | No success badge from save alone |

The scene-specific setting type must retain all-MASK selected families and native
RGBWAF control. Live colour intents cannot represent both; reuse capability
checks, not a conversion through live `colour::Intent`.

## Existing surfaces and readers

`config::Scene` currently contains slot/name/description. `Model::new` validates
structure, replays the ledger and matches active objects before generating the
public snapshot. `state_bytes` currently serializes the full config then adds
targets; a private field must be explicitly removed here. `Edit` and its immutable
change path already advance root revisions without changing binding tokens for
description-only metadata. Output rename/readdress preserves Origin; replacement
and removal retire it. `ConfigStore` already saves proposals, freezes on disk
uncertainty and recovers recognized documents.

`service::groups` and `owner/group_changes` demonstrate probe/save/program under
one retained permit with the original physical stamp and completion. Current
`service::scenes` supports one-shot stored reads/recall and must retain its accepted
behavior. Add a concrete scene-settings work kind for its distinct save/program
lifecycle, reusing existing scene read/frame helpers and checked codec plans.
This avoids refactoring the accepted single-operation request protocol.

The approved mockup is `dali-scenes-design.html`: sixteen slots, inline names,
per-fixture Off/Brightness/Keep brightness/Leave unchanged and applicable colour,
filtered bulk preview, reviewed capture, copy/clear destination/scope, and
Saved↔fixtures verification with selected repairs. All need saved desired rows;
an absent row is unmanaged, not assumed to be ignored.

## Private persisted representation

Add an optional/default-empty `contents` array to each saved Scene. Each entry
references a continuing output **Origin**, plus a typed desired setting. Origin
is the existing birth target/revision proof, not a new UUID or current name.
Resolve it to the current address within that bus. Rename/readdress continues
the entry; remove/replace drops its expectations without touching native slots.
Scene removal drops its private contents. Reject duplicate/dangling/non-output/
other-bus references. Validate references after ledger replay; never infer
continuity from a reused short address. Old missing arrays remain valid, null
and malformed arrays do not. Existing file/state size ceilings remain unchanged.

Desired setting contains an explicit brightness value (0..254 or MASK) and colour
choice: masked type, Tc, xy or RGBWAF. Each selected-family attribute may be MASK;
masked type remains separate. Use native raw codes with checked codec constructors:
Tc 1..65534, xy 0..65534, RGBWAF channels 0..254, plus existing checked control.
Reject null/missing required structure, unknown fields, reserved values and
invalid families before admission. Whole-row Leave unchanged means masked
brightness and masked colour type; Off means brightness zero, independently of
colour. A family with all masked attributes still selects that family.

Private edits change the complete inventory revision while preserving scene and
fixture bindings. The public complete snapshot/ledger contain only scene metadata;
explicit inspection/capture command replies may return desired/observed values
to the operator. Independent retained-inventory consumers must never see contents.
Do not append desired settings to history projections. Test serialization/reopen,
rename/address continuity, replacement/removal, no-ops and public privacy directly.

## Shared per-row commands

Keep `read_scene` (output target + slot) and `recall_scene` unchanged. Extend an
explicit row inspection result with private saved setting and comparison when a
named slot and matching output Origin exist: unmanaged, matching, different or
unresolved. Missing/collided/unsupported/stale evidence cannot compare as matching.
No observation is a claim that a scene is currently active.

Add these strict shared MQTT/HTTP operations:

| Operation | Target/selection | Behavior |
|---|---|---|
| `set_scene_row` | Named scene; `fixture`, `setting`, `expected_revision` | Probe/preflight, save desired setting, program/read back selected row |
| `adopt_scene_row` | Named scene; `fixture`, `reviewed`, `expected_revision` | Fresh probe must match reviewed Stored data; save representable values only; no settings write |
| `repair_scene_row` | Named scene; `fixture`, `reviewed`, `expected_revision` | Fresh probe must match review; program saved desired setting and read back; do not adopt drift |
| `capture_scene_row` | Fixture target; `slot` | Read current actual brightness/selected supported colour and existing stored values; return a candidate, no save/write |

The three row mutations are WithReply with an 18-second original deadline;
capture is Read with eight seconds. `fixture` is a canonical named output target
on the selected scene's bus, not a short-address binding. `reviewed` is the
complete typed `Stored` value from read_scene, requiring complete, nonstale,
supported data. Strict deserialization preserves its Reading variants and rejects
unknown fields. New replies identify scene/fixture targets and tokens, revision,
scope, operation, saved/verified/unconfirmed status and native evidence. Keep
disk outcomes distinct from physical outcomes.

`setting` is a strict object with required `level` and `color`. Level is
`{"kind":"value","value":0..254}` or `{"kind":"mask"}`. Colour is
`{"kind":"mask"}`, `{"kind":"temperature","value":null|1..65534}`,
`{"kind":"xy","x":null|0..65534,"y":null|0..65534}`, or
`{"kind":"rgbwaf","channels":[six null|0..254 entries],"control":null|0..191}`.
Nullable attributes are mandatory fields: null means MASK, missing is an error.
Checked constructors and normalized-mode preflight further restrict writable
RGBWAF control; accepting native raw structure is not a supported-mode claim.
The persisted row uses `output_origin` plus `setting` with these same types.

Read_scene gains nullable `saved` and `comparison` fields; absence of a private
row is unmanaged, never an implicit all-MASK expectation. Capture adds nullable
`candidate` and a reason. It uses the existing low-level output actual read helper
followed by stored inspection under one physical permit, preserving their
sequential-sample nature. Only fully known, supported actual values produce a
candidate; masked/unknown/unsupported active values do not fabricate one. A
capture candidate uses the active native family, not a guessed conversion from
the configured public component family. No capture observation is cached as
stored configuration or retained desired data.

Mutations require the original scene token and exact complete inventory revision.
The revision also pins the selected fixture mapping. Retain the selected Origin,
address and physical stamp through save/program. Never resolve a newer fixture
from a reused name/address after an await. The normal read convention applies to
capture/inspection; HTTP still requires generation/session/CSRF. Original trace,
deadline and SDK completion survive all phases. A saved desired edit never claims
physical success; partial/cancelled writes report saved-but-unconfirmed.

Capture returns the values actually read for review. Applying a reviewed capture
uses Set with that frozen candidate; it does not recapture silently. Copy uses
the reviewed source **saved** setting and Set on selected destination rows; clear
uses the explicit whole-row ignored setting. Preserve unselected fixtures/slots.
An unnamed destination is explicitly named using create_scene before writing;
simply browsing a slot must not create metadata. Multi-row UI batches retain
captured fixtures/settings and advance expected revision only through their own
accepted results; external changes stop the batch. No new backend bulk queue.

## Nonactivating physical program

Known repeat-terminal policy is required before settings work. Probe the fixture
and needed colour features using existing discovery; unsupported/unconfirmed
requests refuse before saving/programming. Allow native whole-row Remove without
inventing DT8 support. Colour writes use confirmed supported Tc/xy/RGBWAF and
assignment/normalized-mode constraints; no Primary-N or alternate-mode control.

Save the desired row first using the existing ConfigStore owner. Reuse the
original physical stamp after disk awaits. Whole-row ignore uses repeated Remove,
clearing both participation types. For a participating row, remove old slot data
then explicitly select a confirmed masked stored report and copy it to temporary
colour for known DT8. This prevents an old pending temporary family from leaking
into a brightness-only Store. Stage all selected-family attributes, including
explicit MASK values, then DTR brightness and repeated Store. Never send ACTIVATE,
DAPC, OFF or Recall as part of editing. Store consumes temporary colour per the
accepted codec/model. Re-read the stored row for verification under the same owner.

Compare stored participation/attributes, not actual output. A failed Remove,
partial programming, unsupported report or mismatching readback remains
unconfirmed; retain saved desired intent for explicit repair. No rollback or
automatic replay is promised. No new UART recovery or pending-state journal.

## Parts, documentation and verification

| Part | Concrete area | Proof |
|---|---|---|
| Private desired model/references | config and inventory editor/model | Fail-first valid persisted rows; strict shapes, Origin continuity, removal/replacement/readdress and public privacy |
| Inspection/comparison/capture | scene read helpers and explicit reports | Literal Off/MASK/family/transport cases; known current candidate independent of stored values; no activation |
| Probe/save/program/settlement | scene-settings request/owner/physical work | Same permit/stamp/completion; save-before-write, original authority, repeat policy, stale disk/physical epochs, partial/cancelled results |
| Native row plans | existing checked scene/colour frames | Independent literal sequence and emulator readback; brightness-only cannot store pending colour; whole-ignore clears colour; unrelated rows/slots remain |
| Shared command path | proxy/PreparedCommand and authenticated HTTP tests | Strict payloads and namespaces, reviewed adoption/repair, stale root/item/fixture mapping refusal |
| Docs/catalog | scene/service/web contracts, matrix, progress and CLAUDE | Operator distinction between private desired, native stored evidence, actual State and pending browser composition |

Complete self-review and both sequential design lenses before implementation.
Build fail-first, then literal/PTY/MQTT/HTTP cases,
focused intentional fault controls, restored scoped lint/Pi/format/wait/plans,
frozen-source reviews and full native regressions. P5h3 owns real browser
phone/desktop validation. No Store schema/memory or general SDK change is needed.

## Owner phases and self-review

Scene-settings work has Probe and Program continuations. Probe retains original
scene authority, selected fixture Origin and the physical stamp. The owner
validates reviewed data for adopt/repair, proposes the private edit, and saves
using ConfigStore/accept_save and the original CommandCompletion (None for HTTP).
Adoption ends after confirmed save without settings frames. Set/repair continue
with the same request/permit and required original stamp; cancellation after save
keeps desired intent and yields saved-but-unconfirmed. A no-op desired edit still
programs on explicit Set, because hardware may have drifted. Repair re-reads and
compares its reviewed native data after disk awaits, immediately before writing.

Before participating-row Store, a confirmed DT8 masked type must come from the
cleared destination slot's report, not an actual-level report. If Remove did not
produce that masked report, stop without silently preserving old stored colour.
Unknown DT8 support may allow whole-row Remove with confirmed fixture presence,
but cannot safely program brightness-only Store with a possibly pending colour;
refuse that participating write until support is known. Current RGBWAF discovery,
assignments and normalized-mode checks apply; no alternate-mode conversion.

All public inventory/SSE/HTTP snapshots use Model::state_bytes (owner publication,
proxy reads and web::handlers::snapshot_value). Strip contents there once and
test MQTT plus authenticated HTTP snapshots. ConfigStore::disk::bytes retains
the full private config and enforces 4 MiB; complete public State still enforces
512 KiB. Its recognized recovery documents compare full Inventory values.
Owner's NotCommitted public-state comparison remains safe for ordinary edits
because every real private change advances the complete root revision; add a
regression for private save failure/no-op/reopen rather than a new recovery policy.

Self-review checks private/current/history readers; exact Origin versus reused
address/name; scene/fixture/bus rename and address swaps; replacement/removal and
unmanaged versus explicitly ignored rows; missing/null/empty data; Off versus
brightness MASK and colour type versus attribute MASK; capability discovery
versus actual/stored family; pending temporary colour; manual/automatic gear
without ACTIVATE during editing; repeat count; original token/revision and stamp
across disk waits; adoption versus repair versus explicit overwrite; no-op saved
data with drifting hardware; partial/cancelled persistence versus programming;
frozen capture/copy data and selected scope. Every backend surface belongs to
this phase; visible bulk/capture/copy/clear review and badges belong to P5h3.
An unmanaged row prevents claiming a fully managed/verified bus scene.

## Implementation checklist and design gate

Both sequential fresh-context design reviews approve (implementer and correctness
lenses, same model family, source-only). No blocking findings. Cross-family and
physical-HAT validation are unavailable; no runtime evidence is claimed by review.
The scene-specific type warning and cross-reader invariant table are explicit
above. Owner's standing implementation approval applies.

- [x] Private Setting/reference serialization, strict shape/codec validation and Origin lifecycle tests.
- [x] Explicit inspection/comparison and actual capture candidate tests, including assigned RGBWAF channels.
- [x] Set/Adopt/Repair retained request and ConfigStore continuation, failure/no-op/reopen tests.
- [x] Nonactivating native row program with cleared masked report and independent sequence/readback tests.
- [x] Shared strict MQTT/authenticated HTTP commands and original authority/review/stamp tests.
- [ ] Intentional fault controls, restored scoped checks, final source reviews and native regressions.
- [ ] Scene/service/web guides, support matrix, progress/catalog; assess relevant skill updates.

Browser composition remains P5h3; checked boxes here do not imply that the visible
screen is implemented.

The backend evidence is in `tests/inventory_contract/scene_settings.rs`,
`src/service/tests/scene_settings{.rs,/cases.rs,/literal.rs,/peer.rs}`,
`src/service/hat/tests.rs` and `src/web/tests/jobs/scene_settings.rs`.
Fifteen new cases cover the checked parts. Fourteen current scene-setting
controls have intended behavioral RED evidence; the original held bus-cycle
case exercised overlapping guards, so its control now uses the existing HAT
fixture with an already-observed cycle to isolate original-stamp admission.
The new scene arm also required qualifying the older group-control anchor;
its real control still has its intended RED. No runtime recovery change or new
test-only runtime hook was introduced.

Relevant skill assessment: no new generic workflow or Store authoring surface;
bridge-local scene gotchas and API names belong in CLAUDE.md and the scene/service
guides updated by this phase. Shared user/Store skills are not modified.
