# TCP HAT connector

## Intent and shape

The user wants a standalone emulator with its own web UI, emulating the ATX HAT
over TCP, alongside the bridge and its existing configuration UI. This slice
implements the bridge connector. The parallel emulator session owns its TCP
listener, web UI and scenario/model changes; see the emulator UI handoff.

Tier T2: a new CLI/public builder transport choice, without persisted config or
MQTT contract changes. The user authorized continued implementation. Design
budget: two grounded self-review passes, one fresh design gate, targeted tests
then one native acceptance run and bounded source reviews. Gate token metrics
are unavailable on this host.

Chosen: one small concrete UART/TCP stream enum under the existing HAT owner.
Transactions, monitor, framing, all capability executors and recovery remain
one path. Rejected: a UART-to-TCP helper process (extra deployment/lifecycle) or
a second network HAT driver (duplicates correlation/recovery). Existing IO is
generic in the ATX layer but concrete Serial across service executors; enum
delegation changes those service types mechanically without per-capability
generics or new owner tasks.

## Grounded premises

- `atx::stream::DrainRead` requires actual nonblocking emptiness, not generic
  AsyncRead Pending. `atx::serial::Serial` already uses a direct syscall for it.
- `service::hat::Hat` owns open/setup/recovery/retry/close; its settings currently
  store one path. Executor helpers use Transactions<Serial>.
- `daemon` uses the shared SDK CLI grammar and validates bridge options before
  startup. UART defaults to /dev/serial0; `--uart none` disables it.
- Native parsers/transactions already handle byte splitting, terminal profiles,
  idle L frames and receive boundaries. They must not gain TCP-only logic.
- `emulator::session::Emulator::start` already accepts DrainRead + AsyncWrite;
  a TcpStream DrainRead implementation can be shared as IO infrastructure without
  sharing the emulator's independent protocol interpretation.

Confirmed by reading those modules and enumerating Transactions<Serial> uses.
Do not treat existing emulator agreement as physical protocol certification.

## Public boundary

Add `--hat-tcp IP:PORT`, numeric IPv4 or bracketed IPv6, with a nonzero port.
Reject hostnames, malformed/empty values and any explicit `--uart` together with
`--hat-tcp` (including uart=none). Select TCP without touching /dev/serial0.
Absent TCP, preserve UART default/none behavior. The option is runtime-only,
not saved into inventory. Existing repeat and quiet options apply to either
transport and keep their names/defaults for compatibility.

Add `DaliService::with_hat_tcp(SocketAddr, RepeatPolicy)` alongside with_uart.
As with with_uart, builders configure the one endpoint and perform no IO; the
last builder selects the endpoint. CLI is stricter and rejects ambiguity.
Reject port0 before opening. `with_uart_quiet_period` remains source-compatible
and applies to whichever HAT endpoint has been enabled.

TCP is the raw existing ATX byte stream. No JSON, packet framing, Telnet or
additional handshake. No DNS worker: numeric endpoints avoid introducing a
blocking hostname-resolution lifetime. Plain TCP is for the local/trusted
emulator connection; web login does not authenticate the HAT transport.

## IO and ownership

Introduce public `atx::connection::{Endpoint, Connection}`. Endpoint is cloned
owned startup configuration (UART path or TCP SocketAddr). Connection delegates
AsyncRead, AsyncWrite (including flush/shutdown), and DrainRead to its one owned
Serial or TcpStream. Serial API and tests remain intact; From<Serial> supports
existing service-level PTY setup. No extra queues, background IO or driver tasks.

Implement DrainRead for Tokio TcpStream in atx::stream. Use nonblocking recv
on its owned fd with MSG_DONTWAIT; actual EAGAIN/EWOULDBLOCK means Empty. Zero
bytes means EOF (Data with no advance, matching existing monitor handling),
EINTR yields Pending with wake, other errors propagate. A zero-capacity buffer
must return Data without claiming Empty. Do not use try_read's readiness cache
as proof of actual emptiness. Ordinary reads/writes remain Tokio's TCP methods.

Endpoint::open bounds TCP connect to two seconds with an actual async timer;
enable TCP_NODELAY. Cancellation drops a pending connect or owned connection.
UART keeps its current two-second blocking-worker behavior and test hooks.
Use the existing Hat open/retry/close/quiet initialization ownership for both.
TCP refusal/async-connect timeout is ordinary unavailable/retry, not a fatal CLI
failure or terminal UART-open timeout. Only UART's potentially still-running
blocking opener keeps its existing terminal timeout behavior.
No changes to rare bus-off behavior, persistent journals, or command retry.
No replay of a command whose send failed during disconnect.

The emulator listener starts each accepted connection with a fresh HAT session
and B000004 newline, retaining physical fixture state. It owns one active client.
Connector also accepts a normal peer that waits for V/D queries; ordinary startup
does not require a Boot. A successful NEW TCP connection is a new parser/output
lifetime under this endpoint contract. Clear inherited partial-write/recovery
flags when adopting it, then use the ordinary quiet/setup path. Do not depend on
the initial Boot arriving after fresh_boot_boundary, which deliberately excludes
buffered Boot records. A partial write on the current connection still poisons
that connection; closing/reconnecting is the clean boundary. UART's poison/Boot
policy stays unchanged; do not generalize the TCP contract to physical UART.

