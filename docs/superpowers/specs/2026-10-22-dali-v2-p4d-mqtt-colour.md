# P4d6: selected-family MQTT colour control and observations

## Intent and shape

The owner approved one configured public colour representation per physical
fixture, while exposing all known hardware formats. P4d5 persists that contract
and fences changes; this increment makes it useful through scalar MQTT Desired
and actual State. Keep one target, the existing BridgeKit scalar mailbox and the
existing owned physical colour/base operation. Explicit families avoid competing
retained intents; dynamically changing the public type or adding family devices,
a bridge coalescer or SDK policy framework are rejected for the approved reasons.

T3: public wire behavior and physical mutation. Standing implementation approval
applies; two sequential design/source gates. Scope: individual DT8 outputs, pure
partial-xy support, selected-family parsing, observed values/capabilities and
initial configured-colour polling/budgets. Split-address RGB composition, group
execution, Primary-N writes, gear settings, colour scenes, Web/Store work and any
UART recovery changes remain separate approved capabilities. No implicit RGB/xy
or Kelvin/RGB conversion, hardware mode changes or retry framework.

## Writable values and whole-request validation

Continue PowerState/Brightness and add these scalar properties, usable both as
per-property Desired topics and fields in a target's Desired object:

| Configured family | Property | JSON number and unit |
|---|---|---|
| `temperature` | `ColorTemperature` | Kelvin, converted to nearest valid mirek |
| `xy` | `ColorX`, `ColorY` | Coordinate in 0..65534/65536 |
| `rgbwaf` | `ColorRed`, `ColorGreen`, `ColorBlue`, `ColorWhite`, `ColorAmber`, `ColorFree` | Independent normalized channel, 0..1 |

One configured `rgbwaf` family serves RGB/RGBW subsets according to confirmed
physical assignments; it does not promise six present channels. Channel zero is
valid and independent of master brightness. Power/brightness semantics retain
their existing validation, including contradictory pair rejection. Missing base
or colour values are unchanged, never default on/zero. No colour request is an
implicit on request.

The entire map must validate against the current persisted selection before
physical admission. Reject empty maps, unknown/read-only properties, wrong types,
nulls, range errors, mixed colour families and unconfigured/non-selected families.
Keep SDK admission validation and execution-time validation; original generation
checks prevent a selection change between frontend validation and physical work
from retargeting a request. Existing typed library colour APIs remain explicit
operations independently of this MQTT policy.

Use `WriteIntent` Base/Colour/Combined and the corresponding existing handle API.
Combined maps get the previously reviewed full preflight before either mutation,
ordered off/colour/on commands and fresh actual readback. Capability/mode/assignment
errors still reject before combined mutation; wire faults keep existing uncertainty.

Add `Intent::xy_parts(Option<Coordinate>, Option<Coordinate>) -> Result`, rejecting
both absent. Keep `xy(x,y)` as pair convenience. Its complete plan always loads
both setters: an absent coordinate gets word MASK 0xffff so previous temporary
values cannot leak into this partial request. Preserve ACTIVATE and ownership;
never read an old coordinate into a new desired intent or add a second queue.

## Observed State

Keep base Available/Quality independent. Each output projection also writes:
`ColorRepresentation` (configured string/null), `ColorAvailable` Boolean,
`ColorQuality`, `ActiveColorRepresentation` (sampled string/null),
`ColorCapabilities` (fresh evidence object/null), and all nine colour scalar
properties above (actual numbers or explicit null). Explicit nulls retire stale
properties in BridgeKit's merged State on mode changes, missing replies and
invalidation. A target is not created by observations; input paths are unchanged.

ColorAvailable requires a live same-generation observation, current colour
quality, active kind matching selection, usable required values, and normalized
RGBWAF mode when selected. The colour quality reports unconfigured, unobserved,
stale, unavailable, unsupported, partial, representation_mismatch,
unsupported_mode or current. No stale/inactive/raw-channel value may masquerade
as current normalized output. Active format may still be known during a partial
read and does not change the persisted public contract. Current values follow
actual mirek/65536/254 conversions, allowing clamping and sampled fades.
Quality precedence is: no selection means unconfigured; no matching colour
sample means unobserved; a failed lifecycle or stale outer/colour observation
means stale; otherwise preserve unavailable/unsupported/partial colour quality,
then test active-family match and normalized mode before declaring current.

ColorCapabilities uses the same fresh observation and contains nullable `dt8`,
`supported_representations`, `primary_count`, `rgbwaf_channel_count`,
`rgbwaf_assignments`, `rgbwaf_control`, `automatic_activation` and
`temperature_limits` (Kelvin for configured/physical coolest/warmest). Enumerate
known supported formats from validated feature counts, including `primary_n`
when declared, without implying every format/mode is writable. Known non-DT8
reports an empty supported list; unknown/invalid feature data reports null.
Physical assignments retain ordered known colour names and null for unrecognized,
unassigned, MASK or failed entries; no slot-position inference. Limits decode only
known positive words; missing/zero/MASK stays null. Lifecycle/mapping invalidation
clears capabilities rather than asserting a fresh absence. The service's typed
raw evidence remains available independently for diagnostics.
The four limit keys are `coolest_kelvin`, `warmest_kelvin`,
`physical_coolest_kelvin` and `physical_warmest_kelvin`.

## Reading, timing and ownership

Configured colour must be sampled on the very first ordinary output read/write,
even without an earlier library read_colour. Continue sampling already known
same-generation colour targets. Basic unconfigured/never-sampled outputs retain
their two-query path. Share the existing service owner/permit and whole-plan HAT
helper; native input frames remain continuous.

