# Native scene protocol and emulator

This contributor guide covers the native scene codec and physical emulator.
Named-slot metadata, stored-scene reads, native recall and private per-fixture
settings use shared MQTT/HTTP jobs. The Scenes screen and its reviewed multi-row
capture/copy/clear workflows remain pending.
No scene operation here has been validated on a physical HAT.

## Named-slot metadata

Each bus can save sixteen named slots with an optional description. A name is
independent of the slot's physical contents: creating a name does not program
lights, and removing it from the saved list does not clear any fixture's scene.
Saved per-fixture settings remain private to the bridge; the public catalogue
contains no mirrored scene settings or active flag.

Commands use the standard BridgeKit command envelope with these parameters:

| Command | Target | Parameters |
|---|---|---|
| `create_scene` | Controller | `bus`, `slot`, `name`, optional `description`, `expected_revision` |
| `rename` | Scene | `name`, `expected_revision` |
| `describe` | Scene | `description` (string or explicit null), `expected_revision` |
| `remove_from_list` | Scene | `expected_revision` |

For example, `Bus1.Scenes.Evening` identifies a scene; its slot is 0..15.
Use the original controller generation for creation and the original scene
generation for targeted edits, together with the expected complete inventory
revision. Missing description is refused by `describe`; null clears it. Unknown
fields, duplicate names/slots and invalid slots are refused. Rename preserves
identity; removal/recreation creates a new lifetime. An unrelated controller
edit does not replace the scene generation. Both transports use the existing
durable metadata owner; HTTP requires its normal session and CSRF protection.

The [inventory contract](inventory-contract.md) explains replay and compatibility.
Behavioral controls are in `v2-scene-identity-controls.toml`.

## Stored reads and native recall

| Command | Target | Parameters | Result |
|---|---|---|---|
| `read_scene` | Individual output | `slot` (0..15) | Stored configuration evidence, including unnamed slots |
| `recall_scene` | Named scene | `expected_revision`, optional `group` (0..15) | `outcome: sent` after a current correlated command terminal |

Read uses the usual optional-generation MQTT read convention; a supplied token
must match. HTTP requires its normal generation envelope and session/CSRF.
Recall requires the original scene generation and exact complete inventory
revision. Omitting `group` broadcasts on that scene's bus; a numeric group scopes
the same slot to physical group members. Null, malformed or out-of-range group
values are refused. The group need not have a saved name. Recall is momentary;
it does not save or clear scene contents, change inventory or publish a trigger.

The read report identifies target/generation, revision, bus/address and slot.
`stored` contains `level`, nullable `dt8`/`color_type`, a `values` map, `quality`,
`stale` and an optional reason. Reading tags preserve transport evidence:
`value`/`mask` have `raw`; `no_response` has no value; `receive_collision` and
`transmit_collision` retain an optional code. Off is `{"kind":"value","raw":0}`;
Leave unchanged is `{"kind":"mask","raw":255}` for brightness. Word MASK is
65535, while a word with only its high byte FF remains numeric.

Confirmed DT8 reads preserve the stored colour type and selected supported
Tc/xy/RGBWAF attributes. Raw temperature is the native reciprocal-temperature
code; xy and RGBWAF values are native DALI codes, not converted display units.
A masked type differs from a selected family containing masked attributes.
Primary-N and unknown/contradictory types remain explicitly unsupported.
Unknown capability or missing/collided data remains partial. `complete` describes
query completeness, not physical certification, calibration or a writable setting.
Scene query plans own all DTR/report steps and stop on missing/collided word
parts. Capability/actual probes finish before selecting the stored report.
Stored reads neither activate lights nor replace their actual State.

`sent` does not confirm participation, resulting levels/colour or completed fades.
Recall invalidates cached actual values at admission and again after settlement
across the bus, including for group scope
because physical membership may differ from saved groups. Failed/cancelled recalls
that might have reached the wire also invalidate them. Admission happens before
any possible send, so a caller timeout cannot expose old values as current while
the UART owner is still cleaning up. State then follows the
existing Available=false/Quality=stale contract; stale colour fields are null.
Base PowerState/Brightness may remain merged last-known values, gated by
availability. Refresh obtains actual readback; no old desired command is replayed.
Recall is refused while a continuing fixture on that bus is being identified,
including Blink dwell/restoration. Stop identification before recall.

Contributors use `ServiceHandle::read_scene`/`recall_scene` and the existing
physical owner. Original context, one admission permit, deadline, settlement and
final mapping/physical-stamp checks apply. No additional UART client or scene
cache exists. Controls: `v2-scene-operation-controls.toml`.

## Private per-fixture settings

