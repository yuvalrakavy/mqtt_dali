# DT7 switching protocol and independent physical model

Tier T2: Part208 command framing plus a new independent fixture model. This is
the foundation for the already approved DT7 capability, not a claim of finished
managed settings, relay-specific MQTT metadata, browser controls or certification.
Bound design to two self-review rounds and one fresh combined gate; implement
inline, then measured faults/freeze/sequential source reviews. No owner decision
is needed within the approved capability/architecture.

## Problem, options and existing mechanisms

Current `emulator::model` accepts device_type7 but models every output as a
dimmer. Its `config.level` is used for both command calculation and actual query
replies; it reports physical minimum1 and has no Part208 extended-command path.
`dali::gear` already supplies checked addresses, destinations, levels, native
Part102 queries and settings steps. The emulator has one independent byte decoder,
bus-local enable-type latch, per-bus adjacent base repeat tracking and bounded
PTY/session ownership. No new transport, task, timer or protocol framework.

Options: approximate a relay by a dimmer; add a separate emulator outside the HAT
model; add a DT7-only state beside the existing independent colour/input models.
Choose the last. It preserves one UART/emulator path while separating virtual
control level from actual contact output. No change to the approved web design.
Memory search (including ignored files, with a known BridgeKit hit as control)
found no prior DT7-specific decision; the approved architecture and sources below
govern this slice.

## Checked native library

Add `dali::gear::switching`. Four checked thresholds preserve explicit disabled255:
upward thresholds1..255, downward0..255. Zero upward input is refused by the
library; the physical model separately clamps a raw received upward zero to1.
Do not reuse ordinary actual-level MASK decoding for a threshold.

Pure setter plans use broadcast DTR0, one ENABLE DEVICE TYPE7, then the selected
short/group/broadcast threshold command as a repeated command. Reuse the existing
settings Step record; execute all steps under one future bus permit/repeat policy.
Opcode mappings E1/E2/E3/E4 correspond to upward-on/off and downward-on/off.
Query plans accept only a checked short address: one enable then F0 features,
F1 switch status, F2..F5 thresholds, F7 gear type or FF raw extended version.
Decode only correlated BusReply; preserve silence and receive/transmit collision
codes. A received invalid upward zero remains an explicit invalid raw reading.
Preserve raw feature/status/type/version bytes. Decode only established common
feature bits3/7 and gear-type bits0..4; switch-status raw is diagnostic, not an
actual contact-position proof. Legacy load-error/reference/hold-off features
and units are outside this first slice; do not infer them from reserved bits.

## Independent DT7 model

Add a separate `emulator::switching` module with literal opcodes and no bridge
codec imports. Add optional strict `switching` scenario profile, valid only for
device_type7: adjustable_thresholds (defaultfalse), four native thresholds
(default[1,255,255,0]), raw gear_type (default2). Fixed profiles require default
thresholds. A type7 without a profile has that fixed default relay model.
DT7 initial Output.level must be0 or254; other device types retain existing rules.
The new profile is optional, so existing non-DT7 scenario/snapshot bytes do not
gain null/default fields. A live switching_state is emitted only for DT7.

DT7 starts with virtual level equal to the supplied binary actual level, limits
254/254 and physical minimum254. Its state owns current virtual level, actual
contact state, thresholds, last-threshold evidence and a pending repeated setter.
The snapshot's ordinary level and QUERY ACTUAL LEVEL stay actual0/254. Native
power and lamp-on status reflect the contact, never positive virtual level alone.
STORE ACTUAL LEVEL IN DTR stores the virtual level for DT7 (Part208 override).

Route each existing immediate base level change through Gear helpers: DAPC,
OFF/MAX/MIN/step operations, scene recall and limit clamping. Calculations use
the virtual level for DT7 and the ordinary level for other types. MASK changes
neither. Positive levels clamp to current virtual min/max;0 remains independent.
Only adjustable DT7 profiles accept native MIN/MAX changes below254, bounded by
1..opposite limit. Raising/lowering a limit can move virtual level and operate
the contact. Threshold setters alone retain the contact unchanged. Reset restores
default thresholds/limits, virtual254 and On; profile capabilities remain fixed.
No timed fades, UP/DOWN simulation, mains/bus-failure transitions, electrical
load faults or reference measurement are claimed in this immediate-state slice.

