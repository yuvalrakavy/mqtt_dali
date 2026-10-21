# P4c: common DALI-2 inputs

2026-10-22. Continue the approved switch/input feature using the existing UART
owner and BridgeKit runtime. First deliver the input protocol codec, then connect
discovery, named instances and MQTT state/events to that codec. No shared-SDK
outage extension is required. The owner accepts rejecting commands during rare
bus outages; assess likelihood, impact and simpler alternatives before expanding
infrastructure.

## Real problem and smallest useful shape

The ATX parser already preserves 24-bit L frames. Hat::event currently ignores
their input meaning. The target codec already supports
Bus1.Inputs.WallPanel.Instances.EntranceKey, but saved inventory currently contains
only outputs/groups. Common buttons, maintained switches, occupancy and light
sensors therefore cannot be represented or consumed through MQTT.

Use a pure Part 103/301–304 codec plus the existing service and input hierarchy.
A second UART reader would compete for replies; raw frames alone would force
every consumer to implement DALI; recognizing gestures in the bridge would
duplicate the native device's short/double/long-press events. No manufacturer-
specific protocol is presumed.

## First implementation boundary: codec

Add src/dali/input.rs and its tests; export from dali.rs. No I/O, persistent
configuration, new scheduler, hardware mutation or MQTT contract change yet.

- Validate input short addresses 0..63 and instance numbers 0..31 separately from
  output addresses. Preserve unknown instance types rather than declaring support.
- Decode all standard event-source schemes: device/type, device/instance,
  instance/type, device group/type and instance group/type. Preserve the original
  24-bit frame and 10-bit information. Bit 16 distinguishes commands from events;
  a monitored command must never become a button press.
- Device/instance events need the discovered instance type. Leave them unresolved
  until supplied; do not assume every instance is a button or guess its target.
  Explicit source types remain authoritative if a supplied type disagrees.
- Part 301 actions: released, pressed, short press, double press, long start,
  long repeat, long stop, free and stuck. Preserve native actions without a new
  gesture recognizer or invented pressed/released state for action-only events.
- Part 302 exposes the raw 10-bit absolute position. Part 303 exposes occupied,
  movement, repeat and movement/presence sensor flags. Part 304 exposes the raw
  10-bit relative light measurement. Unknown/reserved event data remains raw;
  it must not fabricate a valid state or action.
- Build individual read-only 24-bit queries for device status/version/number of
  instances and instance type/resolution/status/enabled/event scheme/input value,
  input-value latch and event filters. Device queries use instance byte FE;
  individual instance queries use their validated number. Do not reuse 16-bit
  control-gear addresses or the repeated-16-bit t command.

An explicit type does not establish a unique physical endpoint. Device/type can
match several instances of that type in one module; instance/type contains no
device identity; group sources can also be ambiguous. The service will resolve
against discovered metadata or leave the event unassociated, rather than route
it arbitrarily.

The pinned code covers the generic 10-bit event field but has no Part 302-specific
decoder. The DALI Alliance identifies that type's event as position. Keep its
field raw here; normalization or additional position semantics require their own
verified definition in the integration slice.

Light measurements are not universally lux. The pinned python-dali primary
reference describes relative values, and Theben's own manual specifies a
product-specific conversion. Preserve raw values; do not apply an unrelated
universal logarithmic/lux formula. Resolution, calibration and normalized
position belong to the later typed service representation.

## Proof for this boundary

Use independently written literal frames for every source scheme and address
extreme, command/event distinction, contextual type resolution and each button
action. Include representative absolute, occupancy and light values. Literal
query vectors must verify address, command bit, instance byte and opcode without
using the production builder to generate expectations. A valid unknown type or
event stays unknown; invalid range/source combinations cannot yield a press.

Run focused tests, the native suite, strict scoped Clippy, Pi compilation,
format/whitespace/wait registry and meaningful controls for the protocol masks,
type resolution, action values and query address bytes. Keep tests proportional:
no transport-failure campaign or SDK refactor for this pure codec.

Update support matrix and a contributor guide to state exactly that the codec
exists while the end-to-end input feature remains in progress. Commit locally.

## Following vertical boundary

Use the same protocol module to query addressed input devices and their instances,
extend saved inventory/identity projections with Inputs/Instances, and route
continuous events to their current named mapping. Publish current state separately
from non-retained native actions through BridgeKit. Start unknown until observed,
keep read-only instances read-only, and provide emulator switch/sensor controls
with independent literal protocol behavior. Common discovery/state/configuration
and input feedback remain required feature work; this first codec is not the
completion claim for switches. Freeze that integration detail against the actual
codec before changing persisted input inventory or the MQTT surface.

## Protocol evidence

Primary cross-check: python-dali commit
4b21c18e79d02d2a97f91d135f0eee450e8c2fb9, dali/device/general.py,
pushbutton.py, occupancy.py and light.py. Read its command/event definitions;
implement independently, without copying its source or sharing emulator code.
The DALI Alliance's sensor page confirms common Parts 301–304 and native events:
https://www.dali-alliance.org/dali/sensors.html.
Theben PlanoSpot manual's section 8.2 illustrates product-specific light scaling:
https://www.theben.de/ocsmedia/optimized/full/o31363v42_PlanoSpot_360_DALI2_S_DE_WH_Hand_book.PDF.

Parent source pass checked ATX frame widths, Command::Send's l encoding,
Hat::event, existing target encoding, input reference schemes and standard
query definitions. The alternative to adding this codec is repeating DALI
interpretation in each frontend; the codec solves a present feature gap with no
runtime policy expansion. Review focus is standard wire meaning and useful
integration boundaries, not rare transport behavior.

The first source-grounded design review approved this codec boundary. Its two
clarifications are incorporated: explicit type is not unique endpoint identity,
and Part 302 data stays raw rather than acquiring unverified scaling semantics.
The second sequential design review also approved, highlighting literal cases
for reserved source combinations, reserved occupancy bits and high data bits;
all are represented in the planned tests. Reviews use the same model family.