The bridge stores desired scene rows in the existing v2 configuration file.
Each row references the fixture's continuing ledger Origin. Renames and verified
address changes preserve it; removal/replacement drops its saved expectations.
A replacement at the same short address does not inherit them. Removing a scene
name removes its private rows without changing hardware. Old files with no
`contents` remain valid. Public Inventory, its history, HTTP inventory snapshots
and SSE snapshot events contain scene metadata only. Explicit command results
are protected operator data; job summary feeds omit results. All authenticated
operators have equal configuration authority. Store keeps opaque scenes.

An absent row is **unmanaged**. An explicitly ignored row is a saved expectation
that the fixture will leave its brightness and colour unchanged on recall.
Inspection adds nullable `saved` and `comparison`: unmanaged, matching, different
or unresolved. Incomplete, unsupported or stale evidence cannot be matching.

| Command | Target | Required parameters | Result |
|---|---|---|---|
| `set_scene_row` | Named scene | `fixture`, `setting`, `expected_revision` | Probe capabilities, save desired row, program and verify native stored data |
| `adopt_scene_row` | Named scene | `fixture`, `reviewed`, `expected_revision` | Re-read reviewed stored data and save it; no settings writes |
| `repair_scene_row` | Named scene | `fixture`, `reviewed`, `expected_revision` | Re-read review before programming saved intent; no adoption of drift |
| `capture_scene_row` | Output | `slot` | Read actual output plus stored evidence; return a candidate without saving |
| `read_scene_settings` | Named scene | None | Complete saved settings and current output identities from one snapshot, without UART IO |

`fixture` is the canonical output target on the named scene's bus. `reviewed`
is the complete `stored` object from inspection. If native data changes, refresh
and review it again. Mutations require the original scene generation and exact
complete inventory revision; this revision also pins the selected fixture.
The fixture generation remains independent. Capture follows the normal MQTT
read convention; HTTP still requires generation, session and CSRF.

`read_scene_settings` returns `target`, `target_generation`, complete root
`revision`, `bus`, `slot`, and `fixtures`. Each fixture entry contains `fixture`,
`fixture_generation` and nullable `saved` Setting. Every configured output on
that bus appears exactly once; null means unmanaged. It reads one immutable Ready
snapshot and needs neither enabled bus nor UART. A supplied MQTT token must match;
HTTP still requires its original token. The local read budget is two seconds and
the complete HTTP result budget is 1 MiB. No physical evidence is claimed and no
native scene is queried merely to display saved settings.

Each setting has required `level` and `color` objects:

```json
{"level":{"kind":"value","value":0},"color":{"kind":"mask"}}
```

Level is `value` (native 0..254, including Off=0) or `mask` (keep brightness).
Colour is one of:

```json
{"kind":"mask"}
{"kind":"temperature","value":350}
{"kind":"xy","x":null,"y":20000}
{"kind":"rgbwaf","channels":[254,0,80,null,null,null],"control":128}
```

Tc uses native mirek 1..65534; xy uses codes 0..65534; channels use 0..254 in
red/green/blue/white/amber/free order. Nullable attributes are mandatory fields:
explicit null means MASK, while missing is refused. RGBWAF control null means
MASK; writable numeric control must use normalized mode and confirmed physical
assignments. Primary-N and alternate control modes remain unsupported.
A selected family with all attributes MASK still selects that family. Level
MASK plus colour-type MASK means Leave unchanged for the whole row.

Set/Adopt/Repair retain one permit and original request deadline (18 s) through
probe, durable save, programming and stored readback. Capture has eight seconds.
Stop identification on that bus before these operations. Settings writes require
an explicit known HAT repeat policy. A no-op Set still programs: unchanged saved
intent does not imply unchanged hardware. Capture uses the active native family,
including only assigned RGBWAF channels, independently of the public component
selection. A null candidate and reason preserve unknown/unsupported actual data.
Actual and stored reads are sequential samples; no coherent instant is promised.

Replies identify scope, targets/tokens and revision, with `configuration_changed`,
`saved`, native `stored`, `comparison`, optional `candidate` and `reason`. A
confirmed save can survive a failed, partial or cancelled physical operation:
`outcome: unconfirmed` preserves desired intent for explicit inspection/repair.
`verified` requires current stored readback. Save uncertainty refuses programming
and uses existing configuration recovery. There is no rollback, retry or journal.

Programming removes the old native row first. On confirmed DT8 it queries the
cleared slot's masked colour type and copies that report into temporary colour,
then stages the complete requested family, brightness and repeated Store. A
failed Remove stops before Store. This prevents pending colour leaking into a
brightness-only row. Whole-row ignore uses Remove. Editing never sends ACTIVATE,
direct levels, Off or Recall, and does not publish stored settings as actual State.
Unknown DT8 support permits only whole-row Remove after confirmed presence;
participating writes must first establish DT8 presence or absence.

