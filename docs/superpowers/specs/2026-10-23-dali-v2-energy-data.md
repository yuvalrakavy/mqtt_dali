# Part252 energy and power captures

Approved next bridge slice, after Part251 acceptance at7938bab. Tier T2. Implements
approved architecture section8 incrementally. The owner requests continuous
implementation without routine approval beats; no Store/schema, v1, dependency,
publication or live-hardware change is involved.

## Grounding and choice

The accepted memory path has checked individual targets, one physical owner,
original revision/generation/deadline/stamp and literal C3/A3/C5 plans. Part251
adds one bounded internal capture with explicit format proof. The emulator has
optional raw bank profiles; neither driver nor model currently implements memory
latch writes. Current code: src/dali/memory.rs, src/service/memory.rs,
src/service/physical.rs, src/service/proxy.rs, src/emulator/memory.rs,
src/emulator/model.rs, web/src/device-info*. Memory recall found no Part252
decision in the canonical Store corpus.

Use the same Memory action/report/owner for one selected standard bank. Add a
pure energy decoder and a tightly checked latch plan; no generic public memory
write API. Reject a new worker/poller/cache, floating-point scaling and claims
that several sequential bank reads are one atomic snapshot. Reading one bank
with a latch avoids torn multi-byte energy/power values. Unlatched reads remain
available through the existing raw API without snapshot claims.

Primary definitions: python-dali memory/energy.py and memory/location.py, and
gear/general.py. Bank202 active input,203 apparent input,204 active load-side.
Each uses locations0..0F, version1 at3; energy scale4 plus six-byte mantissa5..A,
power scaleB plus four-byte mantissaC..F. Scale is signed two's-complement -6..6.
Mantissa allFF = invalid; allFF endingFE = temporarily unavailable;
other values through allFF endingFD are valid. These energy fields support TMASK,
not MASK. Optional banks can be absent; silence alone cannot prove why. Scale is
checked independently before TMASK/range validation. Energy/power units are
Wh/W,VAh/VA,Wh/W respectively.
Values are static configuration in the emulator until explicitly edited; no
energy-consumption simulation is claimed.

## Native capture and authority

Add checked energy::Bank enum Active/Apparent/Loadside with native bank numbers.
MQTT/protected jobs: read_energy_data with exactly one integer parameter bank,
limited to202,203,204, individual saved output only. Read class, same original
prepared revision and absent/present generation semantics. Typed service uses
the checked Bank. No mutation completion or expected_revision payload.
`Hat::enqueue_memory` requires a known repeat-terminal profile1 or2 for this
action before any native commands, following enqueue_output_settings. Unverified
profiles refuse explicitly; identity/raw/luminaire reads remain usable. The
existing transaction engine refuses repeated sends under an unverified policy.

One Memory Work retains one permit, original15s cap and mapping throughout.
Select bank and read bounded prefix through min(header,3), explicit DTR0 each
location. Unknown/missing version, absent/reserved header: no latch writes,
no guessed fields, exact raw prefix and explicit quality/layout status.
Only version1 with header>=3 permits a snapshot attempt. Read existing lock byte
as evidence. Establish observedFF before starting: if the prefix latch is notFF,
writeFF with fresh repeated enable/offset2/C9FF and verify it readsFF. Refuse if
that baseline cannot be proved. Then repeated addressed ENABLE WRITE MEMORY (opcode81), DTR0=2,
WRITE MEMORY LOCATION NO REPLY (specialC9, valueAA). Reload offset2 and queryC5
to verifyAA. Query acknowledgement alone does not prove the latch. Extend capture
under that same owner through min(header,0F); already-read prefix remains raw
preflight evidence, fields4..F come from the latched phase. No retry of holes.
After the fields, explicitly read offset2 again to prove the latch remainedAA.
Release using a fresh repeated ENABLE WRITE MEMORY, offset2 andC9FF, then read
offset2 to proveFF. READ MEMORY can disable write enable, so do not assume the
first enable survives the capture. No55 unlock or writes outside latch location2.

The report marks a snapshot confirmed only when latch start/end and release are
confirmed. If latch/release proof fails, report an evidence failure, not accepted
measurements. OnceAA may have been submitted, attempt bounded release after
transport errors or failed latch evidence (including silence/collision), only while
the existing link/stamp/deadline still permit ordinary commands; preserve the
original error. Never open another connection, extend the deadline, retry a whole
capture, journal state or require a power cycle. Cancellation/owner teardown and
expiry cannot guarantee cleanup beyond the operation's existing lifetime; state
this limit. Every subsequent explicit energy read establishes a fresh latch.
No new persistent recovery policy. One bridge serializes scratch access; no new
multi-master coordination promise.

Memory Work's writes/cuts stay false: latching changes only a bank snapshot,
not level, color, settings, scene, groups or input evidence. Service authority
checks still occur at admission/settlement/reply; metadata does not rebind names.

## Report and browser

