# The wait registry

Every place mqtt_dali waits on something else — a lock, a channel, the broker, a dependency's
`async fn` — carries a tag naming a row here, on its own line directly above the statement (a
trailing tag after a `{` is moved into the block by rustfmt, away from its wait):

```text
// WAIT: mqtt-pump-queue
let event = self.rx.recv().await;
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
| `mqtt-request` | acyclic | rumqttc's request channel (`publish`, `publish_with_properties`, `subscribe`) | The channel drains only while the event loop is polled, and the pump (`Pump::start`) polls it in a task of its own that waits on nothing but the network. So a full channel is back-pressure on the session, never a cycle: with the broker stalled the session waits until it recovers, the pump reading its acks meanwhile (`a_command_burst_against_a_stalled_broker_completes_once_it_recovers`; it failed on the single-task session, where the poller awaited its own publishes). A session held here does not see a stop until it is released; `main`'s wait for it is bounded all the same (`dali-shutdown`), and the session's goodbye never waits here (`try_publish`, `try_disconnect`). |
| `mqtt-pump-queue` | acyclic | the pump's forward queue (`Incoming::recv`), raced in `run_session` against the stop flag and against the timer that ends a broker outage once the connection has held (`STABLE_AFTER`) | Unbounded (no-hang §14.6): the pump never waits on the session, so the session's wait ends with the next message, with `Ended` when the connection fails (the session returns and `run` reconnects), or with a stop (`dali-stop`), which wins over queued commands. The timer waits on nothing in this process. Inside the goodbye the same queue is read under `mqtt-goodbye`'s timeout. |
| `mqtt-poll` | acyclic | the broker, over the network | The pump waits on nothing in this process, and nothing waits on the pump: the session waits on its queue, which an ended pump closes, and a stop never waits for it (the goodbye is bounded, and dropping the pump aborts its task). No bound is claimed for the poll itself. A broker that stops answering is noticed by the keep-alive: a ping every 6 s, and an error at the next tick if it went unanswered, so within about 12 s. But rumqttc awaits its network writes and flushes inside the poll, so a broker that stops reading holds a write until the kernel abandons the connection (TCP retransmission, many minutes), or indefinitely while the broker's kernel keeps advertising a zero window. Meanwhile the session's publishes back up on the request channel (`mqtt-request`). |
| `mqtt-backlog-lock` | acyclic | the forward queue's high-water flag | Held to read, set or take one timestamp; nothing waits under it. The WARN and INFO it decides are logged after the guard is dropped, since a log write can block (tracing-init's console and file writers are synchronous): `the_backlog_logs_after_releasing_its_lock`, failing first. |
| `mqtt-goodbye` | bounded | the pump writing the session's DISCONNECT (`MqttDali::goodbye`) | At most `GOODBYE_BOUND` (2 s). The goodbye's retained Active=false and its DISCONNECT are queued without waiting (a full request channel skips them), then the session waits, under this timeout, for the pump to report the DISCONNECT written. On expiry the session returns anyway: its pump is dropped, the connection closes, and the broker's will publishes the same retained Active=false (`a_stop_against_a_stalled_broker_is_still_bounded`: the broker's receive window full, so rumqttc never takes the DISCONNECT). |
| `mqtt-reconnect-delay` | bounded | the delay after a failed session, or the stop flag (`MqttDali::run`) | Ends within `RECONNECT_DELAY` (10 s), or at once on a stop. On expiry the next session starts; on a stop `run` returns. |
| `dali-stop` | acyclic | the stop flag (`stopped`), set by `main` on SIGTERM or SIGINT | `main` sets the flag without waiting on the session (`send_replace`); a dropped sender ends the wait too, as a stop. Nothing the session does can delay the flag. |
| `dali-session-thread` | acyclic | the session, run to its end on a thread of its own (`spawn`'s `block_on`) | The thread exists only to run the session, and `block_on` returns when `run` does, on a stop. Nothing waits on this thread unbounded: `main` waits for its result under `dali-shutdown`'s bound, and the session never waits on `main`. The pump runs on the runtime's workers, not on this thread, so a synchronous DALI call here never stops the polling. |
| `dali-supervise` | acyclic | a signal, or the session ending by itself (`shutdown::supervise`) | The signal comes from the OS; the session's result comes from its thread, which never waits on `main`. |
| `dali-shutdown` | bounded | the session's end after a stop (`shutdown::supervise`) | At most `SHUTDOWN_BOUND` (5 s). On expiry, a WARN `shutdown_timeout` and `main` returns: the logging guard is dropped (and flushes), the process exit ends the session's thread, and the broker's will publishes Active=false (`a_session_that_never_answers_its_stop_is_left_behind_in_bounded_time`; end to end, `sigterm_publishes_inactive_and_exits_cleanly`). |
| `dali-signal` | acyclic | SIGTERM or SIGINT from the OS (`shutdown::stop_signal`), or nothing, when no handler can be installed | The OS never waits on the bridge, and `supervise` races this wait with the session's end, so even a wait that never ends holds nothing. |
| `dali-uart-idle` | bounded | the DALI HAT's UART, until the bus is quiet (`hat_line::wait_for_idle`) | Each read waits at most the quiet time (termios `VTIME`, which rppal rounds down to tenths of a second, so the 10 ms before a bus command is a poll of the input queue), and the loop gives up once the bus has kept sending for `IDLE_DEADLINE` (1 s): it returns within 1 s and one read. On expiry, `HatError::NeverIdle`: the DALI command fails and its error goes to the status topic; a read error is `HatError::Read`, never a panic. Shown on fake byte sources, failing first (`a_bus_that_never_goes_quiet_ends_the_idle_wait_with_an_error`, `a_read_error_is_an_error_not_a_panic`). |
| `dali-uart-line` | bounded | the DALI HAT's UART, for one reply line (`hat_line::read_line`) | Each byte waits at most `byte_timeout`; the line ends at its newline, or fails at `max_len` bytes, past `total`, or when it stops before its newline: it returns within `total` and one byte's wait — 1.1 s for a bus command's reply (`REPLY`), 11 s for the version query at start-up (`VERSION`, before the session exists). On expiry, `LineTooLong`, `LineDeadline` or `Truncated`: the command fails with that error (at start-up, the bridge does not start). Shown on fake byte sources, failing first (`a_reply_that_never_ends_is_cut_at_its_length_limit`, `a_reply_that_trickles_without_ending_is_cut_at_its_deadline`, `a_reply_cut_short_is_an_error`). The protocol's loops over these exchanges are bounded in total as well, not only per exchange (`dali_manager::BUS_LIMITS`): a broadcast the bus keeps answering with a collision fails after 300 sends or 10 s, and programming a short address fails once its WITHDRAW has been answered 10 times or for 5 s, its collision retries included — each within its bound and one exchange (`Collisions`, `WithdrawAnswered`; the command's error goes to the status topic). Shown on fake buses, failing first (`a_broadcast_that_keeps_colliding_fails_at_its_deadline`, `a_withdraw_that_keeps_being_answered_fails_after_its_retry_cap`, `a_withdraw_that_keeps_being_answered_fails_at_its_deadline`; through the session, `find_lights_on_a_bus_that_answers_withdraw_fails_and_the_session_goes_on`). |

## Settings

```wait-lint
# rumqttc's client calls, which wait on its request channel, and its event loop's poll.
wait-methods = publish, publish_with_properties, subscribe, unsubscribe, disconnect, poll
# The HAT protocol's one read from the UART (`hat_line::ByteSource`): a synchronous wait on the
# device.
blocking-methods = read_byte
# Dependency calls whose .await waits on nothing in this process.
not-waits = sleep, sleep_until, yield_now
# This code's own async methods, awaited on a receiver other than `self` (checked: the DALI
# manager's, and the session's own).
local-methods = set_light_brightness_async, set_group_brightness_async, run_session
```

## Waiters (generated)

```wait-lint-waiters
dali-session-thread src/mqtt.rs spawn
dali-shutdown src/shutdown.rs supervise
dali-signal src/main.rs main
dali-signal src/shutdown.rs never
dali-signal src/shutdown.rs stop_signal
dali-stop src/mqtt.rs stopped
dali-supervise src/shutdown.rs supervise
dali-uart-idle src/hat_line.rs wait_for_idle
dali-uart-line src/hat_line.rs read_line
mqtt-backlog-lock src/mqtt.rs Backlog::popped
mqtt-backlog-lock src/mqtt.rs Backlog::pushed
mqtt-backlog-lock src/mqtt.rs Backlog::session_ended
mqtt-goodbye src/mqtt.rs MqttDali::goodbye
mqtt-poll src/mqtt.rs Pump::start
mqtt-pump-queue src/mqtt.rs Incoming::recv
mqtt-pump-queue src/mqtt.rs MqttDali::run_session
mqtt-reconnect-delay src/mqtt.rs MqttDali::run
mqtt-request src/mqtt.rs MqttDali::run_session
mqtt-request src/mqtt.rs publish_with_trace
```
