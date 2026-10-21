# P2 ATX emulator execution contract

Implements the already approved emulator architecture. Tier T2: independent wire
and device behavior. No physical HAT behavior is inferred from an emulator pass.

## Shape and independence

Keep the legacy in-process emulator unchanged. It constructs its fixtures from
the bridge inventory and bypasses UART, so reusing that ownership would hide
discovery and addressing bugs. The new `emulator` library owns physical fixtures
from a separate strict JSON scenario. `dali_hat_emulator` opens a private PTY and
prints its slave path; the bridge uses the production Serial/Transactions stack.
The HDL and Lutron emulators establish separate physical configuration and live
controls as useful patterns; their network transports and Dioxus UI are not
required for a UART core. Emulator browser controls follow the shared protected
HTTP work in P6/P7. P2 provides private local JSON controls for automation.

Files: `src/emulator/{mod,scenario,model,wire,session,pty,control}.rs`, the binary,
independent fixture JSON and integration/process tests. The emulator command
parser and reply encoder must not import the bridge's `atx::wire` codecs or
command constants. Golden byte examples assert literal values. The shared Serial
adapter is transport only and is appropriate for the PTY master.

## Scenario and model

Scenario version 1 has a deterministic seed, an explicit emulated repeat policy
(one terminal or two terminals), and 1–4 numbered buses. Outputs have private
fixture IDs, optional short addresses, initial level, group mask and 16 scene
registers. Inputs have separate addresses and instance metadata. A clock setting
chooses realtime (default) or frozen model time; `advance_time` moves either
forward explicitly. Live commands sample elapsed monotonic time plus that offset.
Repeat windows and INITIALISE expiry use only this model time. UART framing,
response fault delays, control deadlines and shutdown use Tokio time even when
model time is frozen. Validate counts,
addresses, ranges, IDs and scene lengths before opening resources. Duplicate
short addresses are allowed deliberately to model used fixtures/collisions;
fixture IDs remain unique. Fixture IDs never become bridge inventory identities.
The scenario and snapshot have no bridge names or MQTT targets.

Implement base output addressing, direct arc level with MASK, off/step/max/min,
level/status/device-type/version queries, DTRs, group and scene storage/query,
and INITIALISE/RANDOMISE/search/COMPARE/PROGRAM/VERIFY/WITHDRAW/TERMINATE.
Repeated configuration commands take effect only on a matching second frame
within 100 ms. The t command issues the pair inside that window. INITIALISE
expires after 15 minutes of explicit monotonic model time. Seeded randomization
is reproducible and never reads wall time. A device is selected only at exact
random/search address equality; withdrawn devices no longer compare. Missing
short addresses are represented by None, never silently address zero.
COMPARE includes eligible non-withdrawn devices at or below the search address.
INITIALISE 0 selects all, 255 selects unaddressed, and an odd short-address byte
selects that address. Repeat-required commands are INITIALISE, RANDOMISE, RESET,
configuration opcodes 32–48, scene store/remove, group add/remove and short-address
store. A different frame invalidates a repeat latch; the second matching frame
within 100 ms consumes the latch. Test both 100 ms and 101 ms boundaries.

Queries aggregate backward responses. Identical responses may coexist without a
collision; differing replies produce a receive collision. Duplicate addressing
must not make a mutating command update just the first fixture. Unsupported
capability commands have no invented positive device reply; later P4 extends
DT8/input/emergency/data models together with driver behavior and tests.

## UART session and controls

Parse lowercase h/t/j/l/m/v/d with optional bus 0–3, exact widths, LF/CRLF,
bounded lines and a total partial-line deadline. Publish uppercase monitored
forward frames and J/N/X/Z terminals independently. Emulated version text must
be documented as a test profile, not a hardware firmware claim. Unsupported
widths are accepted as transmitted frames with no fabricated device response.

One session task owns model, UART and control execution; use a bounded control
channel, no unbounded forwarding queue. Private control commands inspect state,
inject monitored input events/raw bytes, reboot the HAT, remove/restore a fixture's
short address, change bus state, and arm the next response fault (delay, drop,
collision, truncation). A delayed response must not block input injection or stop;
use a bounded timer queue. HAT reboot clears only HAT parser/output scheduling,
emitting B; fixture DTRs and initialization state survive. Fixture power-cycle is
a separate future control. A control whose caller has already canceled/expired
is skipped before execution; timeout after execution does not undo a mutation.
Every control has a bounded caller deadline and explicit success/error result.

Use control capacity 32, scheduled-output capacity 64, immediate-output capacity
256 frames/64 KiB. Full control queue returns Busy; scheduling/output overflow
ends the session explicitly rather than dropping protocol bytes. Next-response
faults apply to the next eligible terminal only (the first terminal in a repeated
pair); echoes/status/input events do not consume them. Equal due times preserve
insertion order. Delays use transport time, including in frozen model mode.

UART output is driven alongside reads/controls/stop, retaining each frame's write
offset across selects. Each frame has a 1 s write deadline; expiry closes the
session explicitly. An unread PTY must not hold controls or shutdown. Reboot
discards unsent/scheduled data; if a prefix was already accepted, send a newline
before B to delimit that interrupted record. Ordinary readiness/cancellation
never restarts a partially sent frame; truncation is an explicit fault.

Standalone control endpoint is an explicitly supplied Unix-domain socket, mode
0600 in an operator-owned directory. Bind without replacing an existing path;
remove only the socket owned by this process. Maximum request/response sizes,
one request per connection, connection limit and per-connection deadlines keep
unread/unterminated controls bounded. No TCP control or MQTT connection in P2.
CLI scenario loading and startup output use bounded detached workers; install
stop handling before filesystem setup. Runtime shutdown is bounded and must not
wait forever for stdin/stdout, a PTY peer, a filesystem worker or controls.

PTY creation uses openpty on macOS/Linux, raw/nonblocking master through Serial,
with the slave retained until shutdown to permit a later bridge connection.
Descriptor ownership is RAII; close releases both descriptors even if a control
handle survives. No symlink to `/dev/serial0`, no real UART opened by emulator.

## Verification order and limits

First failing tests: strict scenarios, literal UART vectors, independent model
addressing/group/scene semantics, repeated-command timing and MASK. Then actual
PTY tests initialize Transactions, query and change independent fixtures, inject
input while a reply is delayed, and recover from faults/reboot. Standalone tests
spawn the emulator, use its published PTY and private socket, and terminate it
with unread output/active clients. Add negative controls for framing bounds,
repeat gating, exact selection and queue/deadline protections.

Fresh design review before implementation and code review after the vertical
slice. Cross-family reviewers and physical HAT transcripts remain unavailable.
The support matrix distinguishes P2 base coverage, P4 extensions, unsupported
features and physical validation.

Reference behaviors checked against the existing legacy command table and
[ATX's manual](https://atxled.com/pdf/AL-DALI-HAT-I.pdf). The independent
[python-dali command definitions](https://github.com/sde1000/python-dali/blob/master/dali/gear/general.py)
provide a primary implementation reference for base gear commands and MASK.

Implementation refinement: P2 applies direct and scene levels immediately.
Timed UP/DOWN remain unsupported until P4 models fades; STEP UP does not ignite
an off fixture. Per-gear search state and modeled RESET defaults are checked
against [Renesas's reset tables](https://www.renesas.com/en/document/mah/dali-master-controller-gui-rx65n-cloud-kit-dali-2-option-board-users-manual).
Reboot delimiter tracking follows actual accepted bytes across complete, raw and
truncated records. PTY descriptors are close-on-exec and released before done.
See `docs/support-matrix.md` for the current capability boundary.
