# Single-output service API

This guide is for bridge contributors and embedders. The shared service can read
and control a saved individual DALI output through its UART owner. The same paths
serve [MQTT base output properties](mqtt-outputs.md) and the protected browser UI.
Groups, color and addressing
have separate execution contracts; this API does not reinterpret a group as an
individual address.

Explicit common settings and diagnostics use `read_output_settings` /
`configure_output` through this same owner, with separate reports and no scalar
settings cache. See [fixture settings](gear-settings.md) for strict patches,
physical/current preflight, original revision and matching actual confirmation.

Start the managed service with UART enabled, then use its existing ServiceHandle:

```rust,ignore
let observed = handle.read_output(target, original_context, deadline).await?;
let intent = mqtt_dali::dali::gear::Intent::new(Some(true), Some(0.5))?;
let observed = handle.apply_output(target, intent, original_context, deadline).await?;
```

`deadline` is an absolute Tokio Instant. `original_context` is the caller's owned
BridgeKit RequestContext. Writes require the exact saved target-generation token.
Reads may omit it; an explicitly supplied token must match. A returned observation
contains the token captured during actual address resolution, plus the original
trace cause. It never adopts a newer token after a rename or session change.

The typed Intent validates the complete power/brightness pair before admission.
Power alone recalls the configured maximum; off uses immediate OFF. Brightness
uses DALI control-code position divided by 254, including zero for off. It is not
a conversion from luminous flux. Contradictory values are rejected before I/O.

Each operation shares the service's nonwaiting admission with metadata edits.
Busy means another operation still owns that admission. The owner resolves the
name to the enabled bus and short address again after enqueue, then preserves
that mapping until actual backend settlement. A frontend timeout/cancellation
does not undo an admitted write. Inspect the owned snapshot before retrying an
uncertain operation.

Queued canceled or expired operations do not transmit. Started operations may
finish within their original operation deadline after the caller disappears.
Absolute deadlines propagate through transaction admission, receive fences and
write polling. Bytes already accepted by the kernel or HAT may still produce
later physical effects; software cancellation does not recall them from the wire.

After its helper actually joins, the service proves backend retirement by taking
the same transaction admission permit. A terminal session requires actual
close/join even if that permit is free. Cleanup has one fixed 1 s ceiling and may
outlive the caller's deadline; service admission remains held throughout. A failed
teardown fails the managed service instead of admitting another physical write.
The managed process stop retains its existing shared shutdown ceiling.

## Observations

A write's ordinary N terminal means transport completion. The service then queries
actual level and status at the individual short address. Successful write
completion requires usable post-write fixture samples. It returns the observed
control level even when the fixture clamps it or a fade is still running.

Level and status are sequential samples, not an atomic fixture snapshot. Their
typed QueryReply fields preserve a returned value, no response, or collision.
Observation quality controls whether a current power/brightness can be derived:

| Quality | Meaning |
|---|---|
| Current | Known ordinary level plus a status value from the current observation epoch |
| Partial | Some returned evidence; a missing sample or MASK prevents current power/brightness |
| Unavailable | Neither query supplied a usable fixture value |
| Stale | UART, bus or mapping evidence was invalidated after capture |

Actual level zero is known off. Values 1..254 are known ordinary levels. Actual
255 is MASK: the fixture answered but its level is unavailable. `power()` and
`brightness()` return None unless quality is Current. `available()` requires both
queries to have answered and the observation not to be Stale; it can therefore
be true for an answered MASK while brightness remains unknown. Raw partial
evidence stays available for diagnostics and never becomes an invented off/max.

`handle.snapshot().outputs` holds recent typed observations. Missing fixtures stay
in the inventory. Their failure does not prevent another healthy target operation.
Bus power loss invalidates the affected bus. Boot, ordering loss, poison, lag and
session replacement invalidate UART evidence. Actor-owned counters preserve
invalidation even if a watch value changes from Late back to Ready before the
owner polls. Readback from an older epoch is stale; it cannot initialize a newly
renamed target. Metadata activation retains only exact surviving target tokens.

The service operates independently of broker availability. Tests use the real
UART/PTY path, independent emulator physical state and isolated FakeBroker
instances. These checks do not establish physical I4 behavior or DALI certification.
See the [support matrix](support-matrix.md) and [phase ledger](v2-progress.md).
