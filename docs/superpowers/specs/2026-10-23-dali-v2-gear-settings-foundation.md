# Common output settings: typed protocol foundation

## Problem and verified starting point

The bridge controls actual base levels and groups, but has no managed common
fixture settings interface. Operators need dimming limits, startup/bus-failure
levels and fade configuration. This foundation supplies checked native plans
and lossless query values before the shared service and protected UI are added.
No live hardware, configuration migration, Store, UI or UART lifecycle change.

Current `dali::gear` already owns checked addresses/levels, Destination command
framing and correlated Query replies. Minimum/maximum/physical minimum, status
and version queries exist. The independent emulator already stores and queries
minimum/maximum, power-on/failure and packed fade settings, implements DTR0 and
repeated setters, and models limit clamping. Legacy `set_light_fade_time` confirms
DTR/repeat framing, including the fact that basic fade0 alone is not an assurance
of instant output. Do not replace these mechanisms or invent another transport.
Memory recall found the generic BridgeKit decisions, but no prior output-settings
design. Current code and primary sources govern the contract.

Shapes considered: expose unchecked raw commands immediately; implement all
management/UI/DT6/extended-fade/timed simulation at once; or add a small typed
foundation over the existing base codec/model. Choose the last. It closes the
wire/value gap without widening ownership or pretending simulator timing exists.
The subsequent vertical service must add fresh physical preflight/readback;
constructing this plan alone cannot prove a fixture supports or attained it.

## Types and frames

Add `gear::settings` and extend the existing Query/QueryValue enums for power-on,
system-failure and packed fade time/rate. Keep existing query variants and their
semantics. All queries still require an individual short address and a correlated
BusReply. Never turn silence/collision into a value or a group query into evidence.

Checked types:
- LimitLevel: raw1..254, optionally positive normalized conversion through Level.
  It does not know the fixture's physical minimum or current opposite limit.
- PowerOnLevel: fixed ordinary Level (including Off) or LastActive (raw255).
- SystemFailureLevel: fixed ordinary Level (including Off) or Unchanged (raw255).
- FadeTime: code0..15; code0 has no ordinary seconds value and means basic fade
  time disabled/use the other supported fade mechanism. Codes1..15 use the native
  formula. Never label0 as proven instant or change extended/DT6 fast settings.
- FadeRate: code1..15, with native steps/second conversion.
- FadeSettings: preserve all raw packed bytes; upper nibble is FadeTime, lower
  is optional valid FadeRate (0 stays raw/unknown). FF is two numeric15 codes,
  not a MASK. Query power-on/failure255 has its own special meaning, distinct
  from actual/minimum/maximum255 unavailable MASK.

Setting variants cover Minimum, Maximum, PowerOn, SystemFailure, FadeTime and
FadeRate. Each creates exactly two Steps: broadcast DTR0 value (ordinary) and
the destination's addressed setter (repeated). Use existing checked Destination
for short/group/broadcast frames; query construction remains short-only. Steps
carry native frame and repeat requirement, not bus ownership or execution.
The future owner must keep the whole DTR/setting sequence under one permit and
use a verified HAT repeat policy. No automatic reset, arc-power command, store
configuration write, mode change or persistent-variable-save command is added.

The later managed API begins with individual gear, despite native codec support
for group/broadcast setters. It must fresh-read selected support and relevant
limits, validate the complete patch, order crossing min/max edits to avoid native
clamping, and compare actual requested settings. Minimum/maximum writes can
change an already-lit actual level; preserve Off and retire affected cached base/
colour/group evidence through the existing physical owner. Those responsibilities
are documented here, not implemented by this pure foundation.

## Independent model and boundaries

First test the existing independent model with literal opcodes rather than bridge
plans. It should already pass normal DTR/repeated store/query behavior. Add only
a demonstrated missing/correctness behavior; do not refactor the model to mirror
the codec. Check repeat interruption/expiry, target/group/bus scope, Off versus
lit limit clamping, power-on/failure255 raw values and fade nibble preservation.
Keep existing scenario and snapshot shapes. HAT reboot retains the existing
powered model; no mains-cycle or timed bus-failure/fade simulation is claimed.

## Implementation, self-review and checks

1. Add focused fail-first codec cases for legal limits, special255 meanings,
   packedFF/invalid rate0, exact code/time/rate conversions and independent
   short0/63/group15/broadcast DTR/setter/repeat vectors. A compile-only failure
   does not count as behavioral RED; use a temporary inert implementation only
   while establishing the new pure API's runtime oracles.
2. Implement checked values/plans and extend existing query decoding. Enumerate
   all Query/QueryValue consumers; existing actual/status/capability code remains
   unchanged. Add literal model regressions; report honestly when baseline model
   behavior already works. At most fix a demonstrated defect with its regression.
3. Measure serial controls for missing repeat, wrong setter/destination, merged
   power-on/failure MASK semantics, fadeFF treated as MASK, reversed nibbles and
   ineffective preservation of an unrequested nibble. No runtime ownership or
   test infrastructure framework is needed.
4. Freeze; sequential codec/contract and independent-model/compatibility source
   readers. Full native, strict Clippy/Pi, formatting/wait/whitespace/affected
   controls. No new web source or behavior, so retain the latest real browser
   evidence without a redundant UI run. Update codec/emulator/contributor guides,
   support matrix, CLAUDE and progress; managed service/UI remain explicit next work.

Two bounded self-review rounds checked existing command/model/consumer artifacts,
then context-specific255, basic0 versus effective fade, repeat ownership, raw
invalid rate0, limit clamping and model continuity. Primary source cross-checks:
[python-dali Part102 command source at the pinned commit](https://github.com/sde1000/python-dali/blob/4b21c18e79d02d2a97f91d135f0eee450e8c2fb9/dali/gear/general.py)
(retrieved through GitHub; file blob99a728c354b7fd12833a16d43515850db754117f),
[Beckhoff's native variable reference](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/5908382347.html)
and [DALI Alliance's DALI-2 technical note](https://www.dali-alliance.org/data/downloadables/1/2/7/1711_technical-note-dali-2_v3.pdf).
The public protocol source confirms setters2A..2F, queriesA3..A5 and repeated DTR
usage; native formulae and special-value descriptions agree with the vendor
reference. Physical certification and extended/DT6 effective fade control remain
unverified. Budget: two self-review rounds, one fresh combined design gate;
same-family review capability is the available limitation. No SDK, memory, live
system, deployment, publication or push/merge work is included.
