# P4d2: independent DT8 HAT-emulator semantics

## Purpose and shape

The pure codec is reviewed, but the physical emulator still treats DT8 commands
as unsupported. Extend that existing model so later colour service/MQTT work can
be exercised through the real PTY. This T2 scenario/protocol extension is within
the approved mission; no new owner choice is needed. One focused design/source
review carries both implementation and protocol lenses. Budget one implementation
batch, targeted fail-first tests, controls and one restored regression gate.

Reusing `dali::colour` would make agreement circular. A separate simulator would
duplicate lifecycle/fault handling. Add independent semantics under the existing
model, preserving private PTYs, strict scenario validation and existing controls.

## Model and files

- `src/emulator/scenario.rs`: optional `colour` on Output, omitted when absent.
  Existing scenario version 1 stays readable unchanged. A colour profile requires
  device_type 8, at least one representation, and an explicitly supported active
  representation. Optional xy pair is raw 0..65534; optional temperature contains
  initial value and ordered physical/configured mirek limits (1..65534); optional
  RGBWAF vector contains 1..6 channel values 0..254. No inferred RGB capability.
  Active type is the explicit enum xy/temperature/rgbwaf, with no primary-N model.
- New independent `src/emulator/colour.rs`: private runtime model from the profile,
  actual values plus staged temporary values/type, features/status, Tc range flag.
  No production `dali` imports. Snapshot exposes actual and staged raw values for
  test inspection. Base output level remains independent.
- `model.rs`: add optional runtime colour per gear and one broadcast enabled-type
  latch per modeled bus (every fixture hears the same C1). The next standard 16-bit command consumes
  the selection across the bus before address filtering and before a repeat-first
  early return. Special DTR frames preserve it; 24-bit input events are ignored
  by the gear and preserve it. The existing `intervening_frame` still clears the
  base repeat-pair state, but must not clear DT8 selection. A C108 must enable a
  selected DT8 extended command. No-enable/wrong-type/non-DT8 commands cannot change colour or
  fabricate a positive device response. Existing base addressing/selection/reply
  collision aggregation and DTR storage remain the single path.
- Implement temporary X/Y (E0/E1), Tc (E7), RGB/WAF (EB/EC), ACTIVATE (E2), gear
  features/status (F7/F8), type features (F9), value query (FA), RGBWAF control
  query (FB), extended version (FF). Other extended opcodes remain unsupported.
  RGBWAF control reports the fixed normalized mode 0x80; mode/configuration and
  channel-assignment writes remain a later explicit extension.
- Temporary setters replace the addressed temporary fields, including MASK;
  MASK overwrites an earlier staged numeric field rather than retaining it.
  Temporary values/type start MASK and return to MASK after ACTIVATE. Setters
  selecting another representation clear that previous family's staged values.
  Within RGBWAF, RGB and WAF triples accumulate until activation. MASK fields
  leave actual channels unchanged at activation; repeated ACTIVATE without a
  new setter does nothing. Tc zero is ignored; Tc MASK does not change actual Tc
  or select it as active. Setters never change arc power. ACTIVATE applies the
  selected representation, keeps unrelated
  representation values and untouched channels, clamps Tc to configured limits,
  and sets the corresponding active bit/range flag. Unsupported representation
  requests are ignored. No colour conversion or xy gamut claim.
- Value selectors match the codec's required current x/y/Tc (0/1/2), RGBWAF
  channels/control (9..15) and Tc limits (128..131). Active representation values
  are readable; inactive/unsupported values yield MASK (no recalculation model).
  Word replies place MSB on the bus and in DTR1, and LSB in the gear DTR0; byte
  values preserve both registers. Unsupported word selectors yield FFFF; unsupported byte
  selectors yield FF. Distinguish actual no-response from a MASK byte.
- Queries and writes keep existing short/group/broadcast fixture selection.
  Different simultaneous values collide; identical values follow the emulator's
  existing aggregation policy. Address/group changes do not replace fixture
  identity. The existing base RESET path is unchanged; Part 209 reset/settings
  semantics remain unsupported and must not be inferred from base RESET success.
- Add a separate `examples/emulator/colour-i4.json` (at most four buses); preserve
  the existing basic scenario. No new control command or wait is required.

