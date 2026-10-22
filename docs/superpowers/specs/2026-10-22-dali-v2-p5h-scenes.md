# P5h: native DALI scenes

## Approved behavior and implementation boundary

Continue the approved scene architecture: slots 0–15 per bus/fixture, opaque
scene objects for Store, group/bus recall, reviewed capture/edit/verify/copy/clear.
Off and Leave unchanged differ. A colour-only scene can preserve brightness;
whole-row Ignore removes both level and colour participation. Editing storage
must not activate the light. No PanelScript or live Store work is included.
Standing architecture/workflow approval applies; no new product decision is
needed for the first implementation boundary. Keep the mission's T3 discipline.

The dependency missing before managed jobs/UI is a complete native scene codec
and independent physical colour-scene model. Implement **P5h1** first, then extend
durable scene identity/settings and owned jobs (**P5h2**), then wire the approved
Scenes screen (**P5h3**). The later boundaries require their grounded contracts
before code. P5h1 does not make the current Scenes placeholder a working control.
Budget: about ten minutes design/reviews, 25–40 minutes codec/model/verification.

Selected shape: typed protocol primitives plus the existing independent emulator,
followed by the existing durable owner and job façade. Reuse the existing bus
permit/deadline/trace architecture in P5h2, rather than inventing another owner.
Rejected shapes: browser-only scene sequences bypass shared MQTT authority;
ordinary property presets would lose native scene/MASK behavior; putting fixture
settings into Store contradicts the approved opaque hardware-scene abstraction.

## Grounded facts and cross-boundary invariants

| Claim | Verified evidence | Reached from |
|---|---|---|
| Scene target namespace already exists | `src/targets.rs`, `Namespace::Scenes` | Existing target codec/parser |
| Scene location/config/managed operation are absent | Enumerated config/inventory/service and `physical::Work` variants | Current service routing; current browser Scenes placeholder |
| Base emulator has 16 level slots and query/store/remove/recall | `src/emulator/model.rs`, opcode arms 0x10–1f/40–5f/b0–bf | `Model::transmit` from PTY session |
| Physical colour scenes/report selection are missing | Full `src/emulator/colour.rs`; `State::command/value` | Same Model DT8 and base query dispatch |
| Existing live colour plans activate | `src/dali/colour.rs`, `Intent::frames` | Current owned colour execution |
| Live value plans issue QueryActualLevel | `ValueQuery::plan` | Current scalar colour readback |
| Repeated settings already have transport support | `atx::Transactions`, RepeatPolicy; groups settings | Existing physical groups/commissioning work |
| Current file/public inventory bounds differ | Config 4 MiB, published inventory 512 KiB | Config validation/Model construction |

| Datum | Codec | Independent model | Later service/UI |
|---|---|---|---|
| Slot | Checked 0..15 | Sixteen independent slots | Bus singleton scene identity; group scope uses same slots |
| Level 0 / 255 | Off / MASK | Store exact, recall/clamp only known nonzero levels | Distinct Off / Leave unchanged controls |
| Colour type MASK | Preserve stored colour on ordinary Store | Does not erase previous colour | Full Ignore uses Remove, not level MASK |
| Whole-row Ignore | Remove-from-scene setting | Clears level and colour type | Explicit operation, then stored readback |
| Stored versus actual | Query scene report, no actual query | Report storage is independent of actual and temporary | Verification does not claim scene currently active |
| Manual activation | Native recall primitives only | Recall loads temporary; auto bit governs activation | P5h2 explicitly handles manual gear; never changes its mode silently |

Primary protocol grounding: IEC 62386-209:2011 §§9.11–9.12 and Table 11,
cross-checked against the vendor's scene implementation. The following concise
rules are factual protocol constraints, not copied implementation:

- Store records DTR level and selected temporary colour; masked temporary type
  preserves prior stored colour. Temporary values are cleared after use.
- Remove masks both scene level and scene colour type.
- Recall copies stored colour to temporary; activation follows the automatic
  activation setting. Both scene level and colour type masked means no effect.
- Query scene level loads the corresponding colour report. Querying actual level
  instead would replace that report with current output values.