## Tests, controls and staged execution

1. Fail-first CLI tests for numeric endpoints/port validation/conflicting UART,
   plus exact help/argument process coverage. Pure parsing must perform no IO.
2. Loopback stream tests: genuinely queued bytes drain before Empty, EOF distinct
   from emptiness, pending ordinary reader wakes after a prior empty drain,
   bidirectional byte fidelity. Do not use packet boundaries as assertions.
3. Managed service with a literal TCP peer: actual V/D initialization, split
   replies and L frames between terminals; apply/read a fixture and prove native
   actual State and input event routing. Disconnect rejects current work, old
   state becomes unavailable, subsequent connection reinitializes and resumes;
   retained handles cannot keep the socket alive after stop. Test refused-port
   startup remains stoppable, and quiet-period byte activity remains owned by
   the existing monitor. Explicitly inject async-connect timeout completion into
   Hat to prove retry eligibility (paired UART timeout stays terminal), test
   connect future timeout/cancellation using paused time and an owned future,
   and reopen with inherited poison plus an immediate queued Boot to prove
   ordinary setup can proceed without a delayed/second Boot.
4. Connect the existing independent emulator core over loopback TCP in a bridge
   test (no changes to emulator files) for normal output/input behavior. This is
   complementary to the literal peer, not the sole wire oracle.
5. Source-only service type substitutions plus existing UART regression suite
   catch compatibility changes. Use targeted tests after edits; one final full
   native suite, strict scoped Clippy, Pi build, formatting, wait-lint and asset
   byte-check. No UI changes, so no fresh browser claim.
6. Named deliberate faults for endpoint selection/conflict, true-empty drain and
   bounded connector/ownership where practical, with independent behavioral
   oracles. Do not call a compile failure a successful negative control.

Documentation: running guide, service/contributor guide, wait registry and progress
record. Update handoff only for a changed shared seam; avoid editing emulator
files or its own guide. No SDK publication, live hardware/Store or deployment.

## Self-review before design gate

Pass1 checked actual endpoint/open call sites, complete daemon grammar and
Serial constructors in service tests. Seven manual Hat-session tests need their
Serial converted to Connection; other PTY fixtures keep Serial unchanged.
Pass2 checked neighboring failure arms: TCP Empty must be a real syscall result,
EOF must close the monitor, connect is canceled by the existing owned open job,
numeric address parsing happens before logging/config IO, and default UART must
not be opened when TCP is selected. Emulator-side connection/fixture lifetime is
explicit in the handoff. No remaining user requirement gap blocks this slice.

Design-gate verdict: surely yes; inspect buildability, receive-fence soundness,
UART compatibility and unasked requirement/alternative shapes. Same-family
review is available; independent model-family review is not claimed.

## Implementation and acceptance

Implemented the chosen Endpoint/Connection adapter, direct TcpStream DrainRead,
numeric CLI endpoint and existing-owner startup/reconnect. Eighteen executor
files have mechanical Serial-to-Connection substitutions; Serial itself and the
MQTT/config/v1 contracts are unchanged. Emulator implementation files and bridge
web source/assets were not changed. Both sequential source readers APPROVE,
including the final comment/test delta. Reviews are source-only, same-family;
gate token metrics are unavailable. One native review reported about75s plus
a20s extension; other review elapsed time was not reliably instrumented.

The first process regression failed because --hat-tcp made no TCP connection.
Twelve new cases cover CLI selection/invalid options, actual kernel drain/EOF,
pending reader wakeup, timeout/cancel drop, owner retry/poison differences, raw
literal replies/input events/pending-query disconnect/reconnect/actual EOF,
independent emulator core and refused-port metadata/stop. The final emulator
matrix programs minimum_level2 with actual repeated frames and checks independent
native readback under matching one/two-terminal profiles.

The full native run selected753 tests across11 binaries:752 passed, with only
the wait-registry test failing on a missing endpoint.open annotation; one opt-in
browser test was skipped. Added the v2-hat-open comment/row/generated waiter.
There was no subsequent production behavior change. Final scoped13 (the12 new
tests plus wait registry) pass,1.841s execution. Strict Clippy, Pi library/four
binaries, changed formatting, affected control plans, wait validation0findings,
asset byte-check and whitespace all pass. Logs: `/tmp/dali-tcp-acceptance/`
and final `/tmp/dali-tcp-restored/`. This is not claimed as a second fresh753-test
run. Prior browser/Node results are unchanged historical evidence.

All eight current deliberate faults produce their named REDs, plus the refreshed
old UART timeout control. An initial force-OneTerminal fault was GREEN against
the emulator matrix: do not infer that every surplus-terminal mismatch must
change its outcome. The final fault instead forces TwoTerminals against a
one-terminal peer, proving the missing-second-terminal case. No runtime behavior
was altered to make that control fail. Logs: `/tmp/dali-tcp-controls.log`,
`/tmp/dali-tcp-repeat-control{,-final}.log`, `/tmp/dali-tcp-uart-control.log`.

No physical HAT, live broker/Store, SDK publication, deployment, push or merge.
Standalone emulator TCP listener/scenario editor/live UI remain assigned to the
parallel session. The handoff and running/service guides document the seam;
the project contributor catalog and capability/progress records are updated.