Select budgets before admission: plain 2 s, configured/known-colour ordinary reads
and base-only writes 6 s, colour/combined writes 8 s. The existing 15 s All-read
ceiling, cursor fairness, 20 s SDK operation bound and explicit caller deadlines
remain; no per-frame renewal. A literal paced peer must demonstrate why the
longer write budget is required and catch regression to a shorter bound. A normal
incidental colour query failure still cannot fail a successful base-only write.
Verify during implementation: the proposed 8 s ceiling must pass the paced
six-channel+Tc combined path, and a 6 s regression must fail its behavioral
completion assertion. Do not claim the numeric timing choice verified from this
design calculation alone.

## Grounded implementation and proof

Owners: `service/proxy/state.rs` currently parses only base fields and projects
base observations; both Apply call sites in `service/proxy.rs` invoke it.
`output_budget` has apply/target-read/All callers. `ServiceHandle::output` already
adds colour reads after a same-generation sample, but not from configured metadata.
`dali/colour.rs` currently requires both xy coordinates and already emits MASK for
unrequested RGBWAF channels. `output::WriteIntent` and all three public apply APIs
already exist. Enumerated with `rg -n 'state::intent|output_budget|include_colour'
src/service` and complete owning functions; no SDK change is needed.

Add a small `service/proxy/colour.rs` for family intent parsing, projection and
capability conversion; keep base validation in `state` and full actor ownership
in output. Reuse private broker/PTY/emulator fixtures and the independent literal
colour peer. Do not import production codec into emulator physical behavior.

Test first through real Desired/refresh: initial configured-colour read discovers
all capabilities; temperature/xy/RGBWAF individual and combined writes change
independent physical state and report actual clamped values; scalar partial xy
and RGB preserve other channels and power, including prior staged temporaries;
whole invalid maps never mutate base or colour. Pure tests cover strict numeric
families and literal xy MASK plans. State tests cover mismatch, missing replies,
non-normalized RGB mode, stale lifecycle, unknown/malformed capabilities and
explicit null clearing in actual merged MQTT State. Retired tokens/family replay
cannot mutate after a configuration change. Native events/original contexts use
the existing tested owned path. A six-channel+Tc 50 ms paced peer exercises the
full write/readback budget; it deliberately ignores setters, so expected actual
values must match its independent answers rather than the desired numbers.

Run focused fail-first/positive tests and meaningful guard-removal controls, then
the restored native suite, strict scoped Clippy, Pi compile, format/whitespace,
wait-registry and doctests. Update MQTT output/colour/service guides, support
matrix, contributor catalog and progress. Record same-family source-review and
physical hardware limits honestly; no reusable skill or shared SDK change.

## Self-review and gate verdict

One persisted family solves the approved replay ambiguity without pretending to
order unrelated Desired families. Complete validation precedes physical work;
the actor generation recheck closes configuration races. Partial setters use MASK,
not inferred old desired values. Values require fresh compatible actual evidence;
capability support never fabricates availability. Explicit nulls address the
existing merged State behavior. Budgets cover read-before-write and read-after-write
within the unchanged ownership/deadline mechanism.

Design gate surely yes: sequential buildability and experienced correctness
reviews, with false-positive valid scalar subsets, false-negative malformed
families, capability/mode truthfulness, and real timing path coverage. Audit the
chosen shape without reopening the owner's settled representation preference.

Design gate 1 approved with no material blocker after source grounding. The 8 s
budget remains explicitly provisional until the real paced MQTT apply regression
passes and detects a shorter-bound fault. Read-only/same-family inspection only;
no builds, tests or mutations performed.

Design gate 2 approved the selected-family wire/ownership contract with no
material blocker. It retained the paced 8 s/6 s evidence obligation. Review was
read-only and same-family, with no verification or mutations performed.

## Implemented evidence

Three real-MQTT/emulator tests failed on missing selected-family colour State
before production edits, then passed with the new parser/projection/owned dispatch.
Twelve focused cases now pass: actual clamped Tc with combined brightness and
original trace, scalar xy/RGB preservation, whole invalid-map refusal, merged
State null clearing, native assignment-role order/unknowns, bad feature/mode/
inactive evidence, partial xy literal plans and independently staged-mask effects,
generation/family retirement across reconnect, and first configured/complete
write timing. Existing service API signatures, SDK policy, UART recovery and hardware settings are
unchanged. No colour request implicitly turns power on.

The reconnect test initially sent deliberately invalid and healthy current-token
properties back-to-back; under coalescing they correctly formed one invalid
batch and both were rejected. Broker errors proved that mechanism. The test now
observes the invalid request's rejection before testing a later healthy scalar;
no runtime retry, filtering or coalescer workaround was added.

The six-channel+Tc peer with 50 ms response pacing passes the complete MQTT write
in 8 s; changing its budget to 6 s fails the intended completion assertion.
The first configured read also passes its paced path and detects a plain 2 s
regression. All **13 new controls** and **3 affected existing timing controls**
fail at their declared behavioral assertions. Existing timing-control anchors
were updated for the extended call signature without changing their faults.

Restored native suite: **382 passed**. Scoped strict Clippy, Pi library/all four
binary compilation, changed-file rustfmt/whitespace, unchanged wait registry and
doctests (none defined) pass. Logs: `/tmp/dali-mqtt-colour-controls` and
`/tmp/dali-mqtt-colour-restored-gate`. Source acceptance follows the frozen commit;
no hardware/firmware, live Logmon, Store, merge or release validation is claimed.

Both sequential source gates approve frozen `5e7dc2e` against `96c818a`, with no
material findings. Reviews inspected source/tests/controls only and performed
no builds, tests, mutations or external operations; same-model-family limits apply.
