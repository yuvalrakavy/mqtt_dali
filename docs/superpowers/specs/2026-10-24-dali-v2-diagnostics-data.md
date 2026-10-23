# Part253 diagnostic and maintenance captures

Next approved comprehensive-bridge slice after Part252 acceptance. Tier T2 for
the externally visible read/report contract. The owner authorizes continuous
implementation. No Store/schema, legacy v1, dependency or live-hardware changes.

## Problem, grounding and chosen shape

Existing ordinary fixture status exposes immediate failure bits, but cannot tell
an operator operating hours, fault history, supply/output measurements or rated
life. Standard optional banks205/206/207 provide those observations. Their absence
does not imply a healthy fixture or a faulty bus. Memory recall found no competing
Part253 decision in the canonical Store corpus. Approved architecture section8
already includes Data251–253.

Reuse the existing Memory action/owner/permit and protected Device information
dialog. Alternatives: a new diagnostic poller/cache would add runtime and stale
State semantics; exposing only raw bytes would leave users unable to interpret
standard measurements. Choose an explicit read of one checked standard bank,
with decoded fields and original raw evidence. Do not infer retained fault State
or merge this historical capture into the existing ordinary status bits.

Current analogues: src/dali/energy.rs, src/service/memory.rs and proxy.rs,
src/emulator/memory.rs, web/src/energy.ts and device-info*. The Part252 capture
already proves FF→AA and release under one original15s owner; generalize its
bounded last location and error context for the two dynamic diagnostic banks.
No new worker, retry, cache, journal or public memory-write API.

## Standard layout and exact interpretation

Primary definitions: python-dali memory/diagnostics.py, maintenance.py and
location.py (DiiA Part253 version1.1 definitions). Banks205/206 are latchable,
with version1 at3 and last standard locations1C/20. Static bank207 has version1
at3, last7 and no latch. Use header-bounded prefix through min(last,3); absent,
reservedFF or unreadable header/version and unknown versions never authorize
extension or latch commands. Dynamic supported layouts require the known HAT
repeat profile1/2 before any native traffic. Static207 remains usable under
Unverified repetition policy because it needs no repeated command.

All values below are big-endian unsigned raw integers. Every field supports
TMASK (allFF endingFE), which means temporarily unavailable. AllFF means
read_protected ONLY for the marked diagnostic fields; other diagnostic allFF is
invalid. AllFF for maintenance fields means unknown. Missing/collided/unvisited
bytes mean unavailable and never combine across holes. Boolean raw0/1 only;
other unreserved codes are invalid. Temperature is unsigned raw minus60°C,
not signed two's complement. Decimal scaling uses integer arithmetic and
canonical strings, not floating-point JSON values. Limits exclude TMASK/MASK.

Bank205 (driver,19 fields):

| Key | Locations | Conversion / maximum | allFF |
|---|---|---|---|
| operating_time |04..07| seconds, FFFFFFFD | invalid |
| start_counter |08..0A| count, FFFFFD | invalid |
| supply_voltage |0B..0C| ×0.1 Vrms, FFFD | read_protected |
| supply_frequency |0D| Hz, FD; zero indicates DC/rectified AC | read_protected |
| power_factor |0E| ×0.01, maximum100 raw | read_protected |
| overall_failure |0F| boolean | invalid |
| overall_failure_counter |10| count, FD | invalid |
| undervoltage |11| boolean | read_protected |
| undervoltage_counter |12| count, FD | read_protected |
| overvoltage |13| boolean | read_protected |
| overvoltage_counter |14| count, FD | read_protected |
| power_limit |15| boolean | read_protected |
| power_limit_counter |16| count, FD | read_protected |
| thermal_derating |17| boolean | read_protected |
| thermal_derating_counter |18| count, FD | read_protected |
| thermal_shutdown |19| boolean | read_protected |
| thermal_shutdown_counter |1A| count, FD | read_protected |
| temperature |1B| raw−60°C, FD | invalid |
| output_current_percent |1C| %, maximum100 | invalid |

Bank206 (light source,17 fields):

| Key | Locations | Conversion / maximum | allFF |
|---|---|---|---|
| resettable_start_counter |04..06| count, FFFFFD | invalid |
| start_counter |07..09| count, FFFFFD | invalid |
| resettable_on_time |0A..0D| seconds, FFFFFFFD | invalid |
| on_time |0E..11| seconds, FFFFFFFD | invalid |
| voltage |12..13| ×0.1 V, FFFD | invalid |
| current |14..15| ×0.001 A, FFFD | invalid |
| overall_failure |16| boolean | invalid |
| overall_failure_counter |17| count, FD | invalid |
| short_circuit |18| boolean | read_protected |
| short_circuit_counter |19| count, FD | read_protected |
| open_circuit |1A| boolean | read_protected |
| open_circuit_counter |1B| count, FD | read_protected |
| thermal_derating |1C| boolean | read_protected |
| thermal_derating_counter |1D| count, FD | read_protected |
| thermal_shutdown |1E| boolean | read_protected |
| thermal_shutdown_counter |1F| count, FD | read_protected |
| temperature |20| raw−60°C, FD | read_protected |

Bank207 (luminaire maintenance,3 fields): rated_life at4 ×1000 h,
reference_temperature at5 raw−60°C, rated_starts at6..7 ×100 count.
Maxima FD,FD,FFFD respectively; allFF unknown and TMASK temporary for all three.
No counter-reset, unlock55, manufacturer-data write or rated-life programming.
Resettable counters are read as existing observations, never reset by this action.

## Owned native/report contract

