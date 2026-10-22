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
| `web-bind` | bounded | optional HTTP listener startup | Inside the original five-second service opening ceiling, before proxy readiness. On expiry reject and drop the listener; close the config owner on failure. No listener exists when web is disabled. |
| `web-auth` | acyclic | bounded session/rate mutex | Holds at most 16 sessions and 10 attempt timestamps for synchronous checks/updates only. No await, Reporter call or service operation occurs under the guard; no other lock is taken. |
| `web-accept` | acyclic | Axum/Tokio accepting a TCP client | Owns only the optional listener; no service caller waits for it. Parent cancellation drops accept through the server signal. At most 64 accepted IO wrappers retain permits; excess connections close. Each wrapper's pending polls also wake on forced cancellation. |
| `web-serve` | acyclic | optional HTTP server lifetime / join notification | Service loop selects cancellation independently. An absent/completed server branch stays dormant; an unexpected server end fails the service. Graceful stop signals Axum and SSE, then bounded close collects the server and actual IO ownership. No secret or session lock spans these waits. |
| `web-handler` | bounded | HTTP handler/body extraction | At most five seconds per admitted handler; at most 32 handlers and 8 KiB bodies. On expiry return 408 and release admission. Streaming response bodies use separate stream/connection limits and stop/expiry cancellation. |
| `web-events` | bounded | snapshot updates, revocation, session expiry or stop | Pull-based stream with no per-client producer queue; at most eight subscribers. Recheck session before each snapshot. Wake at the existing idle/absolute deadline (30 min/12 h), revoke without needing a bus update, and on expiry end the stream; also end on parent stop. Passive traffic never renews user activity; lag re-snapshots latest complete data. |
| `web-close` | bounded | server join and all accepted-IO permits returning | Share the existing absolute 1.5-second HAT/disk/HTTP cleanup ceiling. Reserve the final 100 ms for forced IO cancellation, outer-task abort/join and the all-permits destruction barrier. On expiry return incomplete cleanup; aborting the outer task alone is not proof of connection destruction. |
| `v2-commissioning-randomise` | bounded | DALI random-address generation timer | Wait 100 ms for outputs / 200 ms for inputs under the original search deadline. On expiry reject the search without reserving/programming a short address; retain physical ownership through settlement. Temporary initialization mode may expire or be terminated by the next operation. |
| `v2-service-request` | bounded | capacity-one service operation reply | Nonwaiting permit and bounded channel admission; the caller supplies an absolute deadline. On expiry report uncertainty for mutations or reject an input read, and drop the reply receiver. Keep routing frozen if saving began; physical jobs retain service admission until their backend request settles or actual transport teardown completes under its fixed cleanup ceiling. |
| `v2-service-attach` | bounded | inventory actor installing its first runtime Reporter | At most 2 s, independently of MQTT and UART. On expiry fail proxy connect; reconnect attachment is idempotent and cannot reopen uncertain routing. |
| `v2-mqtt-observations` | acyclic | the managed service's snapshot watch | The service never waits for this subscriber or Reporter publication. The helper owns data only and runs on the device worker; clone the latest snapshot before synchronous projection, then await its next change. Link teardown cancels the owned helper; terminal service state or sender loss ends it and marks the link lost. |
| `v2-input-events` | acyclic | capacity-64 native input notice broadcast | The UART/service producer never waits for subscribers. The helper holds data only, validates the latest listener/stamp before reporting on the SDK worker, and belongs to LinkContext teardown. Lag explicitly reports dropped actions; it never reconstructs actions from State. A test-only pause is released by its test and cancelled with that same owned helper. |
| `v2-service-start` | bounded | durable ConfigStore startup or managed cancellation | ConfigStore has a 5 s startup deadline and the select also wakes on parent cancellation. On expiry startup fails; the managed runtime includes any unfinished blocking owner in actual teardown. |
| `v2-service-save` | bounded | serialized ConfigStore edit/recovery, caller cancellation or managed stop | The original request deadline bounds ConfigStore; caller/parent cancellation drops its request future. On expiry or lost outcome freeze service routing until explicit recognized recovery and successful runtime activation. |
| `v2-service-loop` | acyclic | bounded service requests or managed cancellation | The actor owns routing and performs no waits on its callers. Idle can wait indefinitely; managed cancellation independently wakes it, closes admission and begins locally bounded cleanup under the parent's forced-cancellation ceiling. |
| `v2-daemon-run` | acyclic | shared BridgeKit managed process lifetime | Main drives only the shared runtime; stop signals are installed before logging/resources and every startup/cleanup path uses its shared deadlines. Runtime teardown includes the UART/config owners and reports incomplete work rather than claiming a clean exit. |
| `v2-store-open` | bounded | configuration ownership/import/durability worker | Caller supplies an absolute deadline; on expiry return startup failure and cancel admission. The runtime-owned blocking worker retains its lock until it really ends, so managed-service teardown cannot report success while it remains alive. |
| `v2-store-request` | bounded | one serialized filesystem operation result | Nonwaiting single-slot admission; caller supplies an absolute deadline. On expiry report Uncertain, retain the slot and lock, and freeze mutations/routing until the worker ends and explicit bounded recovery confirms a recognized durable document. Canceled queued work is skipped. |
| `v2-store-close` | bounded | configuration worker completion after stop | Caller supplies an absolute deadline; on expiry return false, requiring the managed service/process incomplete-stop path. True is published only after file ownership is dropped, even with retained public clients. |
| `v2-store-worker` | bounded | capacity-one synchronous request channel on the blocking worker | recv_timeout wakes at least every 50 ms; on expiry recheck closing. No async caller waits for the filesystem without its own deadline. Filesystem syscalls themselves may block; the managed runtime includes this worker in its outer shutdown bound. |
| `emulator-runtime-shutdown` | bounded | Tokio worker shutdown | At most 500 ms; on expiry abandon any remaining worker. Detached filesystem and output threads are never joined. |
| `emulator-control` | bounded | actor's capacity-32 local control queue response | Admission is nonwaiting. The caller supplies a total deadline; on expiry return an error saying an applied mutation is not undone. Expired/canceled requests are skipped before execution. Socket controls supply a 1 s bound. |
| `emulator-close` | bounded | emulator session completion | At most 1 s; on expiry return shutdown-deadline error. Stop wakes the select even with unread UART or no input. Final handle drop aborts the worker and releases both PTY descriptors. |
| `emulator-loop` | acyclic | UART readiness, model-independent transport timers, controls or stop | One session owns the model without sharing locks. Controls never wait for output consumers. Output offsets survive select cancellation; frame writes expire after 1 s and fail the session. Partial commands expire after 1 s. Idle may wait indefinitely; close independently wakes stop and bounds completion. |
| `emulator-pty-open` | bounded | detached openpty/configuration worker | At most 2 s; on expiry return an error. Late owned descriptors are dropped; no worker join occurs at shutdown. |
| `emulator-control-bind` | bounded | detached local socket setup worker | At most 2 s; on expiry return an error. A late listener and owned cleanup guard are dropped. Existing paths are never removed. |
| `emulator-control-serve` | acyclic | local listener, at most 16 client tasks or stop | Each client has a separate 3 s outer bound. Session controls do not await client output. Stop drops the JoinSet, aborting clients; it never joins their I/O. Excess connections are closed without creating tasks. |
| `emulator-control-client` | bounded | one bounded JSON request, actor operation and socket output | Outer bound 3 s; inner request read and response write each 1 s. On expiry drop the client. Maximum request 16 KiB, response 1 MiB. A nonreading client holds no session resource. |
| `emulator-startup` | acyclic | bounded startup workers, PTY creation, optional control binding or stop | Signals are installed first and race all startup. Workers are detached and bounded separately (3 s filesystem/logging, 2 s PTY/control bind). Diagnostic output has its own 200 ms bound. No startup worker waits for this caller. |
| `emulator-startup-worker` | bounded | detached logging/scenario worker | At most 3 s; on expiry return startup-deadline error, dropping any late owned result. Never join a blocked filesystem/logging worker during exit. |
| `emulator-supervise` | acyclic | signals, emulator lifecycle or local control server completion | Lifecycle is published without waiting for this receiver. Signals are independent. Shutdown sets stop, aborts endpoint clients and bounds emulator close to 1 s; runtime shutdown is capped at 500 ms. |
| `emulator-output` | bounded | best-effort raw-descriptor startup/diagnostic writer | At most 200 ms; on expiry detach and continue/exit. The writer duplicates the descriptor and bypasses Rust stdio locks, so an unread output pipe cannot hold process exit. |
| `atx-receive-fence` | bounded | same-reader receive drain and its oneshot result | Caller deadline bounds the oneshot; on expiry return Deadline. Reader drain separately stops after 4096 bytes or 100 ms with Busy. Explicit nonblocking Empty ends draining; retry Pending remains pending under the 100 ms bound. Close aborts the reader and drops the sender. |
| `atx-transaction-result` | bounded | capacity-one actor response | One total caller deadline covers fence, write and response, or explicit quiet recovery. On expiry return Deadline and retain unresolved ordering until its terminal, qualified quiet recovery, or accepted Boot. A ready response is rejected if the caller resumes after expiry. |
| `atx-transaction-settle` | bounded | actual transaction admission after the preceding frontend has joined | Caller supplies one absolute cleanup ceiling (the service selects 1 s once). Acquiring the shared permit proves prior actor requests retired while service admission excludes other producers. Terminal/stopping sessions require actual close/join even if the permit is free. On expiry return Deadline and retain service admission through actual bounded teardown; a failed join fails the service. |
| `atx-reset-guard` | acyclic | synchronous reader reset-slot mutex | Only clones, installs or removes an Arc. No await, I/O, stream lock or cancellation lock is acquired while this mutex is held. The split reader may snapshot it but drops it before any other work. |
| `atx-recovery-cancel` | acyclic | synchronous caller cancellation / reader authorization mutex | Caller Drop and actor checks only set/read a bool. Reader takes it under the split-stream lock and holds it through synchronous framing reset; no await or I/O occurs while held, and no other holder acquires the stream lock. No holder waits on an actor or caller. |
| `atx-quiet` | bounded | raw-byte activity, guarded same-reader fence, actor events or cancellation | One original caller deadline bounds the measured interval and all retries/fences. Managed acquisition supplies quiet interval plus setup allowance. Raw input restarts the timer without discarding complete events. On expiry or cancellation recovery remains required. Guarded framing reset checks identity, cancellation, activity and deadline under the split-stream lock at Empty. New activity after reader proof requires another interval; write poison/EOF never become successful recovery. |
| `atx-transaction-close` | bounded | actual actor join after monitor resource release | Public close supplies 1 s; managed close supplies the original absolute owner deadline. Stop closes admission and wakes the active or idle actor, which force-closes the monitor within the same deadline. On expiry return Deadline, retaining the real join in a lifecycle-owned supervisor. Cancellation of a waiter cannot cancel teardown. |
| `atx-owner-join` | acyclic | actual monitor/transaction task JoinHandle | Inner tasks never wait on their join supervisor, and no supervisor references its caller. Reader join owns only the handle and result sender; transaction join additionally retains bounded failure cleanup until it runs or is discarded. Captured cleanup ownership is dropped before completion publication. Callers wait under their original close deadline; a canceled waiter cannot lose the join. |
| `atx-owner-failure-cleanup` | bounded | resource release after actual actor JoinError | Transaction supervisor force-closes its captured monitor under the first shutdown request's original absolute deadline, or 500 ms if the actor failed before any shutdown request. No-op reader cleanup is immediate. Cleanup ownership is dropped before publishing failure; panic cleanup does not depend on any close waiter remaining alive. On expiry the result remains failure, never quiescence. |
| `atx-transaction-stop` | acyclic | stop flag | The flag is written by close/final-handle drop, which never waits for this helper alone. All active-operation uses race their deadline; idle use races dispatch events. |
| `atx-transaction-dispatch` | acyclic | bounded request queue, monitor events or stop | One actor continuously drains events without waiting for consumers. Idle needs no timeout; close sets stop and bounds completion. Requests retain one permit for queued plus active work; admission never waits. |
| `atx-transaction-write` | bounded | correlated write, monitor events, caller cancellation or stop | Total request deadline bounds the select and is checked before each iteration. On expiry return Deadline. Events stay live during writing; boot/error cancels the write. Potentially accepted partial writes require a later boot before admission can reopen. |
| `atx-transaction-response` | bounded | monitor terminal, caller cancellation or stop | Same total request deadline, checked before accepting a complete result. On expiry return Deadline and retain the remaining terminal count until retirement or explicit recovery. Partial-write poison requires a later accepted Boot. No receiver or subscriber is awaited. |
| `atx-reader` | acyclic | nonblocking UART/PTY reads or a partial-line timer | The one reader never waits for a writer or subscriber: it broadcasts without awaiting. Empty input may wait indefinitely, but nothing joins that read unbounded. Close and final-handle drop abort it; partial lines expire within 1 s and recover at a newline. A read error/EOF closes admission and publishes a terminal event. |
| `atx-write-lock` | bounded | serialized access to the UART writer | Total caller-supplied deadline includes lock and write. On expiry before acquiring the lock, return Deadline without poisoning an unwritten command. The holder waits only on nonblocking I/O under its own deadline, never on a receiver or this lock. |
| `atx-write` | bounded | nonblocking UART write, cancellation/EOF/close, and caller deadline | The write is raced with stop and the caller's total deadline. On expiry, I/O failure or cancellation after possible byte acceptance, a drop guard poisons admission and emits WriteIndeterminate. A Pending write that accepted no bytes stays usable. Stop cancels a potentially partial write. The stop helper waits only for the independent reader/closer and is raced against the write deadline. |
| `atx-close` | bounded | writer release and actual reader JoinHandle completion | Public force-close supplies 500 ms; managed force-close uses the original owner deadline. Close stops write admission, cancels active writers, drops the writer and aborts the reader. A lifecycle supervisor publishes completion only after the actual reader joins. On expiry return Deadline without claiming resource release. |
| `atx-serial-open` | bounded | runtime-owned blocking task opening/configuring a serial path | At most 2 s. On expiry a late owned fd is dropped; the managed HAT owner never retries that timeout. Actual runtime teardown includes the blocked worker and cannot report a clean stop while it remains alive. |
| `v2-hat-loop` | acyclic | owned UART helper, independent monitor/status stream or paced retry/poll timer | The parent actor selects managed cancellation independently. No branch waits on the parent; canceled selects retain the stored join handle and session. Complete-write/receive uncertainty schedules bounded measured quiet recovery; known partial-write poison waits for Boot and survives actual close/reopen. |
| `v2-hat-setup` | bounded | version and enabled-bus status sequence on the existing transaction owner | One original 5 s setup deadline spans the sequence. On expiry retain the session's Late/reset state; metadata stays available and no new probe bypasses that state. |
| `v2-physical-operation` | bounded | owned discovery/input/output execution future | Every transaction receives the same original caller deadline; no per-query renewal. MQTT selects 4 s for input reads and the existing 2/6/8 s output budgets; addressed discovery clips hardware to 8 s within its original 10 s MQTT save/reply budget. On expiry reject a read or report mutation uncertainty, retaining admission through the existing fixed settlement/teardown ceiling after the execution joins. Caller cancellation cannot release active physical ownership. |
| `v2-hat-health` | bounded | a local HAT bus-status snapshot query | Uses the remaining original setup deadline. On expiry retain the same transaction owner and poison state. The proactive D snapshot is not a causal acknowledgement or fixture capability claim. |
| `v2-hat-close` | bounded | helper abort/join and actual UART close, alongside configuration cleanup | Both resource owners share one absolute local 1.5 s deadline inside managed cooperative stop. On expiry report incomplete cleanup; parent cancellation/runtime destruction includes remaining work, including a blocked opener. |
| `v2-config-check` | bounded | the check command's filesystem worker, through `recv_timeout` | At most 3 s. The worker's handle is detached before waiting; on expiry, `main` returns a failure without joining a filesystem operation. `stalled_configuration_worker_does_not_hold_process_exit` holds an actual FIFO read in the worker and full unread output pipes while the child exits. The library loader is synchronous; daemon integration must supply its own bounded worker. |
| `v2-config-output` | bounded | best-effort check-command output on a detached writer thread | At most 200 ms; on expiry the command returns without joining the writer. The writer duplicates the raw output descriptor and never locks Rust's global stdio handles. A full pipe can hold that thread until the process exits, but holds neither the caller nor stdio cleanup. `unread_full_output_pipes_cannot_hold_success_error_or_help_exit` exercises success, invalid options and help with a prefilled pipe kept unread; the filesystem deadline test also holds stderr full. |
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
blocking-methods = read_byte, shutdown_timeout, run_managed
# Dependency calls whose .await waits on nothing in this process.
not-waits = sleep, sleep_until, yield_now
# This code's own async methods, awaited on a receiver other than `self` (checked: the DALI
# manager's, the session's own, and the config saver's, whose one wait is tagged inside it).
local-methods = set_light_brightness_async, set_group_brightness_async, run_session, save_config, fence, control, edit, recover, change, attach, submit, fresh_boot_boundary, initialize, force_until, close_until, fence_until, quiet_fence_using, resynchronize, enqueue, command, execute_until, receive_fence_until, settle_until, read_output, apply_output, read_colour, apply_colour, apply_colour_with_output, input_boundary, read_input, wait_for_receiver_close, discover, assign, assign_space, commission, search, program, groups, read_groups, change_groups, finished
```

## Waiters (generated)

```wait-lint-waiters
atx-close src/atx/monitor.rs Monitor::force_until
atx-owner-failure-cleanup src/atx/joined.rs start_with_failure_cleanup
atx-owner-join src/atx/joined.rs start_with_failure_cleanup
atx-quiet src/atx/monitor/recovery.rs Monitor::quiet_fence_using
atx-quiet src/atx/transactions/engine.rs Actor::recover
atx-reader src/atx/monitor.rs Monitor::start
atx-reader src/atx/monitor.rs line_deadline
atx-reader src/atx/monitor.rs read_loop
atx-receive-fence src/atx/monitor.rs Monitor::fence_until
atx-receive-fence src/atx/monitor.rs read_available
atx-receive-fence src/atx/monitor.rs read_loop
atx-recovery-cancel src/atx/monitor/recovery.rs RecoveryCancellation::cancel
atx-recovery-cancel src/atx/monitor/recovery.rs RecoveryCancellation::is_canceled
atx-recovery-cancel src/atx/monitor/recovery.rs Reset::authorize
atx-reset-guard src/atx/monitor.rs <ReadMode as AsyncRead>::poll_read
atx-reset-guard src/atx/monitor.rs read_loop
atx-reset-guard src/atx/monitor.rs receive_fence
atx-serial-open src/atx/serial.rs Serial::open_using
atx-transaction-close src/atx/transactions/engine.rs Transactions::close_until
atx-transaction-dispatch src/atx/transactions/engine.rs Actor::drive
atx-transaction-response src/atx/transactions/engine.rs Actor::await_response
atx-transaction-result src/atx/transactions/engine.rs Transactions::enqueue_until
atx-transaction-settle src/atx/transactions/engine.rs Transactions::settle_until
atx-transaction-stop src/atx/transactions/engine.rs stop_requested
atx-transaction-write src/atx/transactions/engine.rs Actor::command
atx-transaction-write src/atx/transactions/engine.rs Actor::write
atx-write src/atx/monitor.rs Monitor::write_correlated_until
atx-write src/atx/monitor.rs stopped
atx-write-lock src/atx/monitor.rs Monitor::write_correlated_until
dali-config-save src/mqtt.rs ConfigSaver::save_config
dali-config-writer src/mqtt.rs <Release as Drop>::drop
dali-config-writer src/mqtt.rs ConfigSaver::save_config
dali-config-writer src/mqtt.rs write_config
dali-run src/legacy_main.rs main
dali-runtime-shutdown src/legacy_main.rs main
dali-session-thread src/mqtt.rs spawn
dali-shutdown src/shutdown.rs supervise
dali-signal src/legacy_main.rs run
dali-signal src/shutdown.rs StopSignals::recv
dali-signal src/shutdown.rs never
dali-signal src/shutdown.rs next
dali-startup src/legacy_main.rs run
dali-stop src/mqtt.rs stopped
dali-supervise src/shutdown.rs supervise
dali-uart-idle src/hat_line.rs wait_for_idle
dali-uart-line src/hat_line.rs read_line
emulator-close src/bin/dali_hat_emulator.rs run
emulator-close src/emulator/session.rs Emulator::close
emulator-control src/emulator/session.rs Emulator::control
emulator-control-bind src/emulator/control.rs Endpoint::bind
emulator-control-client src/emulator/control.rs Endpoint::run
emulator-control-client src/emulator/control.rs handle
emulator-control-serve src/bin/dali_hat_emulator.rs run
emulator-control-serve src/emulator/control.rs Endpoint::run
emulator-loop src/emulator/session.rs due
emulator-loop src/emulator/session.rs run
emulator-output src/bin/dali_hat_emulator.rs output
emulator-pty-open src/bin/dali_hat_emulator.rs run
emulator-pty-open src/emulator/pty.rs spawn
emulator-runtime-shutdown src/bin/dali_hat_emulator.rs main
emulator-startup src/bin/dali_hat_emulator.rs run
emulator-startup-worker src/bin/dali_hat_emulator.rs worker
emulator-supervise src/bin/dali_hat_emulator.rs main
emulator-supervise src/bin/dali_hat_emulator.rs run
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
v2-commissioning-randomise src/service/commissioning.rs Bus::search
v2-config-check src/config_check.rs run
v2-config-output src/config_check.rs output
v2-daemon-run src/daemon.rs run
v2-hat-close src/service/hat.rs Hat::close
v2-hat-close src/service/owner.rs <DaliService as ProcessService>::run
v2-hat-health src/service/hat.rs query_health
v2-hat-loop src/service/hat.rs Hat::changed
v2-hat-loop src/service/hat.rs dormant
v2-hat-loop src/service/physical.rs Work::wait_for_receiver_close
v2-hat-setup src/service/hat.rs Hat::advance
v2-input-events src/service/input.rs events
v2-mqtt-observations src/service/proxy/state.rs Watcher::run
v2-physical-operation src/service/hat.rs Hat::advance_physical
v2-service-attach src/service.rs ServiceHandle::attach
v2-service-loop src/service/owner.rs <DaliService as ProcessService>::run
v2-service-request src/service.rs ServiceHandle::submit
v2-service-request src/service/commissioning.rs ServiceHandle::assign_space
v2-service-request src/service/discovery.rs ServiceHandle::discover
v2-service-request src/service/groups.rs ServiceHandle::groups
v2-service-request src/service/input/readback.rs ServiceHandle::read_input
v2-service-request src/service/output.rs ServiceHandle::output
v2-service-save src/service/owner.rs Owner::change
v2-service-save src/service/owner.rs Owner::commission
v2-service-save src/service/owner.rs Owner::discover
v2-service-save src/service/owner/group_changes.rs Owner::groups
v2-service-start src/service/owner.rs <DaliService as ProcessService>::run
v2-store-close src/config_store.rs ConfigStore::close
v2-store-close src/service/owner.rs <DaliService as ProcessService>::run
v2-store-open src/config_store.rs ConfigStore::open_with
v2-store-request src/config_store.rs ConfigStore::submit
v2-store-worker src/config_store/worker.rs run
web-accept src/web/transport.rs <Listener as Listener>::accept
web-auth src/web/handlers.rs access
web-auth src/web/handlers.rs activity
web-auth src/web/handlers.rs login
web-auth src/web/handlers.rs logout
web-bind src/web/server.rs Server::start
web-close src/service/owner.rs <DaliService as ProcessService>::run
web-close src/web/server.rs Server::close
web-events src/web/handlers.rs events
web-handler src/web/handlers.rs limits
web-handler src/web/handlers.rs protect
web-serve src/web/server.rs Server::finished
web-serve src/web/server.rs Server::start
web-serve src/web/server.rs changed
```
