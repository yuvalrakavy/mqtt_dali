# Input feature and feedback foundation

The approved capability scope includes truthful input indication/feedback. The
current codec/model cover inputs and event settings but have no feature queries
or feature-addressed feedback. A wall-panel LED is an extension of its device or
instance, not another input short address. This T2 foundation establishes the
wire/evidence distinction before managed MQTT/HTTP/browser operations are added.
No owner decision is pending; implementation continues under the approved mission.

## Evidence and shape

Inspected `dali/input.rs`, `emulator/input.rs`, `emulator/model.rs`, scenario
validation and the session reboot owner. Existing input `Address::command` can
build the checked scopes; existing model/query and PTY session are sufficient.
No new runtime, task, timer, cache, persistent identifier or settings shadow.
Old scenarios default to no feedback. Existing sensor/button/common settings and
measurement latches remain independent.

Primary references: IEC62386-103:2014+AMD1:2018 table2, table5 and11.9.14–15;
IEC62386-332:2017 table1 and11.3/11.5; the
[Sunricher SR-2422K8-DA2 manual, pages2–3](https://www.sunricher.com/media/resources/manual/SR-2422K8-DA2%20instruction.pdf)
provides independent literal LED activation/stop examples. Its page3 was rendered
and inspected. The public IEC332 preview was fetched and its capability table
cross-checked. Published standard text is available through the
[IEC332 distributor page](https://standards.iteh.ai/catalog/standards/iec/640c1d67-3f88-483c-a432-86377758d77e/iec-62386-332-2017).
[python-dali](https://github.com/sde1000/python-dali/blob/master/dali/device/general.py)
independently confirms feature query opcodes8E/8F. Generic bridge-development and
memory-recall guidance was read; v1/MQTT/runtime compatibility boundaries remain.

Use a pure feature module and a pure feedback module. Feature queries address
deviceFE or instance00..1F; feedback uses deviceFC or instance20..3F. Opcodes
identify the feature; do not send a DT8 enable or use instance setter semantics.
Activate10/Stop11 and Capability4F/Active4E are single ordinary24-bit commands.
They overlap absolute parameter opcodes only in a different selector namespace.

## Pure API

- `features::Scope::{Device,Instance(Instance)}` and `Query::{Type,Next}` construct
  individual checked frames. Feature numbers are32..96 inclusive. Provide a
  checked `FeatureType` preserving unfamiliar valid numbers and a pure bounded
  decoder of an already captured first byte plus continuation bytes.
- FirstFE means a complete empty list, one32..96 means a complete singleton,
  firstFF requires a strictly ascending list of at least two valid feature bytes
  followed byFE, with at most65 features. Reject missing terminator, duplicate/
  descending/reserved values, trailing data and inconsistent continuation. The
  decoder never treats transport silence/collision as no features. The later
  owner must bound collection, keep the adjacent same-address/selector sequence
  under one permit, prove presence/type and revalidate authority.
- `feedback::{Action::{Activate,Stop},Query::{Capability,Active}}` builds frames
  from that scope/address. No broadcast/group public codec or setting plans in
  this foundation. The checked instance remains0..31. These frames prove neither
  feature support nor actual operation.
- `Capabilities` preserves all raw bits and exposes the six IEC2017 flags
  (visible, brightness, colour, audible, volume, pitch), plus unknown bits.
  Do not reject/drop future bits or claim their meaning. This pure value does
  not authorize a mutation. In particular no HAT N terminal proves support.
  A silent Active query means false only after separate current capability and
  presence evidence; this foundation does not implement such a service owner.

## Independent physical profile

Add optional strict `feedback:[{"instance":0},{"instance":null}]` to an input
scenario. The first selects an existing instance; null selects device level.
Reject duplicates and out-of-range/nonexistent instances. Default empty keeps
existing version1 scenarios/snapshots compatible. This is an explicit visible
on/off profile, capability01, not a universal default or audible/colour/timed
feedback simulation. No new bridge config field or synthetic named endpoint.

Create independent per-profile active=false state, serialized under a separate
`feedback_state` diagnostic key; the scenario `feedback` key remains intact.
Both vectors are omitted when empty, avoiding duplicate flattened keys and
preserving legacy serialized shape. Type8E replies32 when that exact scope has a profile, otherwiseFE for
an existing device/instance. Type8F has no response because this model supports
zero or one feature per scope; the pure parser separately covers multiple lists.
Feature selector20..3F/FC dispatches only to its configured profile. Capability4F
returns01, Active4E returnsFF iff active, otherwise no backward byte. Activate10
and Stop11 apply once immediately. Other feedback operations are unsupported.
Keep feature dispatch before ordinary instance lookup, but never alias it to a
sensor setter or a nonexistent instance. Keep FE device status and old repeat
predicates intact. Existing device broadcast/unaddressed selection still limits
receivers to its bus; feature group/type/broadcast selectors remain unsupported.

Profile state survives address changes and HAT reboot because the simulated
fixture remains powered; a fixture power-cycle/reset guarantee is not added.
Disabled input event generation does not disable this independent feedback.
Preserve sensor/button/common settings, value/latch state, other scopes/buses and
output gear. Existing single-request/reply aggregation policy is unchanged.

## Implementation and checks

1. Fail-first independent literal model cases, then pure literal frames/list/
   capability cases, explicit scenario validation, profile and narrow selector
   admission. Tests vary instance0/31, address0/63, device/instance, present/absent,
   enabled/disabled, old sensor selector overlap, otherbus, unaddressed/broadcast,
   DTR and pending latches. No production codec imports into model tests.
2. Literal standalone UART session roundtrip: fresh feature/capability, activate,
   actual active query, HAT reboot, readdress, stop and actual false query with
   independent current presence. Validate old scenarios and invalid profiles.
3. Mid-checkpoint/diff/comment self-review. Serial fault controls for wrong scope,
   query opcode, FF/FE enumeration, single-send activation and cross-instance scope.
4. Freeze; sequential native/codec and model/compatibility source lenses. Full
   native once; strict scoped Clippy/Pi/format/wait/whitespace and affected control
   plan checks. No web source/assets changed, so no redundant browser pass.
5. Update codec/emulator/service-boundary guides, support matrix, CLAUDE and
   progress with observed evidence. No skill/runtime/dependency change expected.

Managed capability/feedback commands, scalar bindings, web controls, configurable
timing/brightness/colour/audible profiles, group/type feedback and natural input
stimuli remain required subsequent work. This foundation does not claim those
public capabilities are complete. No live HAT/Store, publication or deployment.

## Self-review

Round1 traced all existing input selector admission/dispatch and scenario callers;
emulator Input construction uses serde fixtures, so adding an optional field does
not require unrelated struct-literal edits. Round2 checked namespace overlap,
feature type32..96 bounds, FF=many versus FE=none/end, disabled inputs, transient
HAT versus fixture reboot, and no-response active evidence. Correction: no-feature
and enumeration terminator areFE, notFF; the parser and literal tests must make
the distinction explicit. No additional state machine is needed for this model's
single feature. Same-family review limitation applies.

## Design verdict and implementation checkpoint

Fresh combined implementer/reviewer design gate APPROVE, source-only on
GPT6Astra/high. The snapshot clarification above was incorporated before code:
scenario feedback and runtime feedback_state are separate omitted-empty vectors.

After correcting a test-only method-name typo, the first runtime baseline fails
all3 literal model cases on unsupported scenario fields or absent Type8E replies.
The implemented pure codec and independent profile pass24 focused cases, including
17 neighboring button/sensor/common model/UART cases and all7 new codec/model/PTY
cases. A final7-case run adds idempotency, feature-short-address namespace and a
three-feature unterminated list that isolates the terminator guard from length
validation. Source review confirms no production await/owner/task/config write
or public service/UI change; existing repeat predicates remain unchanged.

All7 selected intentional faults produce their intended REDs,94.0s measured
command time. The first terminator row had a whitespace anchor rejected before
mutation; copying the full condition corrected it and its isolated rerun detects
the missing-end list. No production fix was required. Mutation sources were
restored after each row. Logs: `/tmp/dali-feedback-foundation-controls.log` and
`/tmp/dali-feedback-foundation-control-end.log`; ledger:
`docs/v2-feedback-foundation-controls.ledger.json`.