The [Scenes screen](scenes-browser.md) composes these bounded operations. Apply a capture's
frozen candidate with Set; copy reviewed source saved settings with Set; clear
selected rows using the whole-ignore setting. Preserve unselected rows/slots.
Name an unnamed destination explicitly before writing. Multi-row batches must
advance their revision only through their own accepted results and stop on
external edits. No backend multi-minute bus queue exists.

Contributors use `service::scene_settings` and concrete
`physical::Work::SceneSettings`, beside the accepted read/recall work kind.
Fault controls: `v2-scene-settings-controls.toml`. UI validation belongs to P5h3;
local PTY evidence does not certify a physical fixture.

## Storage and participation

Each output fixture has sixteen native slots, numbered 0–15. Group and broadcast
recalls address those same slots; they do not create another set of scenes.
`dali::scenes::Slot` checks the slot before constructing a frame.

| Stored data | Recall behavior |
|---|---|
| Level 0 | Switch off |
| Level 1–254 | Recall that DALI code, subject to fixture limits |
| Level MASK (255), known colour type | Change colour; preserve brightness |
| Known level, colour type MASK | Change brightness; preserve colour |
| Both level and colour type MASK | Ignore this fixture; preserve pending colour too |

`Level::Mask` is valid stored data, distinct from a missing or collided reply.
A selected colour family with all attributes masked is also distinct from masked
colour type. An empty staging plan cannot establish masked type: it would leave
earlier temporary values pending.

Store records DTR0 brightness and the selected temporary colour. Masked temporary
type **preserves the slot's previous colour**. Remove clears both level and
colour participation. Both settings require repetition, encoded as
`Step::Repeat` for the ATX owner's configured repeat policy. Neither activates
the light. Temporary colour is consumed after Store or activation.

## Staging and report queries

`colour::Intent::staged_frames` stages a validated live intent without ACTIVATE;
its existing `frames` retains the complete live sequence. `scenes::Colour` also
accepts independently masked Tc/xy/RGBWAF attributes, including all-MASK values
with a selected family. RGBWAF includes a control byte; `RgbwafControl` refuses
reserved mode bits and an omitted control means MASK. No capability is inferred.

On activation a selected family still becomes active when all its attributes
are MASK. The model preserves its stored numeric values while changing the type.

`ReportValue::plan` first queries the scene level, then supported Tc/xy/RGBWAF
report selectors in 224–240,
including the LSB query for word values. QueryActualLevel would replace that
report with actual colour and must not interrupt the sequence. Stop on a failed
query. Existing byte/word decoders preserve MASK, no response and collisions.
Unknown type bytes stay unsupported; recognizing Primary-N's type byte does not
provide Primary-N staging.

Keep staging, Store and report queries under the existing exclusive bus permit:
DTRs, device-type enable and report selection are shared state. The codec itself
performs no I/O or locking. CopyReportToTemporary copies the selected report,
including masked type/attributes, without ACTIVATE. A managed writer can Remove,
query the masked slot and copy its report to clear earlier temporary state before
writing a complete reviewed entry.

Recall copies scene colour into temporary state. Automatic activation applies
it; manual gear waits for explicit ACTIVATE. Future managed recall must account
for the setting and exact participants, without changing the gear mode or
broadcasting ACTIVATE to ignored fixtures.

## Independent physical model and checks

The standalone emulator implements sixteen colour records, separate temporary
and report data, RGBWAF control and scene clearing in the existing base Reset.
Scenario version 1 and configured level scenes remain valid. Colour scenes start
masked and are programmed through native commands. QueryActualLevel and
StoreActualLevelInDTR select actual colour for reporting.

Actual RGBWAF control reports unsupported channel links as zero while preserving
the selected mode. Tests vary physical profiles from one through six channels.

The model covers immediate values and masks. Fades, photometry, colour-space
conversion, RGBWAF linking/control-mode effects on arc power, other DT8 settings
and hardware conformance remain outside this increment.

Literal codec/model tests and the standalone PTY test exercise repeats,
nonactivating storage, auto/manual and colour-only recall, ignored fixtures,
report separation, partial values/control, Remove and Reset. Sensitivity controls
are in `v2-scene-controls.toml`. Run:

```sh
cargo nextest run --offline --target aarch64-apple-darwin --lib --test hat_emulator -E 'test(scene_)'
```

See the [grounded design](superpowers/specs/2026-10-22-dali-v2-p5h-scenes.md),
[emulator guide](emulator.md), [capability matrix](support-matrix.md) and
[phase status](v2-progress.md).
