# Individual output control over MQTT

This guide is for MQTT consumers and bridge operators using the development v2
bridge. Any MQTT v5 consumer can use it. A Store server is not required. Configure
the controller and saved outputs first; discovery and the browser interface are
subsequent work. See [running v2](running-v2.md) and
[the inventory contract](inventory-contract.md).

For controller `Controller`, bus `Bus1` and output `Light1`, the target is
`Bus1.Outputs.Light1`. Read its current target-generation token from Inventory.
Every write requires that exact token in the MQTT v5 `target-generation` user
property. A rename retires the old target/token. Do not copy a controller token
onto a fixture request.

| Topic | JSON payload | Effect |
|---|---|---|
| `DALI/Desired/Controller/Bus1.Outputs.Light1/PowerState` | `true` or `false` | Recall configured maximum or turn off |
| `DALI/Desired/Controller/Bus1.Outputs.Light1/Brightness` | A number from `0` to `1` | Set DALI control-code position, normalized by 254 |
| `DALI/Desired/Controller/Bus1.Outputs.Light1` | `{"PowerState":true,"Brightness":0.5}` | Validate and apply the complete intent |

Brightness is the DALI control level, not measured luminous flux. Zero means off.
Power true alone recalls maximum; it does not restore an application's remembered
brightness. Nulls, unknown properties, wrong JSON types, out-of-range levels and
contradictory pairs are rejected before any output command is sent. Groups and
color properties are not enabled in this slice.

Use `DALI/Command/Controller` with
`{"target":"Bus1.Outputs.Light1","command":"refresh"}` to request readback.
Supply a v5 response topic for its reply. A refresh may omit target-generation;
an explicitly supplied generation must match. The optional `traceparent` user
property follows the original request through the service and physical readback.
Headerless requests and unsolicited bus invalidations do not inherit old causes.

## Understanding observed State

`DALI/State/Controller/Bus1.Outputs.Light1` contains observed values:

```json
{"Available":true,"Quality":"current","PowerState":true,"Brightness":0.5}
```

Writes query actual level and status before reporting success. A fixture can clamp
the requested brightness or still be fading; State reports its sampled result.
The HAT's ordinary `N` response alone never establishes the requested brightness.
Level and status are sequential samples, not an atomic physical snapshot.

| Quality | Current control values available? |
|---|---|
| `current` | Yes, when Available is true |
| `partial` | No; some evidence is missing or the fixture answered MASK |
| `unavailable` | No usable query evidence |
| `stale` | Earlier evidence was invalidated by bus, UART or mapping changes |
| `unobserved` | No sample for this saved mapping yet |

Gate usable State by `Available`, and by the controller's BridgeKit Active/Status.
BridgeKit merges properties: after an outage, old PowerState/Brightness may remain
in retained State as last-known values with Available=false. They are not a new
off reading. A missing fixture stays in Inventory and does not prevent controlling
another healthy fixture. Available is not a certification that a fixture is
fault-free. Saved inventory alone never proves physical presence.

Target reads and writes have a 2 s service budget. Background polling runs every
30 s, with a 15 s aggregate ceiling and at most 2 s per fixture. Large/incomplete
passes report an error while keeping completed samples, and the next pass resumes
fairly from a persistent cursor. Disabled buses are not queried. Without a ready
UART, background reads reject before taking physical admission; metadata remains
accessible. Busy or expired operations can fail. A canceled frontend cannot undo
a command already sent: inspect observed state before retrying an uncertain write.

Current development limitation: a Desired request consumed while a fixture's bus
or UART is unavailable is rejected. HAT/bus recovery alone does not retry it,
including retained Desired consumed before startup readiness. A later broker
reconnect may replay retained property intent. Target-scoped deferred state is
the next integration slice; automatic hardware-outage convergence is not yet
implemented. Metadata and healthy fixtures remain accessible during that outage.

These paths are tested with isolated brokers and independent PTY fixtures. Physical
I4 firmware and fixture validation remain pending; see the
[support matrix](support-matrix.md).
