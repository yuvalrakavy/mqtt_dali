# The wait registry

Every place mqtt_dali waits on something else — a lock, a channel, the broker, a dependency's
`async fn` — carries a tag naming a row here:

```text
let event = self.rx.recv().await; // WAIT: mqtt-pump-queue
```

The row says, once, why that wait cannot hang the bridge: **`acyclic`** (nothing it waits on can
wait back on any of its waiters) or **`bounded`** (it ends within a stated bound, and the row says
what happens on expiry). The rule and the tag format are Store's no-hang spec, §13.3 and §14
(`Store/docs/superpowers/specs/2026-09-30-no-hang-wait-graph-design.md`); the check is the
`wait-lint` crate, run by `tests/wait_registry.rs`.

It does not prove the order locks are taken in, nor see a wait reached through a call to this
crate's own `async fn`; the arguments are checked by reading, and the waiters block below makes
every new waiting function a reviewed diff. Regenerate it after a change, from a checkout of
tracing-init:

```text
cargo run --manifest-path wait-lint/Cargo.toml -- --root <mqtt_dali> --src src --registry docs/wait-registry.md --write
```

## The rows

| Key | Kind | Waits on | Argument |
|---|---|---|---|
| `mqtt-request` | acyclic | rumqttc's request channel (`publish`, `subscribe`) | The channel drains only while the event loop is polled, and the pump (`Pump::start`) polls it in a task of its own that waits on nothing but the network. So a full channel is back-pressure on the session, never a cycle: with the broker stalled the session waits until it recovers, the pump reading its acks meanwhile (`a_command_burst_against_a_stalled_broker_completes_once_it_recovers`; it failed on the single-task session, where the poller awaited its own publishes). |
| `mqtt-pump-queue` | acyclic | the pump's forward queue | Unbounded (no-hang §14.6): the pump never waits on the session, so the session's wait ends with the next message, or with `Ended` when the connection fails — the session returns and `run` reconnects. |
| `mqtt-poll` | acyclic | the broker, over the network | The pump waits on nothing in this process. A dead connection ends the poll with an error within the 6 s keep-alive. |
| `mqtt-backlog-lock` | acyclic | the forward queue's high-water flag | Held to read or set one timestamp; nothing waits under it. |

## Settings

```wait-lint
# rumqttc's client calls, which wait on its request channel.
wait-methods = publish, publish_with_properties, subscribe, unsubscribe, disconnect
# Dependency calls whose .await waits on nothing in this process.
not-waits = sleep, sleep_until, yield_now
# This code's own async methods, awaited on a receiver other than `self` (checked: the DALI
# manager's, and the session's own).
local-methods = set_light_brightness_async, set_group_brightness_async, run_session
```

## Waiters (generated)

```wait-lint-waiters
mqtt-backlog-lock src/mqtt.rs Backlog::popped
mqtt-backlog-lock src/mqtt.rs Backlog::pushed
mqtt-poll src/mqtt.rs Pump::start
mqtt-pump-queue src/mqtt.rs Incoming::recv
mqtt-pump-queue src/mqtt.rs MqttDali::run_session
mqtt-request src/mqtt.rs MqttDali::run_session
mqtt-request src/mqtt.rs publish_with_trace
```
