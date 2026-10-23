# DT7 relay foundation

DT7 switching gear controls a contact using a virtual DALI level and directional
thresholds. Its actual level is binary0/254, so a positive virtual level is not
proof that the load is on. This development slice supplies checked native plans
and an independent ATX HAT model. Relay-specific managed settings, MQTT component
metadata and web controls remain pending; existing base power reads/writes use
the real actual level. No physical fixture validation or certification is claimed.

## Emulator use

Start `dali_hat_emulator --scenario examples/emulator/switching-i4.json` and use
its PTY like the [other scenarios](emulator.md). DT7 initial `level` is an actual
binary value0 or254. Omitting `switching` selects a fixed normally-open relay:
thresholds `[1,255,255,0]`, virtual min/max254/254 and raw gear type2. An optional
strict profile enables adjustable thresholds, selects four initial thresholds
in upward-on/upward-off/downward-on/downward-off order and supplies raw gear type.
Profiles belong only to device_type7; fixed profiles keep default thresholds.

The adjustable example still starts at native min254. Program virtual minimum
through native Part102 settings before testing a wider threshold range. For DT7
that minimum may be below physical minimum254; the current generic managed
fixture-settings preflight is not yet relay-aware. Native library/model support
for this distinction does not imply that the generic web form exposes it.

Private snapshots retain the initial `switching` profile when provided and expose
current `switching_state` only for DT7. Ordinary `level` remains actual0/254;
`switching_state.virtual_level` is test-bench evidence. Query/status reads never
operate the contact. Threshold programming alone leaves it unchanged. DAPC,
immediate steps, native recall/scenes and virtual-limit changes evaluate the
applicable directional pair. Disabled thresholds retain state; equal thresholds
prefer On. Reversing direction can change the applicable result without crossing
another threshold. Raw switch status is not a second source of contact truth.

The model changes levels immediately like its existing dimmer model. Timed virtual
fades/UP/DOWN, electrical faults, power/bus-failure transitions and legacy reference/
hold-off features remain unimplemented. Relay-aware scene capture and broader
managed capability workflows require the subsequent DT7 service slice. Stored
virtual scene levels and actual binary contact values must remain distinct.

## Native library for bridge contributors

`dali::gear::switching` is pure: it creates frames and decodes only replies already
correlated by the transaction owner. Upward threshold values are1..255; downward
values0..255. Disabled255 is distinct from ordinary actual-level MASK. Raw invalid
upward zero, silence and both collision directions/codes remain explicit evidence.
Raw feature/status/type/version bytes retain unknown bits; only common supported
feature/type flags are interpreted. Optional support must be queried, not inferred
from a sent command or HAT terminal.

Each setter plan is DTR0, one ENABLE DEVICE TYPE7 and one repeated addressed
setter. Run all three steps under one physical permit and the verified HAT repeat
policy. Do not insert a second enable between the two copies. Query plans accept
only an individual address and put enable immediately before the query. Actual
readback is a separate caller obligation; this module does not mutate inventory.

The independent model uses literal opcodes rather than these constructors. Its
per-gear100ms pair remembers the first correctly enabled setter, rejects addressed
interruption (including a deferred base configuration frame) and permits traffic
for another address. A HAT reboot does not reset fixtures. Native STORE ACTUAL
uses a repeated Part102 command and stores the virtual level for DT7. RESET
restores model defaults without erasing prior raw switch-status evidence.

Literal codec/model cases and standalone UART/PTY tests cover both HAT repeat
terminal counts. A managed base-output test checks actual power observations
and unchanged inventory. Fault controls: `v2-dt7-foundation-controls.toml`.
See [capability status](support-matrix.md) for the remaining vertical integration.

References: [Beckhoff behavior](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/10378581387.html),
[thresholds and diagnostics](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/10378583307.html),
[repeated setter](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/10375643403.html),
[public IEC preview](https://cdn.standards.iteh.ai/samples/15977/473f21f6cbf04867a4035e9424f134a9/IEC-62386-208-2009.pdf),
[manufacturer opcode table](https://www.controlfreakuk.com/images/download/LIDA-D-User-Manual.pdf).
