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
| `dali-config-save` | bounded | the configuration file's write, on the blocking pool (`ConfigSaver::save_config`) | At most `SAVE_BOUND` (2 s): everything the bridge does next waits on the session, so the session never waits on the filesystem unbounded (fleet class F1). On expiry, the command is reported on the status topic as completed but not saved (WARN `config_save_failed`; the change holds in memory), the session goes on, and the write is left running on its blocking thread. While it runs, a later save is queued at once (only the latest; DEBUG, the status topic says so) and written by that same thread when the stuck write ends, so a filesystem that stopped answering holds one thread, not one per command, and no change is left unwritten. Each write goes to a temporary file renamed over the old one, so a write cut short (this bound, or the process's exit) never leaves the file truncated. The process's exit does not wait for it (`dali-runtime-shutdown`). Shown failing first, on FIFOs nobody reads: `a_config_save_that_stalls_does_not_hold_the_session` (synchronous in the session, the write held every later command), `a_change_queued_behind_a_stuck_write_is_saved_once_it_ends` (a save refused behind a stuck write was never written), and `a_config_write_cut_short_leaves_the_last_good_config` (the file-size limit cutting the write: in place, it left the file truncated). |
| `dali-config-writer` | acyclic | the writer's state, `busy` and the latest queued configuration (`ConfigSaver::save_config`, `write_config`, `Release::drop`) | Held only to read or set `busy` and to take or put the queued bytes: no I/O, no logging, no other wait under it. The session and the one writing thread take it, neither while waiting on the other. |
| `dali-stop` | acyclic | the stop flag (`stopped`), set by `main` on SIGTERM or SIGINT | `main` sets the flag without waiting on the session (`send_replace`); a dropped sender ends the wait too, as a stop. Nothing the session does can delay the flag. |
| `dali-session-thread` | acyclic | the session, run to its end on a thread of its own (`spawn`'s `block_on`) | The thread exists only to run the session, and `block_on` returns when `run` does, on a stop. Nothing waits on this thread unbounded: `main` waits for its result under `dali-shutdown`'s bound, and the session never waits on `main`. The pump runs on the runtime's workers, not on this thread, so a synchronous DALI call here never stops the polling. |
| `dali-supervise` | acyclic | a signal, or the session ending by itself (`shutdown::supervise`) | The signal comes from the OS; the session's result comes from its thread, which never waits on `main`. |
| `dali-shutdown` | bounded | the session's end after a stop (`shutdown::supervise`) | At most `SHUTDOWN_BOUND` (5 s). On expiry, a WARN `shutdown_timeout` and `run` returns: the logging guard is dropped (and flushes), `main` gives the runtime's blocking threads at most `RUNTIME_SHUTDOWN_BOUND` (`dali-runtime-shutdown`), the process exit ends the session's thread, and the broker's will publishes Active=false (`a_session_that_never_answers_its_stop_is_left_behind_in_bounded_time`; end to end, `sigterm_publishes_inactive_and_exits_cleanly`). A stop takes at most 5 s, the guard's flush (bounded by tracing-init at about 4 s), and 1 s: about 10 s in all. |
| `dali-signal` | acyclic | SIGTERM or SIGINT from the OS (`shutdown::StopSignals`, installed first in `run`, before logging, the configuration and the hardware), or nothing, when no handler can be installed | The OS never waits on the bridge, and every wait on it is raced — with the logging start and start-up (`dali-startup`) and with the session's end (`supervise`) — so even a wait that never ends holds nothing. A handler that cannot be installed leaves its signal's default action, logged as WARN `signal_handler_unavailable` (`a_signal_without_a_handler_is_logged_as_signal_handler_unavailable`). |
| `dali-startup` | acyclic | the logging start (`init_logging`: tracing-init's synchronous `init`), then start-up's synchronous steps (`start_up`: the configuration's read, the HAT's version, an interactive configuration or `--setup`), each on the blocking pool, raced with a stop signal in `run` | Nothing either waits on waits back on `run`. They may wait long — tracing-init gives a destination that will not start 5 s and reads its logging config with no bound of its own; a filesystem that stopped answering, the HAT (up to 11 s), the operator — so a stop ends this wait at once: `run` returns (said on stderr by a thread of its own before logging is up, logged after), and a step still stuck is left behind by the runtime's bounded shutdown (`dali-runtime-shutdown`). Shown failing first: the logging start held deterministically — in debug builds `MQTT_DALI_TEST_LOGGING_GATE` names a file the start reads before tracing-init's init, inert unless set and absent from a release build; the test names a FIFO it holds — `sigterm_while_the_logging_start_is_held_exits_cleanly` (unraced, the bridge never exits; with the log file a FIFO nobody reads, the base exited 4.5 s after SIGTERM, when tracing-init gave the destination up), and `sigterm_during_start_up_takes_the_bounded_path`, the configuration a FIFO nobody writes (with the handler installed only after start-up, SIGTERM's default action killed the bridge, nothing logged). |
| `dali-run` | acyclic | `run`, driven to its end on the process's main thread (`main`'s `block_on`) | The main thread exists only to drive `run`, and nothing in the bridge waits on it. `run` ends on a stop: at once during start-up (`dali-startup`), else within `dali-shutdown`'s bound; or when the session ends by itself. |
| `dali-runtime-shutdown` | bounded | the runtime's blocking threads, once `run` has returned (`main`'s `shutdown_timeout`) | At most `RUNTIME_SHUTDOWN_BOUND` (1 s); on expiry the threads are left behind and the process exits. Dropping a runtime instead waits for each without limit, and each can be stuck in the kernel: a configuration write (`dali-config-save`), the logging start or a start-up step (`dali-startup`), rumqttc's name lookup for a broker given by name (the units name theirs, e.g. `control-bz`). Shown failing first: `sigterm_while_a_config_write_is_stuck_exits_in_bounded_time` (the file a FIFO nobody reads; with the runtime dropped, the process never exited). |
| `dali-uart-idle` | bounded | the DALI HAT's UART, until the bus is quiet (`hat_line::wait_for_idle`) | Each read waits at most the quiet time (termios `VTIME`, which rppal rounds down to tenths of a second, so the 10 ms before a bus command is a poll of the input queue), and the loop gives up once the bus has kept sending for `IDLE_DEADLINE` (1 s): it returns within 1 s and one read. On expiry, `HatError::NeverIdle`: the DALI command fails and its error goes to the status topic; a read error is `HatError::Read`, never a panic. Shown on fake byte sources, failing first (`a_bus_that_never_goes_quiet_ends_the_idle_wait_with_an_error`, `a_read_error_is_an_error_not_a_panic`). |
| `dali-uart-line` | bounded | the DALI HAT's UART, for one reply line (`hat_line::read_line`) | Each byte waits at most `byte_timeout`; the line ends at its newline, or fails at `max_len` bytes, past `total`, or when it stops before its newline: it returns within `total` and one byte's wait — 1.1 s for a bus command's reply (`REPLY`), 11 s for the version query at start-up (`VERSION`, before the session exists). On expiry, `LineTooLong`, `LineDeadline` or `Truncated`: the command fails with that error (at start-up, the bridge does not start). Shown on fake byte sources, failing first (`a_reply_that_never_ends_is_cut_at_its_length_limit`, `a_reply_that_trickles_without_ending_is_cut_at_its_deadline`, `a_reply_cut_short_is_an_error`). The protocol's loops over these exchanges are bounded in total as well, not only per exchange (`dali_manager::BUS_LIMITS`): a broadcast the bus keeps answering with a collision fails after 300 sends or 10 s, and programming a short address fails once its WITHDRAW has been answered 10 times or for 5 s, its collision retries included — each within its bound and one exchange (`Collisions`, `WithdrawAnswered`; the command's error goes to the status topic). Shown on fake buses, failing first (`a_broadcast_that_keeps_colliding_fails_at_its_deadline`, `a_withdraw_that_keeps_being_answered_fails_after_its_retry_cap`, `a_withdraw_that_keeps_being_answered_fails_at_its_deadline`; through the session, `find_lights_on_a_bus_that_answers_withdraw_fails_and_the_session_goes_on`). |

## Settings

```wait-lint
# rumqttc's client calls, which wait on its request channel, and its event loop's poll.
wait-methods = publish, publish_with_properties, subscribe, unsubscribe, disconnect, poll
# The HAT protocol's one read from the UART (`hat_line::ByteSource`): a synchronous wait on the
# device. tokio's `Runtime::shutdown_timeout`: a synchronous wait for the runtime's threads.
blocking-methods = read_byte, shutdown_timeout
# Dependency calls whose .await waits on nothing in this process.
not-waits = sleep, sleep_until, yield_now
# This code's own async methods, awaited on a receiver other than `self` (checked: the DALI
# manager's, the session's own, and the config saver's, whose one wait is tagged inside it).
local-methods = set_light_brightness_async, set_group_brightness_async, run_session, save_config
```

## Waiters (generated)

```wait-lint-waiters
dali-config-save src/mqtt.rs ConfigSaver::save_config
dali-config-writer src/mqtt.rs <Release as Drop>::drop
dali-config-writer src/mqtt.rs ConfigSaver::save_config
dali-config-writer src/mqtt.rs write_config
dali-run src/main.rs main
dali-runtime-shutdown src/main.rs main
dali-session-thread src/mqtt.rs spawn
dali-shutdown src/shutdown.rs supervise
dali-signal src/main.rs run
dali-signal src/shutdown.rs StopSignals::recv
dali-signal src/shutdown.rs never
dali-signal src/shutdown.rs next
dali-startup src/main.rs run
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