Add diagnostics::Bank checked enum Driver205/LightSource206/Maintenance207.
Expose Read-class read_diagnostics with exactly integer parameter bank205..207,
individual saved output only; typed API takes checked Bank. Preserve original
prepared revision, generation, trace, physical stamp and final reply acceptance.
Same Memory Work/permit/original15s, no renewed cleanup budget or connection.

For supported205/206, reuse the checked snapshot sequence: establish observedFF,
write repeated addressed81 + DTR0=2 + C9AA, proveAA, capture fields through
min(header,last_standard), proveAA, release with fresh repeated81 + DTR0=2 +
C9FF, proveFF. Failed proof is an error, not accepted values. Once AA may have
been submitted, bounded release uses the same link/stamp/deadline; preserve
original error and existing StreamLoss recovery behavior. No retry after expiry.
For supported207 extend directly through min(header,7), no writes or snapshot
claim. Header bounds and holes remain evidence. Independent banks are separate
captures, never one global atomic snapshot.

Common Report adds optional diagnostics, omitted on all older report families.
It has version, format_status supported|unrecognized|unavailable, fields and
snapshot:null for207/unsupported layouts, or AA/AA/FF proof for supported205/206.
Fields: status value|temporarily_unavailable|read_protected|unknown|unavailable|
invalid, raw:u64|null, value:string|boolean|null, unit:string|null. Counts use
unit count; power_factor has null unit. Boolean value is real JSON boolean;
numeric values are canonical decimal strings. Raw remains diagnostic evidence
for non-values. No scaled value unless status=value. Complete quality means all
requested locations responded and applicable snapshot proof succeeded; field
invalid/MASK/TMASK is still a received byte, not missing transport evidence.

## Emulator and browser

Extend explicit output-only latch:true capability to205 (at most29bytes) and206
(at most33bytes), keeping202..204 limit16. Reject latch:true on207 and input
modules. Existing independent State already separates frozen copy, runtime lock
and configured bytes. Preserve old default-false profiles and read-only behavior.
Raw values remain scenario configuration; no aging, operating-time accumulation
or causal thermal simulation. Add one example with all three banks and a few
MASK/TMASK observations. Native literal tests drive independent81/C9/C5 frames
and mutate live bytes after freeze to prove new bank lengths and isolation.

In Device information add a compact collapsed Diagnostics and maintenance
expander, selector Driver/Light source/Luminaire maintenance and explicit Read.
Follow the already approved advanced-control interaction; changing selector
does not issue a read. Use keyed persistent details so failed reads retain the
visible controls. Show labeled rows with units, Yes/No for booleans, status text
and expandable raw diagnostics; all39 fields are supported but only selected
bank's fields render. Snapshot confirmation only for dynamic supported capture.
Original frozen scope and synchronous retirement remain; new/failed reads clear
old results. Phone/desktop scrolling stays inside the existing dialog. UI checks
derived fields against raw producer evidence, including boolean/range/MASK rules.

## Plan, tests and documentation phase

1. Pure table-driven decoder failing tests: all39 field offsets/widths/units,
   exact scaling, booleans0/1/2, negative/zero/maximum temperatures, maxima,
   FF vsFE by per-field support, 100/101 percentage/power-factor boundary,
   holes/truncation/reserved/unknown versions. Independent literal fixtures.
2. Extend independent model; fail-first literal tests for205/206 capture after
   live edits, lengths and rejected207/input latch, original scenario preservation.
3. Reuse native owner/action/report and strict proxy, checked typed API. Literal
   UART for205/206 fresh snapshot/release/holes/collision and207 no-write under
   Unverified; TCP/MQTT both known policies; strict target/params, unknown/absent
   banks, original revision/generation/deadline/stamp, read class and unchanged
   native settings/levels/colors/scenes/groups/config/caches. Mid-checkpoint.
4. Strict frozen-scope browser decoder and controls, unit tests for exact values,
   nonvalue semantics/malformed/late/lost reports. Real Chrome phone/desktop
   all3 banks, long33-byte raw table, boolean/temperature/status labels,
   unknown/absent/truncated data, lost/held reply, explicit retry and retirement.
5. Documentation: diagnostic guide, common service/MQTT/emulator pointers,
   support matrix/progress, example scenario and negative-control registry.
   No Store memory/skills edits. Mutation finder, frozen fresh depth review,
   then full native/frontend/linked browser, Clippy/Pi, wait/format/assets/plans.

Bounds: one33-byte internal capture; public raw limit64 unchanged; emulator at
most5 explicit latchable standard banks per output. Same owner15s cap, no new
runtime tasks. Physical HAT/fixture verification remains a release prerequisite.

Self-review: do not carry Part252 allFF semantics onto per-field diagnostic MASK;
do not treat207 static lock byte as a latch; do not turn unknown/read-protected
into zero/false; do not reset counters or infer ordinary retained fault state.
Existing energy controls and report shape must remain compatible after helper
generalization. Verify against primary source and fresh design gate before code.

Primary references verified2026-10-23:
https://github.com/sde1000/python-dali/blob/master/dali/memory/diagnostics.py
https://github.com/sde1000/python-dali/blob/master/dali/memory/maintenance.py
https://github.com/sde1000/python-dali/blob/master/dali/memory/location.py

Design gate: fresh GPT-6 Astra/high review APPROVES without P1/P2/P3 findings.
The field table and all three bank semantics were checked against primary sources
and the current owner/admission/model/browser analogues. Preserve the pre-traffic
repeat-policy guard for205/206 only, and partial quality for unknown/truncated
layouts. Same-family/source-only review; implementation and physical validation
remain separate.