Extend common Report with optional energy, omitted on identity/raw/luminaire
reports. Preserve the bank, raw prefix/field readings, route/revision and
configuration_changed:false. energy has version and format_status, a snapshot
proof object for latch start/end/release readings, and fields energy/power when
layout supported. Each field: status value|temporarily_unavailable|
unavailable|invalid, raw:u64|null, scale:i8|null, value:decimal-string|null,
unit:string. Raw six/four-byte mantissas are exact; decimal value uses integer
arithmetic/string formatting, never f64 or JSON number for a scaled result.
For example281474976710653 at scale6 is281474976710653000000Wh, not a rounded
number. Scale-6 preserves fractional digits. Zero stays a real zero. Raw and
scale may remain available diagnostically on non-value fields; value is null.
Unsupported/unreadable format leaves fields empty. Quality complete means every
requested location responded and snapshot proof succeeded, not every field
implemented. Holes/truncation or unknown version are partial; no header reply is
unavailable. Preserve original unavailable/collision evidence without filling gaps.

Within Device information add collapsed Read energy data controls: bank selector
with Active input/Apparent input/Load-side and explicit Read button. Existing
identity/luminaire/raw actions remain. Show two captured values with exact units,
scale/raw diagnostic access, per-field statuses and snapshot confirmation. No
automatic polling or read on changing the selector. The original protected scope
and cancellation rules apply. Failed/new reads clear the previous result. Keep
phone/desktop body/footer scrolling and raw table bounds.

## Independent emulator

Add optional per-bank latch:true (defaultfalse, reject input-module scope,
non-energy banks and latchable arrays longer than16 bytes) without
changing old scenario behavior. Runtime per-output write-enable/latches are
separate from persistent raw profiles. Decode literal81/C9 independently; require
the existing repeat pair for81 (extend its current range ending80) and observe
write-enable resets before early repeat returns. Disable write-enable on unrelated
commands as the protocol requires. Accept writes only to location2 of explicitly latchable
banks with valuesAA/FF. Other memory writes stay unsupported and unchanged.
AA freezes a copy of the bank's raw locations; reads use frozen values untilFF.
Runtime location2 overlaysAA/FF separately from configured bytes and frozen payload.
Snapshot/capture/export preserve configured bytes and latch capability, not a
runtime frozen copy as installation data. Scenario reset creates fresh scratch
state; HAT transport reboot follows current device-state retention semantics.
Literal tests mutate the live profile internally between reads to prove snapshot
stability and release, without deriving expected frames from the bridge codec.
Add a ready-to-use three-bank scenario, invalid/TMASK/scales/optional-bank absence.

## Execution and verification

1. Pure decoder fail-first: exact48/32-bit maxima, scales-6/0/+6, signedFF=-1,
   invalid scale7/F9, validFA=-6, TMASK/range independent of scale, missing scale/mantissa,
   holes, zero, truncated/header/version gating. Fixed producer-shaped fixtures.
2. Independent model/latch codec literal tests before managed wiring: correct
   bank/address, repeat enable, disabled/unrelated traffic, frozen vs live data,
   bank/bus/gear isolation, prohibited writes and original profiles unchanged.
3. One owned action + strict parser/API, literal UART complete/collision/proof
   refusal and independent TCP/MQTT under both known repeat policies, plus an
   unverified-profile refusal with no native send. Original prepared
   revision, generation, stamp/deadline and cache/config/native state preservation.
   Mid-checkpoint before UI.
4. Frozen UI scope/report decoder, exact decimal/status/unit tests and real
   phone/desktop reads across3 banks, missing/unrecognized versions, lost/held
   replies, explicit refresh and no automatic retry/read on selector changes.
5. Documentation/skills phase: guide, common service/MQTT/emulator pointers,
   support matrix/tracker; no Store memory/skill edits. Mutation finder first,
   frozen fresh depth gate, final full native/frontend/browser, Clippy/Pi,
   waits/format/assets/control-plan/whitespace checks. Physical validation pending.

Bounds: max16 bank bytes, one at-most16-byte frozen copy per explicitly
configured model bank (at most3 standard energy banks per output), bounded
report and UI clone; one15s physical owner hold.
Model supports latch profiles only in standard energy banks for this slice.
No Part253 field guessing, setting writes, extra runtime or SDK change.

Self-review: the primary Part102 command definitions confirm repeated addressed81,
broadcastC9, allowed DTR/query-DTR exceptions and unrelated-command write-disable.
The latch/release steps remain inside the existing owner/deadline; no new recovery
states. Unknown versions do not authorize scratch writes. All derived decimal
values remain strings. Confirm transient-error cleanup against commissioning::Bus
before implementation; an expired deadline never buys a new cleanup budget.

Design gate: initial fresh review requested two corrections (allFF is invalid,
not MASK; require observedFF beforeAA). Both and the byte2/release/repeat/scale
clarifications were applied; the confirming design verdict APPROVES. Root's
implementation preparation also verified the existing unverified-repeat refusal
and added its admission guard/negative case. Source-only, same-family review;
runtime and physical validation are separate.

Primary references verified2026-10-23:
[energy definitions](https://github.com/sde1000/python-dali/blob/master/dali/memory/energy.py),
[memory latch/value mechanics](https://github.com/sde1000/python-dali/blob/master/dali/memory/location.py),
[Part102 commands](https://github.com/sde1000/python-dali/blob/master/dali/gear/general.py).
