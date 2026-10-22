# Native scene protocol and emulator

This contributor guide covers the native scene codec and physical emulator.
Managed scene configuration, MQTT/HTTP jobs and the Scenes screen are pending.
No scene operation here has been validated on a physical HAT.

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