## Verification and documents

Literal model tests first fail on absent colour validation/semantics. Cover strict
  scenario errors, mixed/Tc-only/RGB fixtures, one-shot type gating and preserved
  selection through 24-bit input events, staging versus activation, staged numeric
  overwritten by MASK, repeated ACTIVATE, all six channel MASKs, Tc
clamping/limits, active/inactive/MASK query widths and word byte order, selected
  groups/broadcast, non-DT8 unchanged, duplicate reply collisions and unchanged
master level. A standalone process test loads the colour scenario, sends literal
ATX 16-bit sequences through the existing Transactions/PTY path, observes real
J replies and checks private snapshot/termination. It does not use the new codec
as its expected-value oracle. Later service tests may use the production codec
against this independently authored model.

Guard-removal controls cover enable consumption, ignored unsupported capability,
staging/MASK, byte order and Tc clamp. After restoration run full native tests,
strict scoped Clippy, Pi compile, changed-file format/wait/whitespace and doctests.
Update emulator guide, colour guide, support matrix, contributor catalog and
progress. No SDK, Store, deployment, recovery or shared-skill change.

## Self-review

Round 1 read existing scenario/model/session dispatch and literal PTY process
tests: extending current selection/collision/DTR handling avoids a parallel path.
Round 2 traced C1 versus addressed opcodes, forward-frame consumption, inactive
values, unsupported channels and absent-profile compatibility. Physical gamut,
fades, colour scenes, DT8 reset/settings and RGBWAF mode changes remain explicit
limitations; the initial reset-to-scenario idea was removed because it lacked
primary evidence and is unnecessary for ordinary colour integration.
Primary facts use the same pinned Part 209/102 and manufacturer sources as P4d1;
implementation remains independent of both the codec and external source code.

## Design review amendments

The combined gate correctly found that MASK overwrite and activation cleanup
needed explicit transitions. Primary gear source now supports the chosen model:
OpenDALI_EVG commit `c1f73d0261c79ae1def6c4a615dcb49f7dc335dd`,
`Firmware/src/dali/protocol/dali_dt8.c` (verified blob
`ce065391005c0a92c5df099b47a3b5ddc2baa56e`) stages MASK and clears temporary
values/type after activation. Its `dali_protocol.c` ignores 24-bit frames,
preserves enable through special commands and consumes it before standard command
address filtering. That contradicts the draft's overly broad every-forward-frame
rule, which has been corrected. Accordingly no InputEvent invalidation hook is
added. This independent, uncertified gear implementation is cross-check evidence,
not proof of universal firmware behavior or formal standards conformance.

The narrow re-review approved these amendments. It withdrew the initial input
invalidation recommendation after inspecting the cited source. No third static
review or new recovery mechanism was needed. Reviews were read-only/same-family.

## Implementation and verification evidence

The profile validation passed first, while four independent literal model tests
failed behaviorally on the prior unsupported DT8 path. Six new model tests now
cover the specified capability/selection/MASK/activation/value behaviors, and a
seventh test exercises the actual standalone executable, PTY and private control
socket with a native input event between enable and query. A test helper's missing
transaction type parameter was corrected before the successful process test.

All eleven focused controls failed for their named behavioral reasons in 63
seconds; the two affected existing addressing/configuration controls also failed
as intended. After restoration, all 338 native tests pass, as do strict scoped
Clippy, Pi library/all four binary compilation, changed-file formatting,
whitespace, wait registry and doctests (none defined). No new wait or actual
resource owner was introduced. A fresh read-only source gate follows the frozen
implementation commit; physical certification remains unclaimed.

The source gate identified a missing word-query side effect in DTR1. The pinned
reference explicitly stores MSB there as well as returning it; a new literal
regression reproduced stale AA instead of 00. The one-line fix updates DTR1 and
the regression checks both normal/MASK words plus known/MASK byte preservation.
The new guard-removal control and two affected word/byte controls failed for their
named assertions. The corrected restored gate passes 339 native tests and the
same scoped lint, Pi, formatting, whitespace, wait and zero-doctest checks. A
final narrow source re-review follows the frozen correction commit. This shows
why shared-register effects need assertions alongside the visible reply byte.
