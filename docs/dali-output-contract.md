# Base output codec (v2 development)

`dali::gear` provides validated Part 102 addresses, control levels, basic write
frames and query replies. It is a pure library: constructing a frame does not
send it, verify a fixture or change configuration. The
[output service](output-service.md) and [MQTT adapter](mqtt-outputs.md) execute
base intents and readbacks. The emulator retains its independent codec/model.

## Addresses and writes

`ShortAddress::new` accepts 0..63; `GroupAddress::new` accepts 0..15. Their storage is
private. `Destination` selects either of those address types or Broadcast.
`set_level`, `off` and `recall_maximum` return 16-bit DALI frames. Queries are
available only through `ShortAddress::query`, since group/broadcast backward
responses cannot provide one fixture's collision-free observation.

Literal maximum-level / Off / RecallMaximum vectors:

| Destination | Direct level 254 | Off | RecallMaximum |
|---|---|---|---|
| Short 0 | `00FE` | `0100` | `0105` |
| Short 63 | `7EFE` | `7F00` | `7F05` |
| Group 0 | `80FE` | `8100` | `8105` |
| Group 15 | `9EFE` | `9F00` | `9F05` |
| Broadcast | `FEFE` | `FF00` | `FF05` |

The existing ATX encoder wraps Group 15 maximum on zero-based bus 2 as
`2h9EFE\n` (the final escape denotes the newline byte).

`Level` accepts raw 0..254. Its normalized form is `raw / 254`; this is a position
on the DALI control scale, rather than a measured luminous-flux percentage or a
color-space conversion. From normalized input, 0 means Off and 1 means 254;
intermediate values round to the nearest code, with positive sub-step values
kept at least 1. Nonfinite values and values outside 0..1 are rejected. The fixture's
minimum/maximum and fade settings can affect the subsequent actual level.

255 is excluded from ordinary `Level`. For direct arc-power commands it is a
special stop-fade value; scene MASK has separate semantics. Never scale 100% to 255
or assume every occurrence of 255 means the same thing. The legacy Store adapter
currently uses a 255 scale; its separate P8 v2 migration must correct that.

`Intent::new(power, brightness)` validates the entire optional pair before
producing one frame:

| Input | Action |
|---|---|
| Brightness alone | Direct arc-power level, including 0 |
| Power false, with absent/zero brightness | Immediate Off |
| Power true, brightness absent | RecallMaximum |
| Power true, positive brightness | Direct arc-power level |
| Empty pair, false+positive, true+zero | Reject |

Off does not use the configured fade time; direct arc-power changes do. This
codec keeps no last-brightness cache. Logical HA equipment owns its separate
RestoreOnPower/PowerOnBrightness policy. The MQTT JSON adapter rejects
unknown properties and incorrect types before calling this typed API.

## Queries and observations

Queries cover Status, ActualLevel, MinimumLevel, MaximumLevel, PhysicalMinimum,
Version and DeviceType. Their literal opcodes are tested independently of the
encoder. `Query::decode` accepts only a `BusReply` already correlated by the ATX
transaction owner. Do not turn an arbitrary monitored frame into that evidence.

`QueryReply` preserves NoResponse and both collision directions/codes. These
results do not become off, zero, unsupported capability or successful mutation.
Successful values retain their query-specific `QueryValue` variant. Level 255 is
`LevelValue::UnavailableMask`; DeviceType 254 means None and 255 means Multiple,
while other type numbers remain `Specific(raw)`. A reported type is not a claim
that its extended commands are implemented. Version remains an uninterpreted byte.

Status retains the raw byte and eight named flags, including gear/lamp failures,
lamp power, limit error, fade running, reset, missing address and power failure.
The fade flag is true while running. Status 255 is a bitmap, not a level MASK.

The service resolves original generations, serializes actual owned jobs, validates
response counts, rejects stale observations after session loss and publishes
readback. An ATX N after a write is transport completion, not fixture confirmation.
This pure module does not implement those service responsibilities or DTR/color/input
commands. See [capability status](support-matrix.md) and [progress](v2-progress.md).

The protocol cross-check is
[python-dali 0.16 Part 102 commands](https://github.com/sde1000/python-dali/blob/4b21c18e79d02d2a97f91d135f0eee450e8c2fb9/dali/gear/general.py),
with the [DALI Alliance terminology](https://www.dali-alliance.org/about-us/terms.html)
for control-curve terminology. Source facts were checked against the local legacy
opcode anchors; emulator agreement alone is not hardware or standard validation.

`dali::groups::state` separately summarizes captured individual-address evidence
for a frozen saved roster. Every roster entry participates, including unvisited
ones. It distinguishes uniform power from mixed levels, known empty membership
from off, full-byte masks/status from level MASK, and complete from partial raw
evidence. Duplicate/outside samples reject. It performs no query, send, cache or
configuration mutation. The [native group service](group-output.md) supplies
owned captures and explicitly limits coverage to saved inventory.