For a changing virtual level, evaluate only that direction's enabled pair.
Retain contact state when no threshold applies. A jump across distinct thresholds
has the outcome of the last threshold encountered in that direction; equal
thresholds give On priority. Preserve raw last-threshold status without claiming
startup evidence that was never observed. Same-level control retains state.
Cover normal hysteresis, reverse switching, two applicable thresholds, ties and
all-disabled profiles with literal model expectations.

Keep Part208 repeat authority in each DT7 state, separate from existing base
repeat handling. The first enabled setter waits for a matching second addressed
frame within100ms. The second frame must execute without a new enable. An extra
enable, DTR change or other command addressing that gear aborts the pair; traffic
for another address does not. Expired/unmatched/wrong-type setters do not change
thresholds. Cancellation observation must occur even when the outer existing
base-repeat guard defers its first frame. Preserve existing Part102/DT8 behavior.
No rewrite of existing repeat recovery or UART policy.

## Implementation and evidence

1. Fail-first checked codec/literal model tests: threshold endpoints/disabled255,
   exact setter/query plans, raw invalid/unknown/collision evidence, default binary
   actuals versus virtual DTR, optional fixed/adjustable profiles and strict
   scenario validation. Initial failure must be behavioral, not a missing symbol.
2. Implement the pure library, independent state, profile and scoped Gear helpers.
   Audit every existing level read/write in model.rs: classify actual query/status/
   snapshot versus virtual command/STORE ACTUAL calculations. Other gear remains
   behaviorally identical. Query-only operations never operate the contact.
3. Literal regressions cover hysteresis/reversed/tied/disabled thresholds, limits,
   nonactivating programming, scenes/RESET, selected addresses/groups/buses,
   both repeat terminal profiles, timing boundaries and addressed versus unrelated
   intervening traffic. Extend the standalone HAT process test to execute typed
   plans through UART/PTY and independent readback. Add an existing managed base
   output read/power roundtrip proving public actual values are binary.
4. Measure intentional faults for enable/duplicate handling, actual-versus-virtual,
   threshold255/tie/direction/nonactivation, fixed-profile writes and limit/STORE
   ACTUAL overrides. Restore all source before freeze/readers.
5. Update emulator/DT7/output guides, support matrix, CLAUDE and progress with the
   precise foundation boundary and remaining managed/UI/timing work. Run scoped
   cases, full native, strict Clippy, Pi library/four binaries, changed formatting,
   waits/whitespace and affected control plans. No browser source/assets change
   is planned; retain the latest1013-command acceptance unless integration changes
   make another browser run necessary. Fresh combined design review followed by
   sequential final native/codec and model/compatibility source gates; available
   reviews are same-family. No live HAT/Store, SDK, memory/skill, publication,
   release, push/merge or deployment changes.

Self-review round1 checked source paths and primary native overrides, including
virtual STORE ACTUAL and the one-enable/two-setter diagram (locally rendered after
web screenshots failed). Round2 checked neighboring base-repeat early returns,
all level mutation families, fixed MIN/MAX and actual-versus-virtual consumers,
simultaneous thresholds, profile defaults and unchanged non-DT7 bytes.

References: [IEC public preview, clauses9–11 and Figure2](https://cdn.standards.iteh.ai/samples/15977/473f21f6cbf04867a4035e9424f134a9/IEC-62386-208-2009.pdf),
[Beckhoff overview](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/10378581387.html),
[native variables](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/10378583307.html),
[repeated setter](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/10375643403.html),
[manufacturer command table, pp7–8](https://www.controlfreakuk.com/images/download/LIDA-D-User-Manual.pdf).
The latter has Type6 typos in adjacent prose; use the IEC diagram for enable and
repeat sequencing. Legacy optional features are not used to define this profile.
