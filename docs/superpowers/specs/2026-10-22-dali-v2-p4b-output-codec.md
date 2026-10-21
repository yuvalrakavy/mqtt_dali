# P4b1: typed base-output commands and reply semantics

2026-10-22. Overall capability effort remains T3. The architecture and implementation
are owner-authorized; this is the next narrowly bounded implementation contract,
not a replacement plan or permission to guess later DALI extensions.

## Grounding and boundary

P4a is committed as e0d2323. ATX Transactions already separates bus replies from
async frames. Hat owns UART health, and Proxy deliberately refuses fixture writes.
The independent emulator models selected Part 102 commands, but is not protocol
truth. Legacy src/dali_commands.rs supplies existing opcode anchors; the current
Store legacy adapter scales Brightness by 255, which selects MASK at 100%.

Add a pure output codec before enabling any fixture I/O. Keep emulator encoding
independent. It can be exercised without a broker, UART, filesystem, or runtime.
No existing MQTT behavior, configuration schema, v1 source or shared crate changes.

## Contract

Validated ShortAddress 0..63 and GroupAddress 0..15 have private storage.
Destination is Short/Group/Broadcast. Address bytes are short<<1, 0x80|group<<1,
or 0xfe, with bit 0 set for an addressed command/query and clear for DAPC.
Expose direct level commands and Off/RecallMaximum actions. Queries require a
ShortAddress, so code cannot accidentally query a group or broadcast and treat
its combined backward responses as a single fixture's state. All returned frames
are 16-bit values; this module does not write or retry them.

Level has private raw 0..254 storage. Normalized Brightness is the DALI control
level divided by 254, not measured luminous flux or an sRGB/gamma conversion.
Conversion rejects nonfinite and out-of-range values, maps 0 to 0 and 1 to 254,
rounds intermediate values to the nearest level and maps positive sub-step values
to at least 1. Document that min/max settings can clamp the actual result and a
fade can delay it. Property value 255 is never accepted as ordinary brightness.
DAPC 255 has context-specific stop-fade behavior; a stored-scene MASK is a different
operation. Do not introduce a generic untyped "255 means unchanged everywhere".

An Intent takes optional PowerState and normalized Brightness, validates the
entire pair before creating a frame, and refuses an empty pair or contradictory
false+positive / true+zero values. Brightness alone produces DAPC; false alone or
false+zero uses Off; true alone uses RecallMaximum; true+positive uses DAPC.
This is an explicit base policy. HA equipment retains ownership of its separate
RestoreOnPower/PowerOnBrightness behavior; no cached prior level is invented here.
Unknown JSON fields/types are validated by the later service adapter, not coerced
by this typed Rust API.

Query variants: Status, ActualLevel, MinimumLevel, MaximumLevel, PhysicalMinimum,
Version and DeviceType. Encode literal standard opcodes. Query::decode accepts
BusReply and returns an explicit result: decoded value, NoResponse,
ReceiveCollision(code), or TransmitCollision(code). An actual/limit level 255 is
UnavailableMask, never Level 254/1.0. DeviceType 254 is None and 255 is Multiple;
other type numbers remain raw types, without claiming we implement them.
Status keeps the raw byte and named booleans for its eight defined bits. Version
stays raw. Replies for different query types remain distinguishable. NoResponse
never means off, zero, unsupported capability, or successful fixture mutation.

Intent/frame creation is not execution or physical success. The next reviewed
service slice owns original-generation resolution, mutation permits, cancellation,
readback, target availability and publication. It must also probe continuously
ready event backlogs at Transactions dispatch/Hat helper collection before adding
physical jobs; P4a's actor recovery expiry regression does not prove those paths.
Refresh tracing across a bridge-owned queue without OTel is another named probe.

## Files and verification

- src/lib.rs exports new dali module; src/dali.rs and src/dali/gear.rs own types
  and codec, with behavioral tests in src/dali/gear/tests.rs.
- docs/dali-output-contract.md records the actual API, conversion, literal frames,
  limits and next integration boundary. Update progress/support status honestly.
- docs/v2-output-controls.toml and its ledger remove address encoding, max/MASK,
  intent validation and response-type safeguards independently.

Use failing tests before implementation: literal vectors for low/high short and
group addresses, broadcast, boundary rejection,0/1/tiny/fractional values,
all 254 plus off round trips, contradictory/empty intent, each query opcode, all status
bits, level MASK versus type Multiple/None, N and both collision forms. Do not
compute expected protocol frames with the production encoder. Test a frame from
this codec through the existing ATX encoder against a literal UART line as an
integration vector, without sharing it with the emulator.

Run targeted tests, full native regressions/wait registry, strict scoped Clippy,
Pi library/binaries compile check, source formatting, intended negative controls
and restored checks. Two sequential read-only design/source reviewers cover this
contract as required by the overall T3 task; exposed reviewers are same-family.
No hardware-ready, complete P4, MQTT control, discovery or Store migration claim.

## Sources and self-review

Primary implementation cross-check: python-dali 0.16, commit
4b21c18e79d02d2a97f91d135f0eee450e8c2fb9, gear/general.py (Part 102 command values,
status bitmap and response meaning); consulted as protocol facts, not copied code:
https://github.com/sde1000/python-dali/blob/4b21c18e79d02d2a97f91d135f0eee450e8c2fb9/dali/gear/general.py
DALI Alliance terms explains its default dimming curve:
https://www.dali-alliance.org/about-us/terms.html
ATX manual documents transport, not fixture success:
https://atxled.com/pdf/AL-DALI-HAT-I.pdf

Two parent checks traced current codec/owner/proxy/emulator boundaries and the
legacy Store conversion, and checked the proposed positive-value rounding,
MASK semantics by opcode, typed query results and collision/no-response failures.
The borrowed HA component names are verified against current power_and_dimmer.sdl;
no Store edits occur. Existing shared runtime/one-controller topology stays intact,
and optional policies are not inferred from the pilot. The testing memory's
layer distinction is retained: these are codec/bridge tests, not Store-emulator
or physical evidence. Two sequential design reviews approved implementation. The pure codec and
its literal-vector tests are implemented. Two sequential read-only source reviews
approved it; the full local gates and 13 intended behavioral negative controls are
recorded in docs/v2-progress.md. Physical integration remains pending.