Sources: [IEC Part 209](https://www.normsplash.com/FreeDownload/137722437/IEC-62386-209-2011-en-fr.pdf),
[vendor scene code](https://github.com/wirenboard/wb-mqtt-dali/blob/main/wb/mqtt_dali/dali_type8_parameters.py),
[Part 209 command definitions](https://github.com/sde1000/python-dali/blob/4b21c18e79d02d2a97f91d135f0eee450e8c2fb9/dali/gear/colour.py).
No third-party source is copied into the codec or independent model. Physical
firmware timing and conformance remain unverified.

## P5h1: protocol and physical model

Add `src/dali/scenes.rs` with checked Slot and stored Level (known 0..254 versus
MASK), literal short/group/broadcast recall and short-address query/store/remove
frames. Settings carry an explicit repeat distinction; they cannot masquerade as
ordinary one-shot sends. Do not execute I/O here or infer hardware support.
Expose typed report selectors for scene colour type, Tc, xy, RGBWAF/control,
and independent byte/word replies. A scene-colour read plan first queries its
scene level, then report values (selectors 224..240), using DT8 enable and word
LSB retrieval. It never inserts QueryActualLevel. No response/collision and MASK
remain distinct; raw type values outside known families remain unsupported data.

Refactor `colour::Intent` to expose **staged_frames** that sets temporary values
without ACTIVATE; existing frames appends its unchanged enable/ACTIVATE pair.
Existing live intents retain their nonempty validation and every current literal
frame sequence. Scene-specific stored colour data may contain masked attributes,
including a selected family with all masked attributes; it is not a live intent.
Provide explicit CopyReportToTemporary. The later managed writer can preserve
stored data or reset a slot with Remove + Query + Copy before storing a complete
reviewed entry, without activating output or replaying stale temporary values.

Make stored-colour staging explicit: a scene colour value can select Tc, xy or
RGBWAF with independently masked typed attributes, including all-MASK attributes.
The MASK colour **type** is a separate reported state, not an empty live intent
or an empty staging plan that would accidentally retain pending temporary data.
Copying a known masked scene report to temporary is the explicit primitive for
that case. Reuse the actual-frame builders where possible without exposing
unchecked raw live intents. RGBWAF has a typed control byte as well as channels;
the emulator tracks actual control separately so recall/ACTIVATE and current
queries remain consistent when a stored non-default control mode is used.

Extend the independent emulator's colour State with bounded sixteen scene colour
records, a separate report and full temporary RGBWAF control. No new scenario JSON
format is needed: colour slots initially masked, tests prime them through literal
native commands. Preserve configured level-scene fixtures. Add independent base
command hooks for Store, Remove, Recall, QuerySceneLevel, QueryActualLevel and
Reset. Store consumes temporary state after storing; masked temporary type keeps
the prior stored colour. Remove clears colour type and level. Query only changes
report/DTR data. Recall handles level and colour independently and honors auto
activation; manual gear remains staged until explicit ACTIVATE. Ignored entries
must not accidentally activate earlier pending colour. Reset clears new scene
storage alongside the existing level reset.

Report selectors read their report snapshot, while existing actual/temporary
selectors keep their distinct sources. CopyReportToTemporary copies full masks,
type and control without activation. Scene colour values retain raw 16-bit xy/Tc
and 8-bit channel MASK values; RGBWAF control is stored/reported explicitly.
Do not import bridge codecs/types into emulator code. Do not add tasks/timers,
new UART recovery, blanket retries, certification claims or unrelated settings.

## Parts and verification

| P5h1 part | Code surface | Verification and fault probe |
|---|---|---|
| Slot/level/frame contract | `dali::scenes` | Literal first/last short/group/broadcast frames, invalid slots, Off/MASK and repeat flags |
| Nonactivating stage | `colour::Intent::staged_frames` | All three families/partial masks stage without ACTIVATE; existing live vectors unchanged |
| Stored report plan | Dedicated scene selector/plan | Literal scene query then report selector/MSB/LSB; fail if actual-level query is inserted |
| Store/remove/query | Independent colour/model hooks | Settings require repeat, no actual change, masked temp preserves stored colour, Remove clears both, other slots/gear unchanged |
| Recall/activation | Same model | Off, level-only, colour-only, both masked, partial xy/RGB channels; auto/manual modes; no pending-colour activation for ignored gear |
| RGBWAF control/masks | Temporary/scene/report | Exact control byte, channel masks and copy-report without activation |
| Report separation/reset | Same model | Scene report differs from actual, query has no actuation, reset clears all sixteen new slots |
| Integrated wire path | Existing standalone PTY/transactions harness | Native frames query/program/recall physical fixture under original transport policy; no broker/live UART |
| Documentation + skills | Scene contract, emulator/support/progress/CLAUDE | Describe raw semantics and remaining managed/UI work; no Store skill or memory changes needed |

Fail behaviorally before implementing the critical scene/model rules. Add focused
controls for silent-mask/activation/report mistakes, restore and run meaningful
local tests. Finish with native regressions, scoped strict Clippy, Pi four-bin
compile, doctests, changed-file formatting, wait validation and affected control
plans. Existing P7 browser files remain unchanged during this pure boundary.
No new loop means no new wake/idle-CPU mechanism; existing PTY actor ownership is
unchanged. Use two sequential same-family design/source reviewers and disclose
the unavailable cross-family independence.

Self-review round 1 left the document: traced actual codec/model call paths and
the native Store/Remove/Query semantics in primary sources. Round 2 checked
siblings: level-only versus colour-only, all-MASK versus no response, auto versus
manual, current versus stored report, ignored gear with pending colour, query
side effects, reset and unsupported colour families. The later managed/UI work
is explicitly unbuilt here, not implied by passing protocol tests. Apply
ways-of-working/staged-design-review for the phase and writing-store-docs
principles for contributor/operator documentation.

## P5h1 implementation record

Both sequential design reviewers approve (45 s and 96 s; same-family,
source-only). The implementation exposes `dali::scenes`, staged live intents and
the independent model hooks described above. Primary §9.12.4 also explicitly
requires temporary RGBWAF control to select RGBWAF type; a separate failing-first
case establishes that control-only scene staging clears prior pending Tc.

Every P5h1 part in the table has its code and a named test: five literal codec
tests in `dali/scenes/tests.rs`, nine literal physical tests in
`emulator/model/scene_tests.rs`, and
`standalone_native_scene_codec_programs_and_queries_physical_colour_over_pty`.
The existing live-colour vectors remain exercised. The whole scene-filtered run
has sixteen cases including the pre-existing base scene test.

All fifteen scene sensitivity controls have intended behavioral REDs, followed
by source restoration. Three affected older controls are also retargeted:
scope repeated anchors to their owning functions and clone the extended
Temporary shape. One initial input-traffic retarget matched `transmit_input`,
while its test exercises `intervening_frame`; that GREEN was a wrong-site probe,
not successful sensitivity. The corrected probe targets the tested method.
Logs: `/tmp/dali-scenes-controls` and `/tmp/dali-scenes-controls-final`.
The corrected three affected controls have intended REDs too. Restored
acceptance passes 544 native tests, scoped strict Clippy, Pi library/four-bin
compilation, doctests (none), changed-file formatting, all eight affected control
plans, whitespace and wait validation. No production wait/loop was introduced.
Logs: `/tmp/dali-scenes-acceptance`. Frozen source reviews follow.

First source review requests one correction (55 s): selected Tc still becomes
active with a MASK numeric value, matching the already implemented xy/RGB rules.
The added cross-family test fails before the fix, then covers all six changes
in auto/manual modes. Condition only Tc's numeric update; preserve its value for
MASK while selecting its family and consuming temporaries. Both older doc claims
are corrected. Sixteen focused cases and the sixteenth scene control pass their
declared checks; two affected old controls remain RED. Restored acceptance and
frozen rechecks follow. See `/tmp/dali-scenes-review-controls`.

The first corrected-source recheck approves (10 s). The second full review
requests unsupported RGBWAF link bits read zero (55 s; IEC command 251). A
three-channel case first fails BF versus 87, then a literal matrix verifies all
one-to-six channel counts, the three supported mode bit pairs, actual report,
recall and later MASK preservation. Normalize actual links at activation; scene
storage and mode bits retain their existing semantics. Seventeen focused cases,
the seventeenth scene control and two affected RGB controls pass their declared
checks. Logs: `/tmp/dali-scenes-links-controls`. Final restored acceptance and
both source rechecks follow.

Final frozen `2e46c31` is approved by both sequential source rechecks (10 s and
3 s). Seventeen scene controls have intended RED evidence; restored acceptance
passes 546 native tests, scoped strict Clippy, Pi compile, doctests, formatting,
all eight affected plans, whitespace and wait validation. Logs:
`/tmp/dali-scenes-links-acceptance`. Reviews are same-family/source-only and ran
no verification; the parent executed checks. P5h1 is accepted locally. P5h2
managed configuration/jobs, P5h3 UI and physical conformance remain unbuilt.
