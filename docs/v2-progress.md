# v2 implementation progress

Development checkout: `Home/mqtt_dali-v2`, branch `feat/dali-v2`.
Base: `db6f39f63da75a0614eb8f1a3b274a163bc4b307`.
The approved architecture and phase acceptance criteria are in
`superpowers/specs/2026-10-21-dali-v2-design.md`.

Bridge validation carried forward: compact current identity snapshots replace
unbounded rename history. The latest command-lifetime slice passes **869/869 native tests**
(one opt-in browser test skipped) and strict production Clippy; ARMv7 Pi library/bin
compile checks pass. **125/125 frontend tests** pass from the unchanged UI baseline. The complete
linked phone/desktop TCP Chrome workflow passes in172.294s. Five compact-identity
mutation controls fail for their expected behavioral assertions. Strict v2
production Clippy, all-bin Pi compile checks, changed-file formatting, asset
consistency and whitespace checks pass. Physical hardware validation remains
pending. The prior DT1/native capability milestone remains accepted at a9acfa5,
0438fea and c52ef8b. The compact slice is accepted locally at787c011 plus5c601e0;
the final confirming source review approves both corrections.

Current work: P8 Store integration after the accepted compact identity correction in `/Users/yuval/Documents/Projects/Store-t3`, branch
`feat/dali-v2-store`, based on Store `3c168f3ce`. The old synthetic324KB test
contained two fixtures, one group and1200 past renames, not a large real
installation. It exposed unbounded history. The owner approved replacing it with
current identity snapshots, bounded histories and a shared 30,000-entry map limit.
All eight practical/dense real-MQTT capacity phases pass. Store correction7e67422
passed3303 native checks (11 skipped;1231.4s); the wrapper stopped before SDL so the
review fixes could be applied. Three expiry regressions reproduced the original
bugs;135 DALI/shared MQTT checks now pass, along with strict workspace Clippy.
The corrected earlier-run/name-reuse test fails under its actual guard removal.
The expiry correction is committed at Store `aa78be4df`; all19 final focused checks,
42 shared keeper compatibility checks and15 data-lint rules pass. The single fresh
confirming source reader approves with no S2+ findings (same-family coverage).
An initially stale base test template was refreshed without rebuilding the full
template catalog. Actual process-restart acceptance passes2/2: rename keeps the
physical ID and request; replacement changes identity and refuses it. A one-sided
expiry precision check was corrected only in the test; final acceptance passed.
The foundation correction is confirmed locally. Opt-in BridgeKit request-lifetime
enforcement is now complete: DALI uses a 180-second ceiling, while other bridges
retain their defaults and public SDK structs remain unchanged. Shared color
components/equipment and split-address composition now pass the focused integration
checks and actual MQTT/TCP-emulator path; their final fault-injection/source review
is running at Store `896ab2213`. Next are native input components and opaque
hardware scenes in presets.
No household migration or physical hardware validation has occurred.

## 24 October — Store color binding checks pass

The shared color slice passes **108/108 scoped Store checks** and the actual
Home → binding → MQTT → canonical bridge → TCP HAT emulator path. RGB, RGBW,
RGBWAF, Kelvin and xy equipment use matching shared components; independent
RGB/RGBW equipment uses three/four output or group bindings. Final fault-injection
checks and fresh source review are running; this is not full P8 acceptance.

Store commit `896ab2213` preserves color/master brightness through split-channel
power off/on and prevents cached child brightness from switching a fixture merely
on rebinding. Expired unknown color values clear through Home bindings. Normal
DALI native-step rounding confirms actual readback; other bridges retain exact
comparison by default. Replaying an unchanged inventory upgrades the preceding
plain output/group projection without changing IDs, Home bindings or revisions.
Both boot ID lock files and the wire format are unchanged.

The emulator reports native RGB `[127,64,254]`, independent group levels
`[102,51,25,76]`, and actual Home green `64/254`; master brightness stays `0.4`
through off/on. Group operations invalidate cached bus observations, so the
polling-disabled test explicitly refreshes the final fixture state. All15 source
lint rules pass. No physical HAT, household migration or deployment is claimed.

Remaining P8 work: final color controls/review, native button/switch/sensor
components, opaque hardware-scene equipment, scene-first mixed-preset ordering and
combined acceptance. PanelScript UI implementation remains outside this effort.

## 24 October — Command lifetime complete; P8 readiness checklist

The canonical v2 daemon now opts into a three-minute request-lifetime ceiling using
BridgeKit revision `d83fa224bf04216dcde4be4102b1b2dc98fd7010`. Nonempty property
Desired messages require MQTT expiry of at most 180 seconds, including live
deliveries; queued commands and target Desired requests receive a deadline bounded
by the same ceiling. Shorter deadlines remain shorter. Empty retractions, Config,
State readback and existing active-operation timeouts keep their existing behavior.
This limits admission/start time; it does not abort an addressing operation already
started or prove a publisher's original age from broker remaining expiry.

Bridge commit `cccaf2a` passes 869/869 native tests (one opt-in browser case skipped),
strict production Clippy and ARMv7 Pi library/bin compile checks. The canonical
daemon regression failed before the change and passes afterward. The SDK passes
231 API/example tests, 176 conformance tests, strict all-target Clippy and one actual
mutation control. Both fixed and factory-created runtimes are tested. Fresh source
reviews approve the SDK and DALI integration with no serious findings; these are
same-family reviews. Other bridges retain the default policy and their dependency
pins. The separate v1 runtime is unchanged. SDK commits are published for the DALI
pin; there is no bridge release, household migration or hardware-validation claim.

P8 can become **ready locally** once all of these boundaries are complete:

- [x] Inventory sync, stable physical IDs and Home bindings through renames/address
  moves; replacements refuse old requests; tested persisted upgrade.
- [x] Bounded histories, 30,000 aggregate map entries, bounded DALI parsing and
  original request expiry through retries, renames and process restarts.
- [x] Opt-in bridge-side three-minute request lifetime, preserving other bridges'
  defaults and the separate v1 runtime.
- [ ] Shared representation-specific color components/equipment: one DT8 binding
  and three/four independent RGB/RGBW channel bindings, including DALI groups.
  Preserve color independently of power/master brightness and avoid double scaling
  or readback feedback loops. Preserve existing object identities/bindings when
  refining capabilities; verify ordinary dimmer/on-off compatibility too.
- [ ] Native input components and bindings: decoded button events, maintained
  switches, occupancy and supported sensor observations. Start unknown without
  fresh data and use the native classifications without another gesture recognizer.
- [ ] Opaque DALI scene targets, generic HardwareScene equipment and shared recall
  trigger, integrated into mixed-protocol presets. Recording saves participation
  and references; activation recalls once; there is no automatic inverse recall.
  Scene recall must take effect before individual-light overrides on the same bus.
- [ ] End-to-end Home binding/preset tests through actual MQTT and the TCP emulator,
  plus relevant upgrade and existing-driver compatibility checks, contributor
  documentation and final scoped review.

The next implementation slice is shared color components and equipment. PanelScript
touch-panel design is outside this effort. Physical HAT/fixture acceptance and P9
release/deployment preparation remain separate; locally ready does not mean deployed
or hardware validated.

Evidence: `/tmp/dali-request-lifetime-{daemon-red,daemon-green,native-final,production-clippy,pi}.log`
and `/tmp/dali-request-lifetime-{sdk-final2,conformance-info-final,control,clippy}.log`.

## 24 October — Store foundation correction and lane cleanup

The Store foundation correction is confirmed locally: bounded histories, the
30,000-entry map limit, bounded DALI inventory parsing, safe rename/replacement
handling and preserved request expiry. P8 remains in progress; shared components
and preset integration are still unfinished.

Production correction: Store `aa78be4df`; final verification/test record:
`5d1c8f1ca`. Checks pass: 135 DALI/shared MQTT cases, 19 final focused cases,
42 shared keeper cases, 2 actual broker/process-restart scenarios, strict workspace
Clippy and 15 data-lint rules. Eight practical/dense MQTT capacity phases also pass.
The final source review approves with no serious findings; review coverage is
same-family. The earlier 3,303 passing native cases are phase evidence, not a full
pre-merge verdict. Nothing is merged or deployed, and hardware validation is pending.

At the owner's request, the idle t3 lane was cleaned with `cargo clean --profile dev`.
Its measured target-directory disk usage fell from 49 GB to 255 MB. Server logs,
configuration and control files remain, the lane server is still running, and other
lanes were untouched. Cargo's missing cache tag was restored after verifying its
target directory. The next Rust build will be cold.

At this checkpoint, opt-in bridge request-age enforcement was next. It is now
complete, as recorded above. Shared color/split-address RGB, input and opaque scene
components, scene-first preset ordering, and release preparation remain.

## Compact identity correction — accepted locally

Bridge wire schema2 now publishes only current Origin/projection/intent-barrier
records, including parent Origin for input instances. Names remain public names.
Saved configuration checkpoints its current identities when diagnostic history
exceeds32 batches or32KiB, then discards that tail. Existing v2 ledgers are read
without changing epoch/revisions or private scene references. The raw loaded
Inventory remains the atomic writer's comparison basis until ordinary save.

Focused checks pass:74/74 inventory/writer/consumer cases, including1200 real
Model edits and offline/restart continuity, color-change-and-back revocation,
checkpoint parent replacement, byte/count bounds and actual worker upgrade/save/
uncertain recovery. The final complete native suite passes868/868 (one opt-in browser
case skipped there); frontend125/125. Four original mutation controls fail for
the expected behavioral reason (22.1s total); the review-added fresh-intent control
also fails meaningfully (5.1s). Strict v2 production Clippy and all
Pi-target library/bin compile checks pass; all-bin strict Clippy also encounters
four pre-existing legacy harness warnings. Full linked phone/desktop TCP-browser
verification passes1/1 in172.294s. Round1 source review found an example-consumer
schema downgrade hole and a missing positive fresh-intent case. The downgrade
regression was behavioral RED, then fixed; fresh intent after revocation is
positively tested and its conditional ratcheting mutant is RED. Current legacy
adversarial fixtures explicitly remain on schema1 so the new refusal cannot make
them pass vacuously. The confirming delta review finds both findings addressed, with no serious
unresolved defect (`/tmp/dali-compact-confirm.md`). Logs `/tmp/dali-compact-{affected2,native-final,web,controls,production-clippy,pi,browser}.log`.

Two required fresh design readers (Astra/high and Sol/high, same GPT family)
identified and resolved raw-file comparison, retained-vs-memory durability,
adoption metadata and explicit parent-Origin issues. Reports are in
`/tmp/dali-compact-identity-design-{a,b}.md`; independent-family coverage is absent.
Design `superpowers/specs/2026-10-24-dali-v2-compact-identity.md` supersedes the
unbounded-history requirement. The512KiB State and4MiB config caps stay in force.

## P8 — Store foundation: compact snapshots

The following compact-snapshot results are earlier checkpoints, before the approved
map-budget/history correction. Current acceptance is summarized at the top of this file.

The bridge compact-history correction is complete. Store integration in the owned
Store-t3 lane now consumes schema2 current identity snapshots. Names remain public
names; private lifetimes preserve physical IDs and Home bindings across proved
renames and address moves. Replacement lifetimes never inherit old requests.
All shared runtime limits remain unchanged.

The combined focused run passes **43/43**:32 DALI inventory/state/request/adoption
cases,8 shared generation/lifetime cases and3 literal-name cases
(`/tmp/dali-store-foundation-focused-final2.log`). It covers real Home bindings,
address swaps, bus/input renames, same-name replacements, stale/invalid atomic
refusal, fault/null observations, bounded early State, original-expiry retargeting,
reviewed legacy joins/retirement and explicit epoch replacement. Actual bridge
Model output supplies the fixtures. All7 mutation controls produce meaningful
behavioral RED and restore source; final anchor check has zero rot.

All **8/8 native upgrade cases** pass together
(`/tmp/dali-store-migration-bootstrap-final.log`,63.912s body). These cover actual
existing-store load and template reset, preserved IDs/values/bindings including
singleton-name collisions, interrupted-v25 recovery, rollback, generic legacy
nonce conversion, Std-only readiness and unrelated modified-driver ordering.
The existing migration regression selection passed31/31 in59.413s before the final
bootstrap correction. Both boot ID lock files are unchanged.

Real old-template testing exposed two bootstrap ordering failures: an all-owner
refresh reached unrelated HDL catalog data too early; recursive dependency refresh
could compile a dependent before Std installed its annotation APIs. Upgrade now
refreshes present Std then Dali owners with dependency recursion disabled, using
the existing reconciliation API. The formerly failing cold base-template path
passes, followed by8/8 generation checks
(`/tmp/dali-store-base-cold-bootstrap.log`,45.5s cold rebake). A fresh source reader
confirmed this narrow fix; that read is not the final frozen-source review.

The actual MQTT/server-restart acceptance passes **2/2**
(`/tmp/dali-store-real-restart-acceptance2.log`): a renamed fixture keeps its
physical ID and recovers its captured request on the new route with its remaining
expiry; a replacement gets a new physical ID and refuses the old request. Old
routes are retracted. The reproducible recipe uses disposable stores/unique roots,
captures actual producer messages, and reseeds them after graceful test-store
teardown to model a previous crashed publisher. It never touches household data.

Earlier carried checks include57 real-broker kit cases plus the corrected orphan
readiness1/1, literal naming9 existing +5 shared unit cases, and the actual compact
MQTT/parser probe1/1 with the unchanged10000-entry map cap. The foundation is frozen locally in Store at0aa7fd54e; strict workspace Clippy
and all data lint rules pass. It is not merged or deployed. The derived gate runs
beside two fresh same-family source readers. Round1 found reused-name request
collisions, large-BLOB guard mistakes, cache invalidation across Root/client
changes, and stale availability after bus enable changes. Corrections and new
regressions are prepared outside the gated checkout; they are not tested or
accepted yet. Owner policy: discard old commands on reused-name conflicts while
preserving fixture IDs/Home bindings, and discard commands that cannot take
effect within a few minutes (implementation uses a maximum180 seconds; retries
and renames retain the original deadline).

The owner approved a shared30,000-entry aggregate map cap, bounded histories and
an explicit DALI inventory allowance. The map/parser regression cases pass.
Retired State cleanup in BridgeKit is capped at1,024 entries/128KiB and retires
at PUBACK; its 222 SDK/API/example tests pass. Public snapshot structs keep their
existing shape. HDL, CoolMaster and Yamaha compile against the changed SDK, including
test targets, using temporary copies; their source and dependency pins are unchanged.
Two fresh source reviews found a cleanup wake ordering issue, now corrected, and
the added sealed-batch regression passes with two effective mutation controls.
Store uses a run token rather than retaining every locally touched topic, and
retains finite imported-request identity through its original broker expiry.
Review also requires preserving bounded protection for generic legacy requests
without broker expiry, separately from their local handling timeout. The earlier 15 focused history/parser/review cases passed across the main run and
the three-case fix rerun. The current correction/handler-integrity batch passes
24/24, and the targeted core map/profile checks pass 28/28.

Actual producer payloads measure218,322 bytes/10,751 aggregate map entries for a
large4-bus installation (256 outputs,48 input panels), and298,574 bytes/13,147
entries for the dense all-groups case. Both fit the wire/map caps. Real MQTT
dispatch exposed the1-million-operation ceiling; removing repeated group-reference
validation was insufficient. An explicit inventory dispatch profile is now
implemented with 20 million operations for payloads<=512KiB, retaining the existing
memory/call/command bounds. It covers the selected dispatch and nested evaluations,
not just parsing. The first 4-million allowance was insufficient: measured validation alone uses
2.63 million operations plus0.84 million for layout, before tree creation. Updates
now reuse the validated plan rather than repeat validation. The bounded allowance
is20million operations; all eight real-MQTT practical/dense create/update/rename/removal phases pass,
including unchanged replay and a generation-fenced observation after each phase.
No Pi CPU or memory performance claim has been made.

BridgeKit cleanup is accepted locally at `ab8a2ba` after two source reviews and
one confirming review. Its public API is unchanged; all 222 SDK/API/example cases,
strict Clippy, three external consumer builds and seven effective controls are
recorded. DALI alone now pins that public feature revision; its full native suite
passes 868/868 with eight test processes. The unrestricted first run stopped at
810 passes/two startup timeouts; all eight startup cases pass in isolation and
the complete final run passes without changing any deadline. One opt-in browser
case remains separately scoped. Strict production Clippy and all-bin Pi cross-compilation pass for the bump.
The unchanged historical v1 regression harness retains four existing Clippy
warnings when included in an all-bin strict lint; production v1 uses its
maintained tag/branch. Other bridges retain their existing source and dependency pins.

Shared color components/split-address RGB equipment, input actions, opaque
hardware-scene/preset integration and release preparation remain. Preset ordering
stays hardware-scene-first, then individual overrides. The bridge serves generic
MQTT consumers independently of Store. No physical hardware validation or
household migration has occurred.

## P4 — combined emergency operations and retained type capabilities

Accepted locally at `a9acfa5` plus `0438fea` and the test-only `c52ef8b`
correction. DT1 supports explicit status/settings,
function/duration test requests and Stop, with independent emulator profiles,
clock-driven pending/running/results and battery/fault simulation. Ordinary
output readback retains nullable relay/emergency type facts; proved relay rows
use power controls and scene values are labeled as virtual levels.

Final evidence:860/860 native tests across11 binaries (one opt-in browser test
skipped there and run separately),124/124 frontend tests and the complete linked
Chrome phone/desktop workflow189.102s. Native wall time109.0s includes18.58s build
and89.225s test execution. Strict production Clippy8.1s, Pi library/four-bin
`cargo check`6.6s, wait lint1.7s with zero findings, formatting, both embedded
asset checks and all affected control plans pass. Earlier focused checks cover
67 affected native cases and21 storage/authority cases. The final source review
of `96e2059..a9acfa5` found only one P3: embedded HTML was missing from the build
timestamp inputs. The five-file correction at `0438fea` also updates the two
remaining literal peers, the wait registry and deterministic owner-admission
test. Its fresh confirming source review approves with no findings. Both reviews
were source-only/same-family and do not certify runtime or physical behavior.

All10 original controls produced meaningful behavioral RED, one executed test
each, no compiler failures and exact source restoration (231.043s including
clean baselines). Only the changed identification fixture's control was repeated:
baseline GREEN, guard-disabled after-submission case RED, exact restoration
(63.812s). Its4 affected regression checks pass in1.454s. Earlier failed native
runs (856/860 and859/860) remain recorded; only the clean-source860/860 run is
accepted.

The second full native run was859/860, but reused the guard-disabled test binary
whose dependency metadata names `/private/tmp/dali-emergency-mutation`; it is not
accepted as verification of the restored source. Earlier attribution of the
observed guard failure to the scheduling race was premature: the mailbox
barrier removes a real unsynchronized test assumption, while the cache explains
the reproduced deliberate-fault behavior. No production guard was changed.

After the package-only clean removed782,578 files/442.5GiB in146.4s, the rebuilt
guard passes and its dependency record names the real checkout. Dependency
caches remain warm. The full browser run then exposed a test-only lost-reply
race: a changed native field preceded completion of the admitted job. Three
copies now wait for the actual job to finish before hiding its response.
The focused emergency workflow passes in7.636s and the complete workflow passes
in189.102s; no production retry or polling policy was added. Native/frontend
results at `0438fea` carry forward because `c52ef8b` changes only external test
scripts and documentation.

The private demo was refreshed after acceptance. Phone/desktop checks preserve
all5 outputs, exact saved bytes, addresses, levels, actual colors/modes, groups,
scenes, settings and lamp-failure flags. Profile-free identity/data values remain
honestly unavailable. No fixture/profile was injected into the user's demo.
Physical I4/DT1 validation, Store integration and release remain pending.

Workflow changes: one combined family milestone, focused checks between edits,
one final review/regression cycle plus the necessary confirming fix run. Build
metadata watches application inputs including embedded HTML; documentation or
browser-script-only edits no longer relink native tests (measured build reuse
0.11–0.14s). The contributor instructions now require a package-only artifact
clean once after a mutation checkout shares the canonical target. No new UART
recovery policy, emergency poller, journal or worker.

Evidence: `/tmp/dali-native-completion-final-execution.log` and
`/tmp/dali-native-completion-final-native.log` retain the first failed run;
`/tmp/dali-native-completion-final2-execution.log` preserves the contaminated
confirming run. `/tmp/dali-native-completion-final3-execution.log` records the
passing native/frontend suites and the browser fault-injection failure;
`/tmp/dali-native-completion-final4-execution.log` records passing complete browser
and all remaining checks. Demo logs:
`/tmp/dali-native-completion-demo-{preflight,restart,check}.log`. Controls:
`/tmp/dali-emergency-mutation-results.json` and
`/tmp/dali-native-completion-fix-mutation-results.json`.
Spec: `superpowers/specs/2026-10-24-dali-v2-native-completion.md`.

## P4 — Part253 diagnostics and maintenance

Implementation `33a214f` adds strict output-only `read_diagnostics` MQTT Read,
protected jobs and typed reads for banks205/206/207. The existing Memory owner,
permit, original15s deadline, parsed revision/generation and final stamp/reply
checks remain. Dynamic205/206 use the verified freshFF→AA snapshot and same-
lifetime release; static207 uses ordinary reads and works with an unverified HAT
profile. Unsupported versions stop at the prefix. No memory unlock/general write,
new worker, poller, retry, persistent cache or journal was introduced.

All39 fields retain raw evidence, exact values/units and per-field MASK/TMASK,
invalid, unknown and unavailable states. Temperature is unsigned raw minus60°C;
booleans and percentages reject invalid codes. Independent emulator profiles
cover both dynamic bank lengths, frozen/live isolation and static ratings. The
protected dialog offers an explicit bank selector and read, compact fields and
raw details, with lost/retired reply clearing and no automatic read on selection.
Guide: [diagnostics data](diagnostics-data.md).

Final acceptance: **841/841 native tests** across eleven binaries;
**118/118 frontend tests**. Real phone/desktop Chrome passes the focused45.957s
workflow and complete linked176.884s regression. Forty-one focused native cases
cover literal UART and independent TCP, both repeat policies, fresh snapshot
proof/release, static unverified reads, unsupported/absent/truncated layouts,
strict commands/targets and preservation of unrelated native/configuration data.
The new behavior first failed against the stub/missing command. Browser checks
cover all3 banks, reserved values/ranges, missing fields, scrolling, lost/held
replies, explicit retry and retired dialogs.

All eight deliberate controls produce their intended behavioral RED, with no
compile failures, empty filters or survivors (8 test executions,290.604s).
Fresh depth review of `24a3e50..33a214f` APPROVES without findings. Review is
source-only/same-family; root ran the runtime checks. Strict Clippy, Pi library/
four binaries, wait lint0, formatting, both asset reproductions, all affected
control plans and whitespace pass. Native body104.238s/full command245.8s.

The private demo was refreshed at its existing endpoints. Phone/desktop checks
preserve all current fixtures, exact saved configuration and native brightness,
colors, modes, addresses, groups, scenes, settings and lamp-failure flags.
Profile-free fixtures honestly show unavailable diagnostic data; input modules
omit the action. Physical HAT/fixture validation and later Store/release work
remain pending.

Evidence: `/tmp/dali-diagnostics-{managed-green,web-test,browser}.log`,
`/tmp/dali-diagnostics-mutation-results.json`,
`/tmp/dali-diagnostics-final-*.log`,
`/tmp/dali-diagnostics-demo-{preflight,restart,check}.log`.

## P4 — Part252 energy and power

Implementation `6031571` adds strict output-only `read_energy_data` MQTT Read
commands, protected jobs and a checked typed API for banks202/203/204. One
existing Memory Work/owner/permit retains the original15s cap, parsed revision,
generation, physical stamp and final reply checks. A supported version1 prefix
authorizes a verified freshFF→AA snapshot, bounded payload, stableAA and
releasedFF. Cleanup uses the same lifetime and retains the original error and
stream-loss recovery. Unverified repeat profiles refuse before native traffic.
There is no general memory write/unlock, new worker, poller, cache, retry,
journal or renewed cleanup budget.

The decoder preserves exact48/32-bit mantissas and signed decimal scales in
string values, including large and fractional values and genuine zero. AllFF
is invalid; allFF endingFE is temporarily unavailable. Holes/invalid scales and
unknown versions remain explicit evidence. An independent opt-in emulator
implements addressed repeated81/C9 latch behavior with runtime frozen bytes
separate from persistent profiles; its ready-to-use scenario covers all3 banks.
The compact protected dialog has an explicit bank selector/read action, exact
values/units, snapshot proof and raw details. Failed/new reads clear previous
results while keeping keyed read controls open. Guide: [energy data](energy-data.md).

Final acceptance: **831/831 native tests** across eleven binaries;
**110/110 frontend tests**; scoped real Chrome phone/desktop47.299s and complete
linked workflow179.174s (browser separate from native suite). Thirty-one focused
cases cover exact decoding, independent latch/frozen/live behavior, both HAT
terminal policies, strict MQTT params/targets/read class, literal UART fresh
transition/holes/collision/release, original queued revision and unverified-profile
refusal before traffic. Browser checks cover all3 banks, exact large/fractional
values, zero/TMASK/absent/truncated/unknown data, no read on selection, lost/held
replies, explicit retry, retired dialogs and unchanged native/configuration data.

Eight deliberate controls each produced its intended behavioral RED, with no
compile failures, empty filters or survivors (24 test executions,247.920s).
Fresh depth review of `647109a..6031571` APPROVES without findings. These are
same-family/source-only reviews; root ran the runtime checks. Strict Clippy,
Pi library/four binaries, wait lint0, changed formatting, both asset
reproductions, all affected control plans and whitespace pass.

The private demo was refreshed at the same endpoints. Its profile-free fixtures
honestly show unavailable energy data; input modules omit the action. The
phone/desktop check preserves all five fixtures, exact saved bytes, native
levels/colors/modes/addresses/groups/scenes/settings and lamp-failure flags.
The dynamic progress pane remains live. Physical HAT/fixture validation, Store
integration, main merge, push/publication and release remain pending.

Evidence: `/tmp/dali-energy-{literal-green2,web-test,browser2}.log`,
`/tmp/dali-energy-mutation-results.json`, `/tmp/dali-energy-final-*.log`,
`/tmp/dali-energy-demo-{preflight,restart,check}.log`,
`/tmp/dali-browser-db59f6bedc801882e3f90c115e8ac031` (complete workflows).

## P4 — Part251 luminaire data

Implementation `4976bff` and GTIN boundary regression `7938bab` add strict
output-only `read_luminaire_data` MQTT Read commands, authenticated jobs and
typed service API. The same Memory Work/owner/permit reads bank1's bounded
prefix, proves content format0003, then extends through min(header,0x77) under
the original15s cap, parsed revision/generation and final stamp/reply checks.
Unknown formats preserve only the prefix; public raw reads stay1–64 bytes.
Identity/raw reports omit the new optional luminaire field. No write, poller,
cache, worker, retry or configuration/observation change.

The pure decoder returns fourteen OEM identity/rating/text fields with explicit
value/unknown/unavailable/invalid statuses and units. GTIN and64-bit luminaire
ID remain exact strings; CCTFFFE is a Part209 indication, never65534K or inferred
DT8 capability. DistributionFF is unknown and6–254 are invalid. ASCII/NUL text
retains holes before termination and is escaped in the dialog; it never changes
saved name/Description/Origin. An independent raw-bank scenario configures the
emulator without a new model. Guide: [luminaire data](luminaire-data.md).

Final acceptance: **819/819 native tests** across eleven binaries;
**104/104 frontend tests**; scoped real Chrome phone/desktop17.291s and complete
linked workflow166.550s (browser separate from native suite). Nineteen focused
memory/luminaire cases cover literalC3/A3/C5 framing, offsets/silence/collision
positions, format/header bounds, generic MQTT/read class, strict targets/params,
original prepared revision and unchanged native settings/levels/colors/groups/
scenes/input data/configuration/caches. Browser checks cover exact IDs/units,
partial/variable-CCT data, safe OEM markup,120-byte scrolling, unknown prefixes,
lost reply without retry, explicit refresh and retired dialogs on phone/desktop.

Eight initial deliberate controls (seven new plus the corrected existing input
feedback/group-cache anchor) each produced their intended behavioral RED. An
extra oversized-GTIN mutation survived the original tests; a literal maximum/
overflow regression first failed against that defect and then passed with the
current implementation. No production fix was needed. The eighth luminaire
control now records that boundary. Fresh depth review of `5572656..7938bab`
APPROVES without findings. Reviews are same-family/source-only; root ran runtime
checks. Strict Clippy, Pi library/four binaries, wait lint0, changed formatting,
both asset reproductions, all affected control plans and whitespace pass.

The existing private demo was refreshed at the same endpoints. Its profile-free
fixtures honestly show unavailable OEM data, and input modules omit this action.
Phone/desktop checks retain all five outputs, exact saved bytes, levels/colors/
modes/addresses/groups/scenes/settings and lamp-failure flags. The dynamic progress
pane remains live. Physical HAT/fixture validation, Store integration, main merge,
push/publication and release remain pending.

Evidence: `/tmp/dali-luminaire-{red,codec,service-red,native,browser2,web-test}.log`,
`/tmp/dali-luminaire-gtin-{red,green}.log`,
`/tmp/dali-luminaire-mutation-results.json`, `/tmp/dali-luminaire-final-*.log`,
`/tmp/dali-luminaire-demo-{preflight,restart,check}.log`.

## P4 — device identity and read-only memory

Implementation `41c2b0f`, wait registration `a335ccd` and admission fix `9a466c7` add
strict `read_identity`/`read_memory` MQTT Read commands, authenticated jobs and typed
service APIs. One existing Memory Work/owner/permit selects DTR1 and reloads DTR0
per location under a15s original deadline. Original generation, parsed command
revision, final physical stamp and caller-reply checks remain. Reads preserve
saved names/Origins/configuration, native settings/levels/colors/groups/scenes
and existing output/group/input/feedback observations. No worker/cache/retry or
memory write. Output and input short-address spaces remain independent.

Raw FF/zero, silence, collision directions/codes and unvisited locations remain
distinct. Public ranges stay1–64 bytes within locations0–254, banks0–255.
Bank0 uses observed version1's 32-bit legacy serial or modern8–254's 64-bit layout;
unknown versions read common bytes and never guess serial/hardware. Serial is
fixed-width uppercase hex; GTIN is a padded decimal string. Truncated/reserved
headers and all-FF multi-byte fields remain explicit unknown/partial evidence.
Optional independent emulator profiles preserve old scenarios and capture/reset/
reboot behavior. The existing scenario editor configures raw bytes and holes.
Guide: [device information](device-information.md).

The compact protected Device information dialog supports identity refresh and
advanced bounded raw reads on output/input-module action menus. Frozen scopes,
strict report decoding and synchronous retirement prevent old results populating
another item. Failed/new invalid reads clear previous evidence. Input instances
do not offer the module action. Phone/desktop layout and menu/dialog scrolling
use the existing interface.

Final acceptance: **811/811 native tests** across eleven binaries;
**99/99 frontend tests**; scoped real Chrome phone/desktop workflow9.913s and complete linked workflow154.708s
(browser run separately from the native suite). Eleven focused memory cases cover
both terminal policies, literal nonzero-offset UART/collisions, independent TCP
profiles, exact serials, unknown/truncated/reserved/absent identity, strict generic
MQTT/HTTP reads, preserved state/configuration and original revision/deadline/epoch.
Six deliberate controls and an extra physical-epoch defect each returned their
named RED. The queued-job test first failed against the original implementation;
both identity/raw reads reject stale admission and fresh reads succeed after fix.

Fresh depth review found one P2: queued protected jobs renewed the inventory
revision despite a continuing fixture generation. Parse now carries revision into
the common service; existing final checks remain. The scoped confirming review
of `9a466c7` APPROVES. Reviews are same-family/source-only; root ran runtime checks.
Strict Clippy, Pi library/four binaries, wait lint0, changed formatting, both asset
reproductions, affected control plans and whitespace pass.

The private demo was rebuilt/restarted at its existing endpoints. Phone/desktop
fixture/module reads keep absent profile data unknown. All five fixtures, exact
saved bytes, brightness, colors/modes, addresses, groups, scenes, settings and
faults are retained. Demo processes and read-only progress pane remain live.
No physical HAT, home broker, Store data, main merge, push/publication or release.

Evidence: `/tmp/dali-memory-{focused,fixed-focused,prepared-red,browser2,web-test}.log`,
`/tmp/dali-memory-final-*.log`, `/tmp/dali-memory-mutation-results.json`,
`/tmp/dali-memory-mutation-*.log`, `/tmp/dali-memory-plans-fixed.log`,
`/tmp/dali-memory-demo-{preflight,restart,check}.log` and
`/tmp/dali-browser-9ecc8e73ca7b7e0c7d41e400657d3633` (complete workflows).

## P4 — DT8 hardware group color

Implementation `b84f68c` and confirmation fix `80f30df` add optional saved-group
temperature/xy/RGBWAF selection with independent consumer/identity continuity,
strict selected-family Desired and read/set commands, all-actual-known-member
preflight, native group staging/ACTIVATE and fresh individual readback. Base and
color availability are independent. Uniform, mixed and unknown scalars have
explicit values/mixed flags; common capabilities and differing temperature limits
come from hardware evidence. Group command reports remain membership/base
envelopes; State and typed native samples carry color. Existing owner, deadline,
stamp, generation and revision acceptance remain. No worker, poller or retry added.

Groups now has a compact Color card, common-format selector and value dialog.
Explicit inspection works before selection. Drafts survive same-generation SSE
refresh; generation changes retire them. Partial edits preserve untouched values,
unsupported members disable editing and Blink blocks same-bus color changes.
Guides: [group color](group-color.md), [group operations](group-output.md) and
[MQTT](mqtt-outputs.md#hardware-groups).

Final acceptance: **801/801 native tests** across eleven binaries, with the opt-in
browser run separately; **93/93 frontend tests**; complete linked real Chrome
phone/desktop workflow144.055s and scoped group-color workflow18.223s. Scoped
native cases cover both HAT terminal policies, mixed/stale/null values, actual-vs-
saved membership, missing/unsupported members, xy/RGBWAF masks, outsiders, preserved
brightness and independently clamped temperatures. Six deliberate defects
returned named REDs in an isolated checkout, including the shared RGBWAF mode
confirmation predicate. That bounded probe found and fixed a mismatch between
typed confirmation and unsupported-mode State, covering both individual and group
writes. The new regression and 85 affected color/Blink tests pass.

Strict scoped Clippy, Pi library/four binaries, normal wait lint, formatting,
both embedded asset reproductions, affected control plans and whitespace pass.
Clippy's only finding was a redundant test-local binding; its one-line cleanup
was self-reviewed and all five affected independent-consumer tests passed again.
Production source at80f30df received fresh GPT-6 Astra/high APPROVE after mutation
verification. Same-family/source-only review limits remain; root ran runtime checks.

The private demo was rebuilt/restarted at its existing endpoints. Phone/desktop
inspection of the new Color card preserves all five user fixtures, exact saved
bridge bytes, levels, active colors/modes, addresses, groups, scenes, settings and
fault flags. Both demo processes and the read-only progress dashboard remain live.
No home broker, physical HAT, Store data, main branch, remote publication or release
was changed. Physical validation and the remaining P4/P8/P9 families remain open.

Evidence: `/tmp/dali-group-color-final-{native,browser,clippy,pi,waits}.log`,
`/tmp/dali-group-color-final-{execution,remaining}.log`,
`/tmp/dali-group-color-mutation-{color,output,scalar,probe}.log`,
`/tmp/dali-group-color-confirmation-control.log`,
`/tmp/dali-group-color-clippy-test-fix.log`,
`/tmp/dali-group-color-plans-final.log`, `/tmp/dali-group-color-web-test.log`,
`/tmp/dali-browser-9cdcfd1468365a0941dde4f93f1a784b` (group color) and
`/tmp/dali-browser-0d34ff6e15183a4bb0864e6831b35f63` (complete workflows).

## P4 — fixture status and simulated lamp failures

Implementation `cafc74e` adds all eight read-only fixture flags plus
FixtureStatusRaw/FixtureStatusQuality to individual MQTT State. Status freshness
is independent of brightness; stale/missing/retired evidence explicitly clears
raw and flags. Existing polling, output ownership, trace context and v1 behavior
remain. No fault is inferred from illumination or requested level.

Output rows show compact red/amber fault icons or neutral unknown status beside
the inline name. The read-only dialog follows current SSE values and permits an
explicit normal read. All-clear rows use no extra icon. The independent emulator
supports live and initial lamp_failure checkboxes, strict output-ID control and
optional default-false version1 scenario data. Native status adds only the lamp
failure bit; level/settings, other fixtures and the bridge configuration stay
unchanged. Capture/export/reboot retain faults; Reset uses the saved scenario.
Guides: [MQTT](mqtt-outputs.md), [fixture status](gear-settings.md#fixture-status),
[emulator](emulator-web.md#simulate-a-lamp-failure).

Final acceptance: **791/791 native tests** across eleven binaries (the opt-in
browser is run separately); **91/91 frontend tests**; complete linked real Chrome
phone/desktop acceptance146.415s; standalone emulator phone/desktop workflows21.3s.
Four deliberate defects returned named REDs and restored source. Strict Clippy,
Pi library/four binaries, normal wait0, changed formatting, both committed-asset
reproductions, affected control plans and whitespace pass. Fresh design review
and final source review of frozen cafc74e both APPROVE; same-family/source-only
review limits remain. Root ran the runtime checks.

The visible private demo was rebuilt/restarted with all five user fixtures and
saved bridge bytes retained. Root exercised Light1 lamp-failure set/read/details/
clear on phone and desktop, restored all original fault flags and verified
unchanged native brightness, active colors, addresses, groups and scenes. Both
processes remain running. Re-read all saved outputs after restart for current UI
values; no new automatic poller or configuration mutation was added.

Evidence: `/tmp/dali-fixture-status-{red,focused,controls,browser-confirmed}.log`,
`/tmp/dali-fixture-status-final-*.log` and
`/tmp/dali-fixture-status-demo-{preflight,restart,check}.log`.
The first corrected RED failed because LampFailure was absent. Browser setup
errors were corrected to use live Inventory authority and await confirmed
checkbox snapshots. No physical HAT, live Store/home broker, SDK publication,
release tag, push or main merge. Specialist/data/emergency diagnostics and other
remaining capabilities stay tracked in [the support matrix](support-matrix.md).
Spec: [fixture status](superpowers/specs/2026-10-23-dali-v2-fixture-status.md).

## P2c — integrated configurable emulator and linked browser tabs

Integrated parallel implementation `d28a5d7` as `a60adbe`, with result handoff
`06e76d4`. Standalone emulator now retains the installation across replaceable
raw TCP HAT clients, serves its own optional loopback UI, edits/imports/exports/
captures/resets strict scenarios and generates semantic button/sensor events.
PTY/control-socket compatibility and native settings/colour persistence remain.
Operator guide: [emulator UI](emulator-web.md).

Root's new managed test drives addressed discovery, rename, actual brightness,
indicator Desired and native semantic input actions through the actual listener
under both terminal profiles. The opt-in browser fixture accepts
`DALI_BROWSER_HAT_TCP=1`; the default PTY path is preserved. Its second real tab
presses the emulator's button and observes bridge state without losing a name
draft, while a bridge power change appears in the emulator's native level.

Combined full native acceptance passes **778/778** across eleven binaries, plus
the explicit full real phone/desktop browser test (218.235s, linked tabs; no bridge-page runtime or CSP errors).
The emulator tab has separate functional assertions; this combined test does
not attach its console/error collector to that tab. Browser artifacts:
`/tmp/dali-browser-121fd67d2cc2c6c2655bbca87cdb3e42/`.
The inactive-stimulus negative control returns its named RED; restored native
acceptance passes both profiles. Strict Clippy, Pi library/four binaries,
TypeScript/assets reproduction, normal wait0, affected control plans, formatting
and whitespace pass. Frozen integration b0e6890 receives APPROVE from a fresh
same-family/source-only review. Rebuilding with locally installed dependencies
normalizes three generated dependency-path comments; behavior is unchanged. No physical HAT, live Store, publication, deployment or main merge.
An explicitly requested visible demo runs separately with its own broker/config;
its disposable process details are in `/tmp/dali-visible-current.json`.

## P4i8 — input feedback scalar bindings

Accepted implementation `1cb2055` with compatibility correction `1cb11ea`.
Existing module/instance targets accept
boolean FeedbackActive Desired values and publish independent feedback evidence
alongside measurements. Native/typed/web command report shapes and configuration
files stay compatible. The owner caches the exact accepted feedback Arc, retires
it on reads/writes, revisions and physical/address changes, and keeps input
Notices/values independent. Whole State rows use the latest owner-accepted input
or feedback cause; same-value observations update provenance without forcing
duplicate MQTT values. Final scalar acceptance checks the exact capture and
requested actual value, rather than transport acknowledgement.

Target refresh reads instance value then feedback, or module feedback alone.
Aggregate refresh keeps its fifteen-second fair cursor and polls only previously
sampled feedback scopes. Focused evidence includes scalar/atomic Desired, strict
maps/authority, module/instance isolation, unique HTTP rows, unknown/unsupported/
Off distinction, exact capture/deadline, native clear/revision invalidation,
unchanged/coalesced trace ordering, sampled-only All and ignored native writes.
The original behavioral test failed because no feedback State was published.
Final full native acceptance passes **765/765** across eleven binaries (one
opt-in browser skipped). Six new deliberate faults and both reanchored existing
faults return their named REDs. Final strict Clippy, Pi library/four binaries,
changed formatting, affected control plans, wait validation0findings, embedded
asset reproduction and whitespace pass. Both sequential fresh source reviews
APPROVE; the confirming correction review also APPROVES. Same-family/source-only
review limits remain; root ran all runtime checks. No bridge UI/assets changed,
so this phase claims no fresh browser run.

The first full run caught an unintended sorted projection order and old tests
expecting measurement-only HTTP fields/query counts. The fix preserves existing
output/input/group order and adds module rows, explicit unsupported feature
responses and exact updated HTTP/count assertions. Its targeted17 cases and
corrected count case pass; the final full run includes all of them.
Logs: `/tmp/dali-feedback-binding-{native-final,clippy-final,pi-final}.log`,
`/tmp/dali-feedback-binding-controls.log`, `-wide-control.log`, `-group-control.log`,
`-plans-final.log`, `-waits-final.log`, `-assets.log` and `-delta.log`.
Native parameter settings, broader feedback profiles and physical certification
remain separate work.
Spec: [feedback bindings](superpowers/specs/2026-10-23-dali-v2-feedback-bindings.md).

The parallel emulator implementation and handoff were subsequently integrated;
the P2c checkpoint above records root-owned combined acceptance. The original
agent also reported653 library/nine standalone tests and phone/desktop UI checks;
those are separate from the root integration evidence.

## P2b — bridge TCP connector and emulator UI handoff

Implementation `f3912a1`. The user requested a separate emulator process with
its own configuration/operation UI, emulating the HAT over TCP. The bridge keeps
its existing web UI and physical UART support. The parallel-session scope and
file ownership are in [the handoff](handoffs/2026-10-23-emulator-ui.md); no emulator
implementation session was launched by this work.

The bridge now accepts `--hat-tcp IP:port` instead of an explicit `--uart`.
It uses the same ATX monitor/transactions and service owner through a concrete
UART/TCP connection adapter. Numeric IPv4/bracketed IPv6, nonzero port and
UART/TCP exclusivity validate before startup. Async connect is bounded to two
seconds and retryable; UART blocking-open timeout and Boot policy are preserved.
A new TCP connection requires a fresh remote parser/output lifetime and clears
inherited parser poison. Input events, stamps, command deadlines and capability
executors retain their existing shared path. No saved schema or MQTT changes.

Fail-first process evidence showed the missing TCP connection. Twelve new tests
cover actual raw process IO, kernel drain/EOF, pending reader wakeup, async
timeout/cancel ownership, old UART/new TCP recovery arms, literal fragmented
replies with input events, active-query disconnect, stale-action retirement,
reconnect, retained fixture state and actual close. The independent emulator
core runs over TCP, programs minimum_level2 and reads it back under both terminal
profiles, alongside output and feedback operations. Refused endpoints leave
metadata and bounded stop available.

The full native run selected **753** tests across eleven binaries: **752 passed**,
with only a missing wait-registry annotation failing; the opt-in browser was
skipped. The annotation/row/generated waiter were corrected without changing
production behavior. Final targeted **13/13**, including the registry and all
twelve new tests, pass (1.841s execution). This is not a second full753-test run.
Final strict Clippy, Pi library/four binaries, changed formatting, affected control
plans, wait validation0findings, embedded-asset byte-check and whitespace pass.
Final restored gate: **0 of 8 failed**. No bridge UI/asset changes, so no new
browser/Node run is claimed.

Eight current deliberate faults and the refreshed old UART timeout fault return
their named REDs. Initial force-OneTerminal mutation was GREEN; the final control
forces TwoTerminals against a one-terminal peer, proving missing-terminal
detection without claiming rejection of every surplus-terminal mismatch.
Both sequential source reviewers APPROVE, including the final comment/test
extensions. Source-only, same-family reviews; token metrics unavailable.

No emulator implementation files, physical HAT, live Store/broker, shared SDK,
release, publication, deployment, push or merge changes. Emulator TCP listener,
model lifetime across connections, scenario editor and its own web UI remain
the parallel session's task. Spec/acceptance:
[TCP HAT connector](superpowers/specs/2026-10-23-dali-v2-tcp-hat.md).
Logs: `/tmp/dali-tcp-{acceptance,restored}/`,
`/tmp/dali-tcp-{controls,repeat-control-final,uart-control}.log`.

## P4f3 — virtual DT7 scene capture

Frozen implementation `b731f96`. Combined design gate and both sequential final
native/contract and regression/compatibility source gates APPROVE without blockers.
Reviews are source-only, same-family; their token usage is unavailable on this host.

Capture now distinguishes relay virtual control from actual contact0/254. It
conservatively classifies the existing correlated type replies, poisons DTR0 with
FF, sends addressed repeated STORE ACTUAL and queries DTR0 under the existing
scene permit/stamp/deadline. Only numeric0..254 can become a reviewed candidate.
Unknown type, partial base, missing/collided/FF data never become contact fallback.
Public actual State stays binary. No new command, schema, owner, inventory write,
polling or SDK/Store coupling. Ordinary dimmer/DT8 capture keeps Unverified-profile
support; positive DT7 requires the known repeat profile. Only harmless DTR scratch
poison can precede unsupported-repeat refusal. Capture does not operate the contact
or program a scene; applying its frozen reviewed candidate stores the virtual level.

Behavioral RED returned254 instead of100 through the real managed MQTT/emulator
path. Nine focused native cases pass in4.015s, including both repeat profiles,
exact literal poison/STORE/query, zero/254, malformed multi-type tails, unknown
colour/base evidence, failed Send and query outcomes, stale final rejection,
binary cache/inventory preservation and reviewed-candidate programming/recall.
The existing unverified RGBWAF capture regression remains green. Strict peers
check worker liveness as well as script completion, so detached task panics cannot
hide unexpected native traffic behind an expected operation error.

All **11 deliberate faults** produce their named REDs,199.9s latest-row sum.
Source is restored. Full native acceptance passes **741/741** across eleven
binaries (79.078s execution,147.1s command; one opt-in browser skipped). Strict
Clippy16.2s, Pi library/four binaries2.5s, changed Rust formatting, wait validation
0findings, generated asset byte-check and whitespace pass. Repository control
plans found one old candidate-construction anchor; it is retargeted to the new
line, and the unchanged ordinary RGBWAF oracle detects that fault in21.3s.
The corrected plan passes; restored focused verification passes8/8 in4.036s after
that mutation.
No production/test assertion change was needed. Other repository plans passed.

No web source/assets changed: the previous1177-command phone/desktop acceptance
and Node89 remain applicable, not claimed as fresh browser execution. Scene guides
explain the current Brightness/Off labels as virtual control for DT7; relay-specific
presentation remains alongside retained classification. Recall evaluates current
threshold/direction, so a captured virtual value need not reproduce an earlier
contact state after other changes. Operator emulator instructions now send socket
JSON immediately with a pipe, respecting the one-second idle deadline, and explain
independent snapshots versus bridge-observed state and the absent simulator GUI.

Methodology: sample contactOn/Off independently from virtual level; verify strict
peer task health when a caller expects errors; refresh changed old fault anchors
at the same boundary. No live HAT/Store, SDK, memory/skill, release, publication,
deployment or push/merge changes. Retained relay metadata, presentation, timed
fades/fault profiles and physical validation remain pending.
Spec: [DT7 capture](superpowers/specs/2026-10-23-dali-v2-dt7-scene-capture.md).
Evidence: `/tmp/dali-dt7-capture-{red,focused-final,controls,send-control,acceptance,affected-control,restored,affected-plan}.log`
and `/tmp/dali-dt7-capture-acceptance/`.

## P4f2 — managed DT7 thresholds and virtual limits

Frozen implementation `e3db774`. Sequential final native/contract and UI/state
source reviews APPROVE after the corrections below. Reviews are source-only and
same-family; the host does not expose their token usage.

The existing typed/MQTT/protected HTTP fixture settings path now carries strict
optional switching patches and fresh explicit relay capability/raw replies.
The protected dialog edits all four adjustable thresholds, including disabled255
and downward0. Fixed/unknown thresholds remain read-only. Positive Type7 and
adjustable-feature evidence permit virtual minimum1 below physical254, with real
physical evidence still required. Common and relay quality remain independent;
the whole mixed patch is preflighted before writes and every requested native
value is compared afterward. Existing ownership, authority and inventory shapes
are reused. Retained relay component metadata remains pending.

Behavioral REDs proved the rejected nested patch and physical254 floor. A further
literal RED caught missing physical evidence incorrectly marked complete. Eighteen
focused native cases pass, including both repeat profiles, malformed multi-type
prefixes, partial requested/unrequested families and successive virtual limits.
The final UI review found a real invalid → Disabled → enabled defect: blank text
submitted a hidden minimum1. The real browser reproduced it; explicit invalid
text now survives that transition. Blank and300 submit no job/native change;
corrected55/56 persist through native readback. The shared renderer covers all
four threshold fields.

Restored native acceptance passes **734/734** across eleven binaries (76.262s
execution,146.4s command; one opt-in browser test skipped). Strict Clippy16.6s,
Pi library/four binaries4.0s, changed Rust formatting, wait validation0findings,
Node89, generated asset byte-check and affected control plans pass. After the
UI-only correction, Node89, assets, Pi1.5s, native protected-assets1/1, whitespace
and every repository control-table plan pass again. Backend source is unchanged.

Final protected phone/desktop workflow passes **1177 real owned commands** in
108.65s on the frozen implementation. Fresh screenshots were inspected: one
relay column on phone, two on desktop, readable scrolling diagnostics. A focused
mode executes the same relay workflow/login/private stack in15.93s (164commands)
for regression checks. The sequential browser harness explicitly disables All
polling; this does not certify operator/poll overlap. A confirmed-recovery
completion barrier fixes the observed stale-enabled-button test race. Earlier
unrelated Busy runs are not counted as acceptance evidence.

All **17 current deliberate faults** produce their named REDs (243.9s latest-row
sum), plus both affected common-limit controls. An initial blur-only mutation
was GREEN because clicking the checkbox commits the text on blur; it remains
historical, replaced by native draft-loss with the same independent snapshot
assertion. Source/assets were restored before freeze. Native review also found
two old generic Off guarantees; both guides now qualify ordinary dimmers.

Methodology: test absent versus explicit invalid drafts through neighboring mode
changes, missing diagnostics beside capability exceptions, and zero/one/two
types around a terminating sentinel. No live HAT/Store, SDK, memory/skill,
release, publication, deployment or push/merge changes. Virtual scene capture,
retained classification, timed fades/fault profiles and physical validation remain
pending. Spec: [managed DT7 settings](superpowers/specs/2026-10-23-dali-v2-managed-dt7-settings.md).
Evidence: `/tmp/dali-managed-dt7-{acceptance,postfix,browser-final,toggle-red,toggle-green,toggle-controls}.log`,
`/tmp/dali-managed-dt7-acceptance/`, `/tmp/dali-managed-dt7-postfix/` and
`/tmp/dali-browser-5139cc4985f2ae73a63f30692a082773`.

## P4f1 — DT7 switching protocol and independent relay foundation

Frozen source `f3b2e30`. Combined design and both sequential final codec/native
and model/compatibility source gates APPROVE without findings. Reviews are
source-only, same-family; this host does not expose their token usage.

Checked Part208 plans preserve threshold255 as disabled, raw unknown diagnostics,
and one enable followed by both copies of a repeated setter. The independent
optional DT7 profile separates virtual control from actual binary0/254. Directional
threshold comparisons, reversal, equal-threshold On priority, fixed/adjustable
limits, virtual STORE ACTUAL, nonactivating programming and RESET retaining raw
status evidence have literal coverage. Addressed interruption reaches per-gear
repeat authority before existing base-repeat early returns. Non-DT7 serialized
shapes and dimmer/colour/input behavior remain unchanged.

Behavioral RED initially returned actual100 rather than254. Fifteen focused
codec/model/scenario/managed/standalone-PTY cases now pass (0.602s), including both
HAT repeat profiles. One early test sent STORE ACTUAL only once; pinned primary
source confirms it requires repetition. Correcting that test required no production
workaround. All **16 deliberate faults** now produce their named REDs (261.5s
latest-row sum). The first batch had one GREEN because it altered private contact
state without the canonical actual-level path; the corrected fault drives that
path and the unchanged native-readback assertion detects it. Source was restored
before freezing and review.

Full native acceptance passes **727/727** across eleven binaries (74.424s execution,
134.1s command; one opt-in browser test skipped). Strict Clippy10.96s, Pi
library/four binaries4.0s, changed Rust formatting, independent wait validation,
generated asset byte-check, whitespace and all eleven affected control-table
plans pass. No browser source/assets changed; the prior1013-command phone/desktop
acceptance remains applicable, rather than being claimed as a new browser run.

This is a protocol/model foundation. Managed relay settings, truthful MQTT
capability metadata/browser controls, virtual scene capture, timed fades, fault
profiles and physical validation remain pending. No live HAT/Store, SDK,
memory/skill, release, publication, deployment or push/merge changes.
Methodology: enumerate actual/virtual consumers and vary direction independently
of threshold crossing, and RESET independently of default settings.
Guide: [DT7 switching](dali-switching.md). Spec:
[DT7 foundation](superpowers/specs/2026-10-23-dali-v2-dt7-foundation.md).
Evidence: `/tmp/dali-dt7-{red,integration,clippy,controls,nonactivation-control}.log`,
`/tmp/dali-dt7-acceptance.log` and `/tmp/dali-dt7-acceptance/`.

## P7 follow-up — input settings retain numeric edits

Frozen source `0b2061a`. Both sequential final source-only UI/state and browser/
regression reviews APPROVE, with no code findings. Same-family limitation remains.
The regression reader identified a documentation precision issue: independent
whole-profile snapshots verify edited states; restoration uses the existing
confirmed save/readback path. The browser guide now distinguishes those stages.

A real browser repro entered1000ms before changing Event priority, then saved;
the independent model retained500ms (raw25 instead of50). A blur-first variation
passed. The form now retains timer/sensor strings on input and separately parses
the native draft; functional nested updates preserve neighboring settings. An
invalid intermediate occupancy hold remains editable but cannot submit a mixed
hardware patch. Refresh clears both text and native draft. Backend, protocol,
scope/cancellation, availability and save/readback behavior are unchanged.

The strengthened phone/desktop workflow passes **1013 actual owned commands**,
91.10s, including all button timers/all three sensor families, number-then-select
edits, invalid hold/no-job/no-native-change, reopening, restoration and unchanged
saved inventory. Fresh button/occupancy screenshots were inspected; dialogs remain
scrollable and readable. All **85 Node cases**, TypeScript/build, restored asset
byte-check, native protected-assets test1/1 (11.02s command), Pi library/four
binaries1.76s, whitespace and the affected control plan pass. No Rust backend
source changed, so the previous broad native/Clippy acceptance remains applicable.

Both deliberate browser faults produce their named REDs: blur-only timer24.5s,
normalized intermediate sensor text27.6s. Source and generated assets were restored
before freeze/review. One initial expanded test failed because the existing UI
displays the same validation error in two dialog locations; the test now checks
all displayed messages and independently proves no command/native change, without
changing production error handling. A separate pre-repro run stopped at an existing
scene Busy and is not counted as input-regression evidence.

Methodology: vary control order before blur and distinguish which stages have
independent native snapshots from stages verified through service readback.
No live HAT/Store, SDK, memory/skill, release, deployment or push/merge changes.
Spec: [input draft retention](superpowers/specs/2026-10-23-dali-v2-input-draft-retention.md).
Logs: `/tmp/dali-input-edit-{early-red,green2,node,build,dist,controls}.log` and
`/tmp/dali-input-edit-acceptance/`; browser artifacts:
`/tmp/dali-browser-87c27f8a0278df1fd9688426eeb4517f`.

## P4b8 — managed common fixture settings and protected controls

Frozen implementation `51e4613`; local wait-method declaration `906a380`.
Final sequential native/owner and web/API source gates APPROVE without blockers.
All reviews are read-only/source-only and same-family. No live HAT/Store,
shared SDK, publication, deployment, push/merge or memory/skill changes.

Individual fixtures now support `read_output_settings` and `configure_output`
through typed service APIs, strict generic MQTT commands and protected HTTP jobs.
The compact Fixture settings dialog covers minimum/maximum brightness, power-on
and bus-failure levels, basic fade time and dimming rate, plus read-only physical
minimum, actual level, raw version and status flags. Unavailable families remain
unknown; readable families remain usable. Reserved fade-rate0 can be repaired.
Native Off/LastActive/Unchanged meanings remain distinct, and basic time0 does
not promise an instant transition. No settings cache or automatic poller.

One original authority/deadline/permit spans fresh whole-patch preflight, checked
native plans and actual readback. Crossed limits are ordered so the fixture can
accept both changes. Active Blink blocks configuring the same continuing fixture
at submission and owner admission. Possible writes retire bus-local output,
colour and group evidence before enqueue and at settlement; unrelated buses,
input values and native Notices survive. A HAT terminal never confirms settings.
Missing/mismatched evidence returns unconfirmed and requires explicit refresh.
The independent model gained only its missing literal PhysicalMinimum reply;
scenario/snapshot formats and timing behavior are unchanged.

Fail-first native and UI oracles caught inert implementations; a literal model
case caught the missing physical-minimum query. Ten new native cases pass,
covering all six fields/both repeat profiles, exact crossing order, no-write
preflight, partial/masked/colliding evidence, original authority/deadline/epoch,
both identification guards and strict Read-class MQTT dispatch. Group cache
neighbors also pass. All **85 Node cases** pass.

The initial full native run passed **711/712** across eleven binaries (74.477s
execution,147.1s command; one opt-in browser test skipped). Its sole failure was
the new local helper missing from the wait registry's `local-methods` declaration.
All three non-self receivers are the bridge's own ServiceHandle. After that
one-line declaration correction, the failed native test passes and independent
wait validation reports zero findings; no production change or broad rerun was
needed. Strict Clippy11.60s, Pi library/four binaries4.3s, changed formatting,
web assets/whitespace and all41 affected control-table plans pass.

All **20 intentional faults** now produce their named REDs (279.3s latest-row
sum):13 native,6 Node and1 real-browser numeric-edit fault. The first batch had
one wrong-reason result: the crossed-limit assertion failed at actual confirmation,
so its unchanged equality gained a precise message and that row was remeasured
RED. Mutations and generated assets were restored before freeze/readers.

Actual phone/desktop acceptance passes **1013 owned commands**,92.87s. It verifies
all six saved native settings, both crossing orders, Off retention, special255,
basic time0, reopen/restore, lost-reply refresh, dismissal/navigation and unchanged
inventory. Both settings screenshots were inspected. An early browser run failed
an existing scene Busy; another run reached the new form and exposed lost numeric
input when a dropdown rerendered it. A functional state update alone did not fix
that DOM behavior. Typed text now updates on input, including intermediate values,
with functional updates and explicit startup modes; native assertions are unchanged.
Methodology: vary edit order across number/select controls and assert physical
values, rather than relying on a blur event or the form's displayed text.

Guide: [fixture settings](gear-settings.md). Evidence:
`/tmp/dali-managed-gear-{settings-final-cases,node-final,clippy,controls,crossing-control,browser4,wait-regression,wait-validation}.log`,
`/tmp/dali-managed-gear-acceptance.log` and its log directory. Browser artifacts:
`/tmp/dali-browser-9dddbd689dbec39e911f4889988cb51a`.
Remaining extended/specialist settings and diagnostics remain explicitly pending.

## P4b7 — common fixture settings protocol foundation

Frozen source `192c7b7`. Combined design and sequential final codec/contract and
independent-model/compatibility source gates APPROVE, with no blockers. Reviews
are source-only/read-only and same-family. No live hardware, Store, shared SDK,
deployment, publication, push/merge or memory/skill changes are included.

Checked `dali::gear::settings` values/plans cover minimum/maximum, power-on and
bus-failure levels and basic fade time/rate; existing correlated Query gains
PowerOnLevel/SystemFailureLevel/FadeTimeRate. Special255 meanings remain distinct:
startup LastActive, failure Unchanged, packed fade numeric15/15, actual MASK.
Rate0 remains raw unknown; basic time0 does not promise instantaneous output.
DTR0 is followed by exactly one addressed repeated setter plan. This is a pure
foundation, with no managed settings command/UI or runtime ownership change.

Two focused runtime oracles first failed on inert timing/plan implementations;
three initial value/query cases already passed. All22 focused codec/model/PTY
cases now pass (0.722s execution), including all six typed settings through the
standalone HAT process and actual correlated readback. Three new literal model
tests verify already-existing behavior: adjacent repeat/100ms boundary, scopes,
clamping with Off preserved, independent255 values and retained fade nibbles.
Production model logic and scenario/snapshot shapes are unchanged.

All nine intentional faults produce their named REDs,130s total; source restored
before freeze/readers. Full final native passes **701/701 tests** across eleven
binaries (72.434s execution,129.5s command; one opt-in browser test skipped).
Strict Clippy18.6s, Pi library/four binaries4.1s, changed formatting, independent
wait validation, whitespace and all10 affected control-table plans pass.
No frontend/assets changed, so the latest actual984-command phone/desktop
acceptance remains applicable; no redundant browser run was claimed.

Guide: [common fixture settings](gear-settings.md). Evidence:
`/tmp/dali-gear-settings-{red,green,targeted,controls}.log`,
`/tmp/dali-gear-settings-acceptance.log` and
`/tmp/dali-gear-settings-acceptance/`. Next: the managed individual settings
service/strict MQTT/protected HTTP/browser slice, including fresh physical-limit
preflight, crossed-limit ordering, actual readback and existing cache cuts.
Other comprehensive capability rows remain explicitly pending.

## P4b6 — hardware group scalar Desired/State bindings

Frozen production `c27d63a`; fixture/control/documentation correction `2a92da2`.
The combined design gate and sequential final native/owner and MQTT/contract
source lenses APPROVE without blockers. Implementation self-review corrected
the private reply to preserve the original Observation/stamp, instead of matching
public Reports alone; a bus-cycle/new-identical-report regression covers it.
All reviews are source-only/read-only and same-family. No physical validation,
live HAT/Store, shared SDK, deployment, publication or push/merge is claimed.

Saved hardware groups now expose normal BridgeKit PowerState/Brightness Desired,
target refresh and aggregate State. Complete maps reject null/wrong types,
unknown/read-only/colour fields, contradictions and retired/missing group tokens
before mutation. One native group frame follows actual hardware membership,
including untracked gear; sampled State covers the complete saved bus roster.
Uniform power and brightness are independent. Complete mixed evidence remains
Available, with explicit null and a mixed flag for each disagreeing scalar.
Empty/missing/MASK never becomes Off; stale/unobserved fields explicitly clear
older merged values. Scalar Set reports actual values before completion and
rejects incomplete/empty/unconfirmed readback; native Report outcomes stay intact.

The owner caches the same accepted original context/stamp/Arc report before
waking its caller. Physical output mutations retire group evidence on their bus
before possible send, including Blink, scene recall, membership Program and
address operations. Other buses and input measurements/Notices survive; input
feedback does not invalidate groups. Root revision and physical epoch changes
retire full-roster evidence. Cached validity shares existing handling of ordinary
Writing/Awaiting, while final acceptance remains strict. Enabled group All reads
share the existing fair cursor and15s ceiling; no new worker, transport, task,
configuration format or poller was added. DT8 group colour remains pending.

All **7 new native cases** and4 old cursor/All neighbors pass. Full final native
acceptance passes **692/692 tests** across eleven binaries (73.997s execution,
132.1s command). Strict Clippy passes10.43s; Pi library/four binaries, changed
formatting, independent wait validation, **77 Node cases**, assets/whitespace and
all33 affected control-table plans pass. Eight new intentional faults produce
their named REDs in149s. An existing colour-budget control was retargeted from
the old bool branch to ReadKind::Output and remeasured RED in20.5s.

The initial full run had690/692 passing: two older strict literal peers rejected
new membership queries during refresh-all. They now answer only the explicit
C0/C1 queries for their fixture's actual group0 membership, retaining unknown-
command failures. Both targeted cases pass, then the complete rerun passes.
Production code was unchanged by those fixture corrections. Ordinary-read cache
invalidation was corrected earlier after diagnostics proved identical stamps and
normal Awaiting, without changing UART recovery. All temporary diagnostics were
removed. Wait generation and validation remain independent.

Actual final phone/desktop acceptance passes **984 owned commands**,92.42s test
execution. All four fresh mixed/partial group screenshots were inspected; the
existing commissioning, inputs, colour, groups, scenes, feedback, addressing,
authentication and recovery workflows pass with group polling integrated.
No browser source/asset change was needed for the scalar adapter. Artifacts:
`/tmp/dali-browser-985c934590f4d658236f412dcaecf5cd`; logs:
`/tmp/dali-group-scalar-final-{native,browser}.log`,
`/tmp/dali-group-scalar-final-execution.log`,
`/tmp/dali-group-scalar-acceptance`, `/tmp/dali-group-scalar-controls.log`,
`/tmp/dali-group-scalar-peer-delta.log` and
`/tmp/dali-group-scalar-colour-budget-control.log`.
Next: common output settings/diagnostics, with remaining specialist families
still explicitly tracked in the support matrix. No memory/skill changes needed.

## P4b5 — native group power/dimming with actual full-roster evidence

Frozen source `a40f840`; reviewed HTTP correction `f6685dc`. Design and final
native/owner source lenses APPROVE. The UI/API lens found one ordinary-path P2:
large valid full-roster reports exceeded the generic HTTP result cap, losing
the result after native work. Both group commands now share the existing bounded
1 MiB roster allowance; ordinary results stay at32 KiB and list/SSE summaries
omit results. Its delta source review APPROVE. Reviews are source-only/read-only
and same-family; no physical HAT certification is claimed.

Strict MQTT/protected HTTP/typed `read_group_output` / `set_group_output` and
compact Groups controls now send one native hardware-group OFF/MAX/DAPC, then
sample individual membership and actual level/status across every saved output
on that bus. Actual hardware membership includes untracked fixtures. Coverage is
explicitly saved_inventory, not a physical census. Uniform power can coexist
with mixed positive brightness; empty/missing/MASK/collision evidence never
becomes false Off or an agreeing-prefix success. Sent means sent, not requested
level attained. Unknown state permits explicit writes under normal authority.
Names/assignments/configuration remain unchanged. Original group/member mappings,
revision, token, trace, deadline and stamp remain under one permit through final
acceptance. Output/colour actuals on that bus stale before possible mutation and
at settlement, including uncertain/canceled work; other buses and input activity
remain intact. No new task, transport or persisted configuration was added.

All **12 new pure/native cases** pass, including literal group15/address63,
actual hardware selection, ignored commands, strict maps, physical epoch, expired
ready reply, Blink exclusion, bus-local caches and continuing button Notice.
Full acceptance passes **684/684 native tests** across eleven binaries
(71.858s execution/134.9s command), strict Clippy, Pi library/four binaries,
changed formatting, independent wait validation, assets/whitespace and32 affected
control-table plans. All **77 frontend cases** pass. The HTTP correction's
additional typed64-member/long-target regression first fails for the named lost
report, then all **5 job-result cases** pass (0.064s), including read/Set complete
retrieval, summary omission, ordinary limits and the1 MiB upper bound. Narrow
Clippy/Pi/format/wait/control-plan/whitespace checks pass after that correction;
no redundant full-suite or browser rerun is claimed for its bounded result check.

All **12 serial intentional faults** produce their named REDs: eleven initial
aggregate/native/client controls in134s, plus removed HTTP allowance in11.2s.
Wait generation and validation were independent. The actual phone/desktop suite
passes **984 owned commands in85.50s**, with mixed/partial states, native On/Off/
level, explicit unknown-state writes, unchanged configuration/other fixtures,
held replies across slot/navigation and restored physical values. All four new
screenshots were inspected. Its initial setup used private configuration rows
as public targets; the fixture lookup was corrected without production changes.

Guides/contracts/support matrix/CLAUDE describe native command coverage and limits.
Artifacts: `/tmp/dali-browser-affea6b2334b8e30389429b4e3ea432b`; logs:
`/tmp/dali-group-output-acceptance`, `/tmp/dali-group-output-controls.log`,
`/tmp/dali-group-output-browser2.log` and
`/tmp/dali-group-output-http-budget-{red,green,control,clippy,pi,wait}.log`.
No live HAT, Store, deployment/publication, dependency or memory/skill change
occurred. Next: base group scalar Desired/State bindings, then remaining explicit
specialist capability families. DT8 group colour remains pending.

## P4i7 — managed input feedback and compact protected controls

Frozen source `9cbbc1b`; reviewed UI correction `4dd92f4`. Design and final
native/owner source lenses APPROVE. The UI/API lens found one ordinary-path P2:
a failed activation reply left old Off evidence actionable. The narrow fix
clears scope/display evidence before I/O and restores it only from a validated
reply; uncertain actions require Refresh before another toggle. Its delta review
APPROVE. All reviews are source-only/read-only and same-family.

Saved input modules and individual instances now support strict
`read_input_feedback` / `set_input_feedback` through MQTT, protected HTTP jobs
and typed API, with a compact Feedback row menu/dialog. Fresh feature32/support,
capability and closing presence/type proof precede a single explicit native
Activate/Stop; complete matching actual evidence confirms Set. Same-value
activation is explicit, collisions never become false, and unsupported/partial
evidence authorizes no mutation. Original generation/revision/deadline/stamp
and command completion remain under the physical owner. Feedback changes do not
retire button Notices, input values, names, bindings or saved configuration.

All **6 new native tests** pass, including23 independent literal evidence cases,
actual module/instance MQTT/PTY toggles, continuing input activity, disabled
buses and final original authority. Full acceptance passes **672/672 native
tests** across eleven binaries (69.661s execution/130.3s command), strict Clippy,
Pi library/four binaries, changed formatting, independent wait validation,
assets/whitespace and31 affected control-table plans. All **70 frontend cases**
pass. All **11 serial intentional faults** produce their named REDs (initial9
in124s; two UI evidence-lifetime controls in0.6s). Wait generation and validation
were run separately. The final correction is UI-only; no redundant native suite
rerun is claimed.

Actual final phone/desktop acceptance passes **964 owned commands in83.35s**:
read/toggle/reopen/restore on module, button and absolute input, unsupported input,
retained inventory/values/settings, held entry reads across dismissal/navigation,
and actual native activation with an unusable browser admission reply. The stale
toggle disappears; Refresh reads actual On and the next explicit action restores
Off. Four new error-state screenshots were inspected. An initial injected502
version passed its interaction assertions but failed the global console oracle;
the fixture now substitutes an unusable200 admission body without suppressing
console errors. Production behavior was not changed to accommodate the fixture.

Guides/contract/emulator/CLAUDE/support matrix describe the managed path and the
remaining scalar Desired bindings, feedback parameter settings and broader
physical profiles. No live HAT, Store, deployment/publication, dependency or
memory/skill change occurred. Existing ownership/invalidation patterns held in
review; the confirmed UI finding is covered by fail-first recovery and measured
fault controls rather than new transport machinery.

Artifacts: `/tmp/dali-browser-37fdd855e2490d8daac53b068c5d347b`;
logs: `/tmp/dali-managed-feedback-acceptance`,
`/tmp/dali-managed-feedback-controls.log`,
`/tmp/dali-managed-feedback-stale-ui-red.log`,
`/tmp/dali-managed-feedback-stale-ui-controls.log`,
`/tmp/dali-managed-feedback-node4.log` and
`/tmp/dali-managed-feedback-browser3.log`.
Next: native group power/dimming, keeping actual mixed/unknown member evidence
separate from saved group assignments. Remaining specialist families stay listed
in the support matrix; this does not mark all P4 capabilities complete.

## P4i6 — feature decoding and basic feedback foundation

Frozen source `8dccb7f`. The combined design gate and sequential final codec/
contract and independent-model/compatibility source lenses APPROVE, without
findings. Reviews were read-only/source-only and same-family. No live HAT,
Store, deployment, publication or dependency change occurred.

Pure `input::features` validates feature32..96 and decodes bounded captured
Type/Next sequences: FE is none/end, FF multiple, with strict ascending numbers
and a final terminator. Pure `input::feedback` supplies individual device/instance
Activate/Stop/Capability/Active frames and lossless base capability flags. Input
and feature selector namespaces stay distinct; basic feedback is one ordinary
24-bit send. Unknown capability bits remain diagnostic data, not authority.

The independent emulator has opt-in visible on/off profiles at device or existing
instance scope. Scenario `feedback` and runtime `feedback_state` are distinct,
omitted-empty vectors, preserving legacy serialized shape. Activation/stop are
idempotent, independent of input enablement and sensor settings. HAT reboot and
readdress retain the powered fixture model; no fixture power-cycle guarantee,
timed/colour/audio profile, managed operation or web feedback control is claimed.
Codec/emulator/service guides, CLAUDE and support matrix explain these boundaries.

The first valid runtime baseline failed all3 new literal model cases. All **7 new
codec/model/actual-PTY cases** and **17 neighboring input cases** pass. All **7
intentional faults** produce their named REDs in94.0s measured command time; one
whitespace anchor was rejected before mutation, corrected and rerun separately.
Full acceptance passes **666/666 native tests** across eleven binaries
(67.577s execution /127.2s command), strict Clippy, Pi library/four binaries,
formatting, standalone wait validation, whitespace and all11 affected control
plans. No new runtime wait/task/owner or bridge configuration field was added.
Web source/assets did not change, so the prior real phone/desktop pass remains
the latest UI evidence; no redundant browser run was claimed.

Logs: `/tmp/dali-feedback-model-red2.log`,
`/tmp/dali-feedback-foundation-native.log`,
`/tmp/dali-feedback-foundation-native2.log`,
`/tmp/dali-feedback-foundation-controls.log`,
`/tmp/dali-feedback-foundation-control-end.log` and
`/tmp/dali-feedback-foundation-acceptance`.
No reusable skill update is needed. Next: managed capability/feedback operations
and protected web controls, then remaining input/output capability families.

## P4i5 — absolute, occupancy and light sensor event parameters

Frozen source `b776346`. Design and final native/model and UI/API source lenses
APPROVE without findings. Reviews were read-only and same-family. No live HAT,
Store, deployment, publication or dependency change occurred.

The existing input-settings operation and protected dialog now support all nine
base Part302–304 parameter bindings: report/deadtime for all three families,
occupancy hold and light percentage/minimum hysteresis. Pure checked plans and
independent emulator profiles use separate command maps. Fresh optional evidence
preflights a whole mixed patch before any common/sensor write, then actual
requested values must match. HoldFF is known unsupported; null is unknown;
raw0 means one second. Other numericFF bytes remain data. Optional quality stays
independent so common-only settings remain usable with partial sensor evidence.
The original owner, permit, authority/deadline and permanent Notice cuts remain.
Saved identities, bindings, configuration and input measurement values are kept.

The same compact dialog shows exact seconds/50ms steps and applicable hysteresis
fields without rounding. Unsupported hold leaves report/deadtime editable. API,
operator and contributor guides describe native bytes versus display units and
the limits of the independent profiles. No natural stimulus scheduler was added.

Tests first failed on missing model/managed support and missing frontend guards.
All **22 focused native cases**, **61 frontend cases** and **11 serial fault
controls** pass (each intentional fault produced its declared RED,109.2s total).
The complete suite passes **659/659 native tests** across eleven binaries
(67.500s execution /132.9s command; the separately owned browser is skipped).
Strict Clippy, Raspberry Pi library/four binaries, changed-file formatting,
standalone wait validation, embedded asset byte check, whitespace and all twelve
affected fault-control table plans pass. No new wait or owner was introduced.

Actual full phone/desktop acceptance passes **921 owned commands in75.76s**,
including all three sensor families, rawFF, one-second hold, save/reopen/restore
and retained inventory. All six sensor screenshots were inspected. An initial
full browser run passed the sensor flows but caught an old four-child count;
the fixture now includes occupancy as its fifth instance. Both reassignment
counts and explicit family coverage are checked in the passing run.

Artifacts: `/tmp/dali-browser-fe073b2c6413c27988b46db167abb41b`;
logs: `/tmp/dali-sensor-settings-browser2.log`,
`/tmp/dali-sensor-settings-native3.log`, `/tmp/dali-sensor-settings-node3.log`,
`/tmp/dali-sensor-settings-controls.log` and
`/tmp/dali-sensor-settings-acceptance`. No reusable skill update is needed.
Remaining inputs include indication/feedback, natural emulator stimuli,
group/source routing, newer sensitivity/range capabilities and physical validation.

## P4i4 — managed push-button timing and exact-unit browser fields

Implemented/frozen at `334b264`; internal allocation/documentation follow-up
`2dea106`. Design and final native/owner and UI/contract lenses APPROVE. The native
lens also approves the follow-up. Reviews are source-only/read-only and
same-family. No live HAT, Store, deployment, publication or dependency change.

Existing `read_input_settings` / `configure_input` now expose an optional strict
`button_timers` patch and separate raw timing evidence. Fresh short/double minima
preflight the whole mixed patch before any common/timer write; double0 still
requires valid minimum evidence. Optional partial timer quality does not block
common-only settings. Actual requested timers must match complete readback.
The original owner, authority/deadline, repeat bound and permanent Notice cuts
are retained. Saved configuration, names/bindings and current input values remain
independent of these hardware settings.

The existing protected dialog adds compact short/double/repeat millisecond fields
and stuck seconds, with exact native steps, reported minima and no rounding.
It sends only changed timers and rejects invalid or forged complete evidence.
The original frozen row/root/epoch and synchronous/layout dismissal guards remain.
Operator/API details: [input event settings](input-event-settings.md).

Two native tests first failed on the unsupported nested patch; two frontend tests
failed on the missing matching/minimum guards. All **12 focused native cases**
(four new plus eight existing) and **54 frontend cases** pass. Literal peers record
actual input traffic, proving missing/invalid minima and a later invalid timer
cannot allow earlier common/timer writes; ignored hardware writes stay unconfirmed.
All **seven intentional-fault controls** produce intended REDs in52.3s. The
no-submit assertion checks command count before error text so later validation
cannot mask a missing preflight. No reusable skill change is needed.

The full suite at `334b264` passes **649/649 native tests** across eleven binaries
(66.695s execution / 125.9s command), with the separately owned browser skipped.
Its sole acceptance failure was strict Clippy: added diagnostic data enlarged
the queued physical request. `2dea106` boxes only the stored fallback report;
wire data and control behavior are unchanged. All12 affected native cases, strict
Clippy, Pi library/four binaries, formatting and whitespace pass afterward.
Standalone wait validation, frontend, asset byte checks and affected control
plans pass; no new waits or owner were introduced. One stale CLAUDE scope sentence
was corrected from Timers to Sensor timers.

Actual phone/desktop acceptance passes **894 owned commands in71.65s**, including
four timer edits, native raw values, reopen/restore, scrollable actions and retained
inventory. Both screenshots were inspected. Artifacts:
`/tmp/dali-browser-449efbb12d15a3cb0bec8763fc9f3374`; logs:
`/tmp/dali-managed-button-timing-browser.log`,
`/tmp/dali-managed-button-timing-acceptance`,
`/tmp/dali-managed-button-timing-native-final.log`,
`/tmp/dali-managed-button-timing-clippy-final.log` and
`/tmp/dali-managed-button-timing-pi-final.log`.

Remaining inputs: sensor timers/hysteresis, supported feedback/indication, natural
emulator stimuli, group/source routing, calibrated units and physical validation.

## P4i3 — push-button timing protocol and independent model

Frozen source `2011e6a`, browser-test compatibility follow-up `2eec299`.
Design and both final source lenses APPROVE; the model lens also approved the
follow-up. Reviews were read-only/source-only and same-family. No live HAT,
Store, deployment, publication or shared dependency change occurred.

`dali::input::button` provides short/double/repeat/stuck values, current/minimum
queries, exact native durations and DTR0/repeated addressed plans. Static ranges
and freshly reported device minima are separate checks; invalid minima cannot
authorize even double0. FF is valid short/stuck data. The independent per-button
profile shares the existing repeat owner, preserves settings/latches through
readdress and HAT reboot, and has explicitly limited defaults/invalid-write policy.
It schedules no gestures. Managed timer commands/UI remain the next slice.

Four literal model cases first failed on missing timing support. All seven
focused codec/model/UART cases pass; all six intentional controls produce intended
REDs in85s. Full native acceptance passes **645/645** across eleven binaries
(66.303s execution / 123.3s command including build/discovery), with only the
separately owned browser test skipped. Strict scoped Clippy, Pi library/four
binaries, changed-file format, standalone wait validation, all affected control
plans and whitespace pass. Logs: `/tmp/dali-button-timing-acceptance`,
`/tmp/dali-button-timing-controls.log`, `/tmp/dali-button-timing-functional.log`
and fail-first `/tmp/dali-button-timing-red.log`.

Final reader inspection found the old browser assertion compared a whole native
settings object with a fixed list of old fields. It now explicitly verifies the
requested event values while retaining unrelated fields from the initial physical
snapshot, so timer preservation is checked too. No production browser change.
The real phone/desktop workflow passes **894 owned commands in72.24s**;
log `/tmp/dali-button-timing-browser.log`, artifacts
`/tmp/dali-browser-23633267e9d2a3cc1cc15cf844f12e07`. No second full native run is
needed after that test-only change.

Contributor/operator example: [button timing plans](dali-input-contract.md#push-button-timing-plans),
[emulator profile](emulator.md#push-button-timing-profile). Managed timing, sensor
timers, indication/feedback, semantic stimuli and physical validation remain.
The existing reader-completeness safeguard covers the browser assertion catch;
no reusable skill change is needed.

## P4i2 — managed input event settings and browser controls

Implemented at frozen source `7bab05b`; `c899f02` corrects wait-registry metadata
and two misplaced annotations without changing runtime behavior. The design,
native/owner and UI/contract review lenses APPROVE; the native review also
approved the metadata correction. Reviews were source-only/read-only and
same-family. No live HAT, Store, deployment or publication changes occurred.

Strict MQTT/HTTP and typed `read_input_settings` / `configure_input` share the
existing physical owner. They read and patch enable, priority, all five event
schemes, common event filters and three instance groups, then return actual
readback. Writes retain the original target token, root revision and deadline;
type/presence proofs distinguish disabled inputs from missing devices. Missing
bytes remain unknown, FF remains data, and uncertain writes cannot claim success.
Admission and settlement permanently retire old input Notices. Settings results
do not replace native values or change saved configuration/identity.

The protected input-instance row menu now opens Event settings. Its compact form
supports changed-field saves, current hardware reads and refresh. Frozen authority,
synchronous dismissal and layout cleanup prevent late replies from reviving a
closed dialog. Schemes 0/3/4 are explicitly diagnostic-only in this bridge;
scheme 1 requires a unique saved device/type and scheme 2 identifies the instance.
Unknown-type filters remain raw and read-only. Operator/API details are in
[input event settings](input-event-settings.md).

Eight focused native cases and all **49 frontend tests** pass. All eleven
intentional-fault controls produce intended REDs. One initial UI outcome control
was masked by its fixture's requested-value mismatch; separating those conditions
made the outcome, mismatch and lifetime controls each fail for their own guard.
See `v2-managed-input-settings-controls.toml` and its ledger.

The full native run passed **637 functional tests**; its sole additional failure
was the wait registry. Two misplaced wrapper annotations and one undeclared local
method were corrected, then both the registry test and standalone wait validation
passed (zero findings). All 638 checks therefore passed across the full and
targeted runs. The full run took 66.751 s execution / 125.1 s including compilation.
Strict scoped Clippy, Pi library/four binaries, affected control plans, changed-file
formatting, committed-asset byte checks and whitespace pass. Clippy's initial
large-enum finding was fixed by boxing only the new internal diagnostic report.

The real phone/desktop browser workflow passes through **894 owned commands in
70.74 s**, including read/save/reopen and cancellation/navigation during a held
entry read, plus the existing scene/address workflows. Both settings screenshots
were inspected. Artifacts: `/tmp/dali-browser-708b3ee307975609fc7b3517b2a0b2d5`;
logs: `/tmp/dali-managed-input-settings-browser2.log`,
`/tmp/dali-managed-input-settings-acceptance`,
`/tmp/dali-managed-input-settings-registry-final.log` and
`/tmp/dali-managed-input-settings-wait-final.log`.

Timers, supported indication/feedback, semantic emulator stimuli and group/source
routing remain pending. No reusable skill change is needed: the existing independent
guard/control requirement covers the fixture correction.

## P4i1 — native input event-setting foundation

Implemented at frozen source `f71719c`; design and both source review lenses
APPROVE. Reviews were source-only/read-only and same-family; no physical HAT or
Store changes occurred. This is a protocol/emulator milestone. Managed setting
commands and browser controls are the next slice, not completed by this one.

`dali::input::settings` supplies checked enable/disable, priority, all five event
schemes, 24-bit filters and three instance-group plans. Only the addressed setting
is repeated, as two ordinary 24-bit sends; the ATX `t` flag is 16-bit only. Raw
filters preserve FF/high bytes; common-type validation is separate. The independent
emulator reads/programs per-instance settings with its existing 100 ms repeat
owner, preserves value latches and fixture state across readdress/HAT reboot,
and limits changes to the selected instance. Discovery remains passive.

Four initial literal cases failed against missing settings support. The final
focused suite passes 11 cases, including every setting's repeat requirement,
100/101 ms boundaries, input/output interruption, FD/FF selection, invalid DTRs,
last-address/instance boundaries, sibling isolation and literal UART/reboot/raw
injection. All five intentional-fault controls produced intended REDs in 63 s.
See `v2-input-settings-controls.toml` and its ledger.

Full acceptance passes **630/630 native tests** across eleven binaries (65.028 s
execution, 121.1 s command including compilation), with the separately owned
browser test skipped. Scoped strict Clippy, Pi library/four binaries, changed-file
formatting, standalone wait validation (zero findings) and whitespace pass.
The first plan invocation incorrectly passed multiple tables to a single-table
tool; a corrected per-table run passes all affected control plans. No production
change was required. No browser source changed, so no browser/Node rerun was
needed. Logs: `/tmp/dali-input-settings-acceptance`,
`/tmp/dali-input-settings-controls.log`,
`/tmp/dali-input-settings-plans-final.log`; fail-first evidence is
`/tmp/dali-input-settings-red.log`.

Contributor/operator emulator coverage is in [input codec](dali-input-contract.md)
and [emulator](emulator.md#input-event-settings-model). Raw `input_event` injection
deliberately bypasses settings; natural stimulus generation, timers, feedback and
hardware persistence validation remain pending. No reusable skill change is needed.

## P5i3 — clear one short address while retaining its saved identity

Accepted locally at frozen source `8dc03fc` (initial implementation `aeac073`).
The design gate and both final source lenses APPROVE. Reviews are same-family
and read-only/source-only; physical HAT and Store deployment remain unverified.

`clear_short_address` is available through strict MQTT, authenticated HTTP, the
typed service handle and output/input-module row menus. It uses a distinct Clear
action in the existing restoration physical owner: fresh presence, DTR0 FF plus
an addressed repeated write, fresh absence and current final stamp/authority.
No ConfigStore edit occurs. Saved names, child identities, binding intent and
the old reservation remain; adoption after rediscovery frees the old reservation.
The report's address is the saved reservation, while `cleared` describes removal
of the physical address. Missing presence refuses writing; uncertainty cannot
claim success. The UI requires confirmed matching Blink stop and performs no
automatic rediscovery. Current observations and queued input Notices use the
existing admission/settlement cuts.

The new preservation regression first failed against a stub native plan.
Four focused cases cover both spaces/profiles, saved bytes and complete model
state, other fixtures/native settings, strict MQTT and original replies,
profile/authority/final-evidence refusal, and an independent literal UART peer.
DTR scratch registers and advancing clocks are deliberately excluded from
native-setting equality. The literal peer rejects broadcast/commissioning
frames and checks required presence, addressed repetition and final absence.
The 21-case recovery/clear suite passes. All eight new intentional-fault controls
produce intended REDs, including the refreshed formatted profile anchor.

Full native acceptance passes **620/620** across eleven binaries, with only the
separately run browser test skipped. Strict scoped Clippy, Pi library/four
binaries, standalone wait validation and **45 frontend tests** pass. The initial
restored gate found only Rust formatting in the admission expression; that was
corrected and its control anchor rerun. The final restored native/lint/Pi/wait,
format and asset checks pass. The control-plan pass also found an older Stop
anchor matching both recovery and clearing; it was qualified to its recovery
error text, reran intended RED, and all affected plans then passed. Those two
check corrections did not alter runtime behavior. Final plan/Node/asset logs are
`/tmp/dali-individual-clear-plans-final.log`,
`/tmp/dali-individual-clear-node-final.log` and
`/tmp/dali-individual-clear-dist-final.log`.

One source-review P1 exposed deferred passive cleanup: dismissing Clear while
awaiting Blink stop could still send the destructive command. The actual browser
regression first reproduces that mistake with passive timers/animation frames
paused (`/tmp/dali-individual-clear-lifecycle-red.log`, unexpected command after
Cancel, 402 versus 401). Explicit dismissal now revokes synchronously and
`useLayoutEffect` cleanup revokes during component removal. Actual Cancel,
Close and navigation all pass before releasing the held completed Stop reply.
The browser guide records this handler/lifecycle test requirement. An unrelated
immediate filter-count assertion was changed to wait for its rendered result;
its failure screenshot already showed the correct empty filtered list.

Final real phone/desktop acceptance passes through **886 owned commands in
70.01 seconds**, including individual output/input clear and explicit recovery,
the three delayed-cleanup dismissal cases, and existing scenes, restoration,
reassignment and logout workflows. Artifacts:
`/tmp/dali-browser-f1f96d998fb1e710520ee3351923226b`; initial phone/desktop Clear
confirmation screenshots were inspected at
`/tmp/dali-browser-7202173371a0437f7688c940a38043d6` (77.49-second initial run).
Full logs: `/tmp/dali-individual-clear-acceptance`,
`/tmp/dali-individual-clear-restored-final` and
`/tmp/dali-individual-clear-lifecycle-final.log`.
See [operator/API instructions](address-recovery.md), the
[specification](superpowers/specs/2026-10-23-dali-v2-individual-address-clear.md)
and `v2-address-clear-controls.toml`. No live bus, Store, legacy, SDK publication,
push, merge or deployment action belongs to this slice.

## P5i2 — identify recovered fixtures and adopt their new addresses

Implemented locally at `f53e4bc`, with the discovery-admission fix at frozen
source `55a782c`. Both design and source review lenses APPROVE; both source
reviewers extended approval over the admission fix. Reviews are same-family,
read-only/source-only. No physical HAT or actual Store migration is claimed.

The Devices page can recover a saved fixture through candidates found by normal
**Add unaddressed** or its own single-step Find. Blink each new light, select the
original saved fixture, review the identified candidate and adopt its new address.
Confirmed Stop restores Blink's original brightness first. The original Origin,
name, descriptions, representation, group/scene intent and consumer bindings
continue; the temporary candidate retires and the old reservation becomes free.
Input modules carry their original children to the identified new address.
Identified recovery queries hardware only. The sole-unaddressed shortcut can
reuse the old reservation, including when all 64 saved slots are occupied.

Fail-first input-model and UI cases reproduced missing child continuity and
scan-first candidate selection. Service, literal UART and independent-consumer
cases cover atomic removal/readdress, save and stamp guards, native preservation,
old-slot reuse, original MQTT completion, child/object continuity and replay.
Permanent per-route Notice revocation prevents queued old presses resurfacing
after unique recovery leaves its token unchanged. Actual paused MQTT and SSE
tests cover that behavior after fresh activity/readback.

All 13 recovery fault controls produced their intended REDs; six older controls
affected by helper/guard changes were rechecked and produced intended REDs.
The longer browser run twice passed recovery and exposed Busy during the existing
full-rescan workflow: periodic state reads share discovery's nonwaiting permit.
Discovery now queues once within its original deadline, then validates its
original root/token. Its fail-first literal-peer test covers contention,
cancellation, expiry and a root retired during waiting, with no invalid UART
submission. The added discovery control also produces its intended RED.

Earlier acceptance passed all 615 native and 40 frontend tests. Final full-suite
execution adds the admission regression: 615/616 passed; the sole failure was
the wait registry requiring the exact words "on expiry" in its new prose row.
That wording was corrected and both the registry test and standalone checker
were rerun successfully (zero findings). No production change followed. The
other restored checks pass: strict scoped Clippy, Pi library/four binaries,
changed-file formatting, affected control plans, 40 frontend tests, deterministic
assets and whole-base whitespace. Logs: `/tmp/dali-restoration-final-acceptance`
and `/tmp/dali-restoration-registry-final`. The earlier doctest run has no cases.

Final real phone/desktop acceptance passes in 66.55 seconds through 867 owned
commands with normal periodic polling enabled. It explicitly scans two fixtures
that lost addresses, Blinks/adopts each, checks both freed reservations and then
passes output-only, input-only and both-space full reassignment. Artifacts:
`/tmp/dali-browser-069b6e2eb496220a1dcc56c23ef91dab`. Phone and desktop recovery
screenshots were inspected. Initial Busy failures and their diagnostics remain
in `/tmp/dali-restoration-acceptance` and
`/tmp/dali-restoration-browser-diagnostic.log`; fail-first and intentional-fault
admission evidence is in `/tmp/dali-discovery-contention-*`.

See [address recovery](address-recovery.md), its
[specification](superpowers/specs/2026-10-22-dali-v2-p5i-address-restoration.md)
and [fault controls](v2-restoration-controls.md). Individual short-address
clearing was still pending at this checkpoint and is implemented in P5i3 above.

## P5i1 — full selected-space address reassignment

Accepted locally at frozen source `2c39885` (production `dbc4f84`): **595 native
tests**, **32 frontend tests**, all **14 intentional-fault controls**, scoped
strict lint/Pi/wait/format/control-plan/asset checks and the real phone/desktop
workflow pass. Both sequential design and source lenses APPROVE. Reviews are
same-family/source-only; no cross-family or physical HAT validation is claimed.

`reset_address_space` exposes strict controller-only output/input clearing via
shared MQTT/HTTP and a typed helper. One atomic removal vector, ConfigStore save
and normal generation activation retire saved identities before native mutation.
Original request/context/deadline/permit/completion and physical stamp survive
the handoff; final success requires all 64 absent short addresses and the current
own committed mapping. Otherwise the reply retains retirement and reports
unconfirmed. Empty saved lists still clear unsaved physical gear. Output group
references/private scene rows and input descendants retire through existing
edits; fresh defaults get new Origins. Other buses/spaces remain unchanged.

Bus actions offers a frozen outputs/inputs/both confirmation. Each selected
space clears, assigns/discovers one device at a time, then refreshes addressed
devices/current values before the next space. Capacity stops normally after 64
verified assignments; it does not promise no additional unaddressed gear exists.
Cancellation, tab lifetime, connection epoch and exact own revision fences stop
later submissions. Native levels/colour, group bits and scene memory remain.
See the [operator/API guide](reassignment.md).

Fail-first independent model tests reproduced absent standard input address
writes/broadcast/DTR0. Literal tests assert 16/24-bit vectors, raw input addresses,
repeat timing/interruption, all 64 addresses and native/other-space preservation.
Service tests cover both profiles, save failure/uncertainty, held-save permit and
original stamp, final stamp, no-op saved spaces, MQTT original completion/strict
params, missing saved rows, unaffected other-bus Origins and fresh reused names.
The literal peer asserts Ready/durable retirement at the first clear frame and
checks transport failure plus partially completed absence queries independently
of the physical model. Eight scoped restored checks pass in
`/tmp/dali-reassignment-restored-final`.

The initial control run records 13 intended RED rows and one diagnostic mismatch:
the Busy assertion lacked the table's custom failure text. The assertion message
was corrected, the two affected tests passed and that control reran intended RED
(ledger retained). No production change followed source review. Both reviewers
extended approval through the test-only `2c39885` delta. Final acceptance is
`/tmp/dali-reassignment-acceptance`:595/595 native across 11 binaries in 58.519 s
execution/111 s command, doc tests, 32 frontend tests and the 53 s browser run with 697
owned commands. Its required assertions match the earlier 614-command run; the
count varies with streamed/private result scheduling. Screenshots/log:
`/tmp/dali-browser-287138a94b72bc3dd89c5eff1c265bef`.

The wait checker required an explicit existing-operation tag at the local clear
call; the final registry reports 0 findings. The input-only confirmation text was
scoped to its own effects during screenshot review. No SDK/UART recovery,
Store, PanelScript, legacy source, live bus or deployment action belongs to this
slice. Lost-address restoration is the next commissioning boundary.

## P5h3 — protected Scenes screen and reviewed operations

Accepted locally at frozen source `77e7ae1` (initial source `f92c496`, revision
fix `9dac765`): **583 native tests**, **26 frontend tests**, the real browser
workflow and both final source reviews pass. Both design lenses approved after
grounding the complete reader's HTTP budget and result-summary transport.
Reviewers are same-family/source-only; no cross-family or physical HAT coverage
is claimed.

The served Scenes page provides sixteen slots, inline naming and raw-preserving
brightness/Tc/xy/RGBWAF settings, per-attribute MASKs, Blink/Stop, native bus/group
recall, frozen capture/copy/clear previews, adoption/repair and all-slot native
verification. Drafts and batches retain original scene/fixture identity, exact
root revision, connection epoch and page/slot lifetime. Public Inventory remains
opaque. `read_scene_settings` is a pure protected saved view without UART or
enabled-bus requirements. Only that complete reply gets a 1 MiB HTTP budget;
ordinary replies retain 32 KiB. SSE/list job feeds carry summaries, and waiters
fetch full results explicitly without replacing streamed observations.

Fail-first reader/transport cases reproduced missing offline reads, insufficient
result capacity and completed-summary handling (`/tmp/dali-scenes-ui-reader-red`).
Four focused native cases cover complete mapping, optional-token/Ready checks,
64 long-name fixtures over real HTTP, saved configuration across ConfigStore
restart, and actual emitted retained Inventory privacy. The restart test first
needed tombstone exclusion, then a fresh broker per process to avoid a historical
subscription being mistaken for the restarted process's live subscription.
The corrected 56-case scene/job library run passes
(`/tmp/dali-scenes-ui-restored-fixed`). Scoped strict Clippy passes
(`/tmp/dali-scenes-ui-restored-first`).

Twenty-six frontend tests pass. Real phone (390×844) and desktop (1440×1024)
final acceptance passes through 325 owned commands, covering all three colour families,
attribute masks, failed save/retry, frozen capture after changed actual output,
scoped copy/clear/adoption, matching repair review, group recall, all sixteen
slots, metadata/privacy, late creation and page cancellation. Both screenshots
were inspected. A harness-only lookup incorrectly expected public targets in
private config; it was corrected. Phone slot-strip duplication was removed, and
desktop screenshot capture now settles layout at the top before capture.
Initial complete logs: `/tmp/dali-scenes-ui-browser-lifetime`; final frozen-source
native/browser/assets acceptance: `/tmp/dali-scenes-ui-acceptance`.
Final browser artifacts: `/tmp/dali-browser-a31c9faed95d94fcb6d10262337946ec`.

The first frozen source review found an unnecessary consecutive-revision bound:
ConfigStore can consume a revision during recovery. The regression first fails
on that bound (`/tmp/dali-scenes-ui-revision-red`); the corrected batch accepts its
own 1→3 result, then submits against exactly 3. Identity, monotonicity and exact
SSE acceptance remain enforced. Twenty-five tests/build/check-dist pass after
the fix (`/tmp/dali-scenes-ui-revision-fixed`). The second source review found
that adoption can commit while its physical evidence becomes stale. The UI now
retains that revision but requires the backend's matching comparison before
claiming confirmation or continuing. Its unresolved/different/matching regression
first fails on the prior code, then passes with the inverse guard control and all
26 frontend tests (`/tmp/dali-scenes-ui-adoption-red`,
`/tmp/dali-scenes-ui-adoption-fixed`). Both reviewers approve `77e7ae1`.

All eighteen current scene UI
controls have intended RED evidence: sixteen in `/tmp/dali-scenes-ui-controls`,
plus the consumed-revision row and refreshed external-edit row in
`/tmp/dali-scenes-ui-revision-controls`, and the stale-adoption row in
`/tmp/dali-scenes-ui-adoption-fixed`. All thirteen older browser controls and
the existing encoded-result bound also produce intended REDs. Restored assets
were rebuilt and byte-checked after the real browser mutation.

The restored **64-case** scene/job/codec run, scoped strict Clippy, Pi library/four
binaries, changed-file formatting, every affected control plan, wait validation
(zero findings) and full base diff whitespace pass
(`/tmp/dali-scenes-ui-restored-final`). The final frozen-source full suite passes
**583/583** across eleven binaries, with only the separately-run browser test
skipped. Doctests contain no cases. The required final phone/desktop run then
passes, followed by all 26 frontend tests and deterministic check-dist.

Browser repair proves the matching/no-change review path; actual hardware drift
repair remains covered by the accepted service/literal cases. Private saved
settings are not an active-scene claim. No Store/PanelScript work, SDK publication,
physical test, push, merge or deployment belongs to this acceptance. See the
[operator workflow](scenes-browser.md), [design](superpowers/specs/2026-10-22-dali-v2-p5h-scenes-ui.md)
and [fault controls](v2-scenes-ui-controls.toml).

## P5h2b2 — private scene rows and nonactivating settings

Accepted locally at frozen source `33ea901`: **579 native tests** and both
sequential frozen-source reviews pass. Both design reviews also approve.
Reviewers are source-only and same-family; no cross-family review is claimed.
Private rows use strict native Setting values and continuing output Origins in
the existing v2 file. Public Inventory/history/HTTP/SSE remain opaque. Bounded
Set/Adopt/Repair/Capture use the existing service/physical owner and save before
program, preserving original scene authority, fixture selection and stamp.
Read_scene adds saved expectation/comparison. No-op Set still programs;
adoption saves only; repair checks reviewed evidence again before writing.
Capture uses actual native family/assigned channels without saving or caching.
Cleared-slot MASK copy prevents pending colour contaminating base-only Store.

Fail-first JSON/private persistence and MQTT operation cases fail for the intended
missing feature (`/tmp/dali-scene-settings-model-red`,
`/tmp/dali-scene-settings-service-red`). Fifteen new focused tests now pass,
including strict masks and privacy, Origin lifecycle, Tc/xy/RGBWAF under both
repeat profiles, native literal sequence/pending colour, partial writes,
save failure/uncertainty, retained physical stamp, final acceptance, reviewed
adoption/repair, assigned-channel capture and authenticated HTTP jobs. Scoped
strict Clippy passes (`/tmp/dali-scene-settings-complete-cases`).

The expanded harness initially needed qualified test-module names and a lint
cleanup. A timeout test overclaimed that Busy must continue after the caller
deadline; the existing actor may settle to late-reply refusal. The corrected
test proves Busy during active work and refusal/no new UART send while ordering
is unresolved (`/tmp/dali-scene-settings-timeout-check`). Recovery policy is
unchanged. An initial parser case caught extra data on serde unit MASK variants;
empty structured variants now enforce strict fields.

The first sensitivity pass required moving physical assertions before generic
outcome assertions. Its held bus-cycle case stayed protected by overlapping
admission guards; the final control now uses the existing HAT harness to isolate
original-stamp refusal after the cycle has already been observed. Tokenless
capture is also explicit. The current table has fourteen intended-RED rows.

The final restored **49-case** scene run, scoped strict Clippy, Pi library/four
binaries, changed-file formatting, all affected control plans, wait validation
(zero findings) and whitespace pass (`/tmp/dali-scene-settings-deadline-final`).
Control-plan validation found an older group stamp anchor matching twice after
adding the scene arm; it is now qualified to the group route and its real row
still produces the intended RED. All fourteen current scene controls have intended
RED evidence (`/tmp/dali-scene-settings-controls-final` plus
`/tmp/dali-scene-settings-control-refinements`). Self-review also aligned capture's
final deadline fence with existing reads; the final restored check includes that
small correction. Full native acceptance passes **579/579** across eleven
binaries, with the opt-in browser case skipped; doctests contain no cases. Final
base-to-source whitespace and clean-tree checks pass
(`/tmp/dali-scene-settings-acceptance`). Both sequential frozen-source reviewers
approve without blocking findings. They note two proof granularity limits:
private reopen reconstructs Model from serialized Inventory rather than restarting
ConfigStore, and MQTT privacy asserts central state_bytes rather than an emitted
packet. Authenticated HTTP privacy is exercised end to end; source tracing confirms
all public publishers use the stripped central reader.
Browser scene composition stays in P5h3; no physical
HAT or Store integration claim is made. See the
[slice design](superpowers/specs/2026-10-22-dali-v2-p5h-scene-editing.md) and
[private row contract](dali-scenes-contract.md#private-per-fixture-settings).

## P5h2b1 — owned stored-scene reads and native recall

Accepted locally at frozen source `b45c471`: **564 native tests** and both final
sequential source reviews pass. Named scenes
can be recalled on their bus or a native numeric group. Individual outputs can
read any stored slot, including unnamed ones, with explicit Off/MASK/missing/
collision and stored-colour evidence. Queries do not activate the scene or
replace actual State. Recall says sent, invalidates bus cached actual values,
including failed/cancelled possible sends, and refuses continuing Blink claims.
All work reuses the existing service/physical owner and quiet recovery.

Both sequential source-only design reviews APPROVE (same family). The initial
real MQTT case fails on unsupported read_scene before implementation. Eight new
cases now pass: native group/bus scope, original scene authority after unrelated
controller edits, tokenless Read classification, independent literal stored
Tc/xy/RGBWAF and word/MASK/collision handling, unknown/unsupported types, canceled
recall ownership/cache invalidation, final stale-stamp handling, authenticated
HTTP/Blink conflict and a real canonical daemon plus standalone emulator.
Scoped strict Clippy also passes. Logs: `/tmp/dali-scene-operations-red`,
`/tmp/dali-scene-operations-expanded`, `/tmp/dali-scene-operations-cases-final`.
The first standalone harness compile exposed an owned String where SendOptions
expects a borrowed string; that test-only error and snapshot control spelling
were corrected before the passing run. All twelve scene controls produce intended
behavioral REDs (196 s), as does the existing headerless group-read classification
control (12 s). The restored eight-case run passes before mutations. Logs:
`/tmp/dali-scene-operations-controls`. Restored eight-case checks, scoped strict
Clippy, Pi library/four-bin check, changed-file formatting, affected control plans,
wait validation and whitespace pass (`/tmp/dali-scene-operations-check`). The
wait scan first required declaring the local `scene` async method; the corrected
validation passes with zero findings (`/tmp/dali-scene-operations-wait-check`). The
stored-colour case additionally verifies every returned attribute after mutation
runs. Full native regressions and frozen-source gates remain pending; no physical
HAT or browser acceptance is implied.
See the [slice design](superpowers/specs/2026-10-22-dali-v2-p5h-scene-operations.md).

Initial frozen-source reviews APPROVE `57fa74e` without blockers (same-family,
source-only). The depth review identifies a control-table EOF blank line, now
trimmed, and notes that the existing caller deadline may return Uncertain before
the owner's settlement. A deterministic fail-first case confirms old actual
values remain Current while recall is held in flight. Admission now publishes
stale values before any possible send, with shared completion-time invalidation
retained. Nine focused cases pass, including actual timeout and cleanup. Logs:
`/tmp/dali-scene-operations-inflight-red` and
`/tmp/dali-scene-operations-inflight-controls`. The control table now has thirteen
rows; the cache control is retargeted to the shared guard, and a new admission
control detects delayed invalidation. Both cache controls produce intended REDs;
restored nine-case checks, scoped strict Clippy, Pi lib/four bins, formatting,
affected control plans, wait validation and final base-to-worktree whitespace
check pass (`/tmp/dali-scene-operations-inflight-check`). Fix-set source review
and full regressions follow below.

Both fix-set frozen-source reviews APPROVE `b45c471` without findings, including
admission publication before possible send and exact bus scoping. Final restored
acceptance passes **564/564 native tests** across eleven binaries (one opt-in
browser test skipped), doctests (none), complete base-to-HEAD whitespace and clean
worktree checks. Native command wall time is 98 s; logs:
`/tmp/dali-scene-operations-acceptance`. Scoped strict Clippy, Pi library/four-bin,
formatting, affected plans and zero-finding wait validation remain green from
the restored fix check. All thirteen current scene controls have intended RED
evidence (eleven unchanged initial rows plus the retargeted/new cache pair), as
does the existing headerless group-read control. Reviews are same-family and
source-only; cross-family and physical HAT validation are not claimed.

## P5h2a — durable named scene slots

Accepted locally at frozen source `a03ffee`: **555 native tests** and both
sequential source reviews pass.
This slice adds only named-slot metadata through the existing config, identity
ledger, durable owner and shared MQTT/authenticated HTTP job path. Creation,
rename, description and list removal do not touch hardware. Rename preserves
identity; removal/recreation starts another lifetime. Other device bindings stay
unchanged. Physical scene jobs and the Scenes screen remain subsequent slices.

Both sequential design reviewers approve (40 s and 15 s, same-family/source-only).
Valid scene JSON and real MQTT creation fail behaviorally before implementation.
Nine new cases now pass, covering strict config/payload validation, independent
current/history replay, identity continuity, durable reopen, original scene
authority after unrelated edits, stale-token refusal and authenticated HTTP jobs.
Logs: `/tmp/dali-scene-identity-red.log` and
`/tmp/dali-scene-identity-cases-final.log`.

All eight scene identity controls produce their intended behavioral REDs (163 s,
`/tmp/dali-scene-identity-controls`). Restored acceptance passes **555/555 native
tests** across eleven binaries (one opt-in browser test skipped), scoped strict
Clippy, Pi library/four-bin check, doctests (none), changed-file formatting,
affected control plans, wait validation and whitespace. Native execution is
51.875 s; the native command takes 95 s. Logs:
`/tmp/dali-scene-identity-acceptance`. Both sequential frozen-source reviews
APPROVE with no actionable findings, covering implementer/correctness and
reader-completeness/parts-ledger lenses. They are same-family/source-only;
cross-family independence and runtime review are not claimed.
No browser source changes or physical HAT validation are part of this slice.
See the [slice design](superpowers/specs/2026-10-22-dali-v2-p5h-scene-identity.md)
and [metadata contract](dali-scenes-contract.md#named-slot-metadata).

## P5h1 — native scene protocol and physical model

Accepted locally at frozen source `2e46c31`: **546 native tests** and both
sequential source reviews pass. Managed scene configuration/UI remains next.

Implemented checked sixteen-slot scene frames, explicit repeated Store/Remove,
Off versus MASK replies, stored colour report plans and nonactivating live/stored
Tc/xy/RGBWAF staging. The independent model now has scene colours, separate
temporary/report registers, RGBWAF control and auto/manual recall. Store consumes
temporary values while masked type preserves previous stored colour. Remove and
base Reset clear colour participation. Ignored scenes preserve pending colour.

Both sequential design reviews approve (45 and 96 seconds, same-family/source-only).
All six initial literal model tests fail behaviorally before production changes.
The focused run passes 32 new/existing scene and colour cases; the standalone
PTY test also passes actual typed programming/query/manual recall/Remove with the
unchanged one-terminal repeat policy. A seventh physical test first fails on
control-only RGB staging retaining Tc type, then passes after selecting RGBWAF.
The initial scene-filtered run passes fourteen cases, including one existing base
case; thirteen tests are new.

The initial fifteen scene controls and three affected older controls have intended RED
evidence. Two older anchors were already ambiguous at the baseline; scope them
to their owning methods. One first retarget used the wrong input path and stayed
GREEN; the corrected tested `intervening_frame` path is RED. Logs:
`/tmp/dali-scenes-controls` and `/tmp/dali-scenes-controls-final`.
Initial restored acceptance passes **544/544 native tests** (eleven binaries; one opt-in
browser case skipped), scoped strict Clippy, Pi library/four-bin compile,
doctests (none), changed-file formatting, all eight affected control plans,
whitespace and wait validation. Native execution is 54.137 s; suite command wall
time is 91 s. Logs: `/tmp/dali-scenes-acceptance`. Source reviews follow the
frozen implementation. Managed scene identity/jobs and the Scenes screen remain
unbuilt; broader DT8 settings/physics and hardware validation are still pending.

First frozen-source review requests one correction (55 s): selected Tc with MASK
must become active while preserving its numeric value. A cross-family test fails
before the fix; the corrected test covers all six family changes in automatic
and manual modes. All sixteen focused scene/old-Tc cases and the new sixteenth
scene control pass their declared checks; two affected old Tc/cleanup controls
remain RED. Correct the two older documentation claims too. Logs:
`/tmp/dali-scenes-masked-family-red.log` and `/tmp/dali-scenes-review-controls`.
Restored regression and source rechecks follow.

The first reviewer's corrected-source recheck approves (10 s). The second full
review requests one correction (55 s): actual RGBWAF control must report links to
unsupported channels as zero. A three-channel literal case first fails with BF
instead of 87. Normalize supported links while preserving mode bits and MASK;
the regression varies one through six channels and checks recall, direct query,
actual report and subsequent masked control. Seventeen focused cases pass; the
seventeenth scene control and two affected RGB/control controls have intended
REDs. Logs: `/tmp/dali-scenes-channel-links-red.log` and
`/tmp/dali-scenes-links-controls`. Full restored verification and both final
source rechecks follow.

Final frozen `2e46c31` is approved by both sequential source rechecks (10 s and
3 s), with no remaining material findings. All **seventeen current scene
controls** have intended RED evidence, including rerun affected RGB controls.
Restored final acceptance passes **546/546 native tests** (eleven binaries; one
opt-in browser case skipped), scoped strict Clippy, Pi library/four-bin compile,
doctests (none), changed-file formatting, all eight affected control plans,
whitespace and wait validation. Native execution is 53.877 s; command wall time
is 98 s. Logs: `/tmp/dali-scenes-links-acceptance`. Reviewers are same-family and
source-only; the parent ran verification. No physical firmware or managed
scene/UI/Store acceptance is implied.
See the [scene contract](dali-scenes-contract.md) and
[slice design](superpowers/specs/2026-10-22-dali-v2-p5h-scenes.md).

## P7 — embedded browser application

Implement the approved responsive Preact/TypeScript interface in `web/`, with
fixed embedded sign-in/application assets, exact npm dependencies/lock and a
byte-for-byte rebuild check. Existing sessions protect application pages/assets;
only sign-in/identity are public. Runtime and Cargo builds need no Node server.
Screens provide overview/bus setup, compact Devices, Groups, Settings and
Diagnostics through the existing bounded jobs. Scenes, full reassignment and
lost-address restoration remain explicitly unavailable in this increment.

SSE is the sole view authority. Decode the existing `inventory.Inventory` State
envelope; late HTTP admission/job/session results cannot replace newer views or
restore closed authentication. Original dirty item tokens and replayed continuing
ledger keys protect inline saves/replacements. Checkmark/Enter/blur deduplicate;
queued explicit commits prepare their controller revision once. Successful rename
refreshes actual values; failed saves retain the draft and Blink. Batch discovery,
group pattern assignment and verification support progress/cancellation and stop
on unrelated revision/bus changes. Missing/full-capacity/unconfirmed results stay
explicit; no automatic write replay or UART policy change.

Two design gates approve (75 and 55 seconds, same-family/source-only); the first
required the HTTP/SSE ordering rule and replacement identity coverage. The initial
Rust route test fails before asset serving. Real browser acceptance first finds
the actual Inventory envelope mismatch; decode it and pin the Rust/TS contract.
Fixture corrections use managed stop, short private macOS socket paths, native
dialog/enabled-control waits and actual DT8 state rather than its setup profile.
Chrome/Playwright passes phone 390×844 and desktop 1440×1024 through **308 real
owned commands** with private MQTT/PTy/disk checks and inspected screenshots.
Includes naming/Blink restoration, failed-save retry, live input during drafting,
power/brightness/DT8, patterns, group drift adoption/repair, removal/rediscovery,
inventory export and logout. No runtime/CSP console errors. Emulator hotplug and
physical/HTTPS/live Store deployment remain unverified.

Fourteen frontend tests pass, including delayed POST/GET, retired stream/session,
safe metadata, atomic rename swaps/replacement and queued item authority. Thirteen
current controls have intended RED evidence. One broad decoder mutation stranded
an unrelated setup; the runner was interrupted and restored source, then rows were
targeted individually. One old-source guard stayed GREEN because snapshot epochs
already protected snapshots; add native-action/retired-error cases and rerun the
five affected controls, all RED. Logs: `/tmp/dali-browser-controls-final.log` and
`/tmp/dali-browser-controls-client-final.log`.

Restored acceptance passes **531/531 native tests** across 11 binaries, with the
explicitly opt-in browser test skipped (execution 51 s, command wall 90 s), all
fourteen frontend tests, build/reproducible dist, scoped strict Clippy, Pi library
and all four binaries, doctests (none), changed-file format, all affected control
plans, whitespace and wait validation (zero findings). Logs are in
`/tmp/dali-browser-acceptance` and `/tmp/dali-browser-wait-final.log`.
The first source gate requested two practical corrections (about 90 s): queried
Part 302/304 sensors used different fields from native events, and Sign out was
incorrectly blocked by foreground busy state. Both reproduce in real browser
tests before fixes. Render exact hex/resolution (plus normalized Part 302 position)
and give logout its own deduplicated path with later-batch cancellation and honest
failure reporting. Fourteen frontend tests and the 308-command browser run pass;
new query-bit and busy-logout controls have intended REDs. The first query mutation
hit the earlier leading-zero assertion; order the wide-value assertion first and
rerun the five model controls, all RED. Logs:
`/tmp/dali-browser-sensors-red.log`, `/tmp/dali-browser-logout-red.log`,
`/tmp/dali-browser-review-fix-functional-final.log`,
`/tmp/dali-browser-review-fix-controls.log` and
`/tmp/dali-browser-sensor-controls-final.log`.
Frozen `08abf86` is approved against `0eb776b` by both sequential source reviews.
Gate 1 initially requested the two corrections (about 90 s), then approved their
focused recheck (20 s); gate 2 approves the full source (98 s), with no material
findings. Both are same-family/source-only; the parent separately executed tests
and inspected browser screenshots. Browser evidence is recorded separately in
`/tmp/dali-browser-review-fix-functional-final.log`, with screenshots under
`/tmp/dali-browser-c5976b6a2974c740cfd0f272e0f118d7`. See
[operator guide](web-access.md), [build/test guide](browser-development.md) and
[slice design](superpowers/specs/2026-10-22-dali-v2-p7-browser.md).

## P5g — output identification

Blink/Stop uses one service-owned polled future and short existing basic output
operations. Fresh stable prior-level capture, immediate maximum/off pulses,
actual restoration, continuing Origin and original physical stamp protect the
inline rename workflow. Metadata waits fairly within its original deadline;
foreground writes to the identified Origin refuse at submission and owner.
Snapshots expose bounded public status. Two-minute expiry restores once; parent
teardown drops the future before HAT close without a new recovery policy.

Both design gates approve (117 and about 55 seconds, same-family/source-only).
The first corrected a real DT8 contract error: ordinary power commands can
activate already-staged auto-enabled colour. Identification issues no explicit
colour/settings commands; it does not promise unchanged staged/actual colour.
Two initial PTY/MQTT tests fail behaviorally on missing Blink. Fourteen focused
tests now pass, including real HTTP jobs, stable-level refusal, staged-Tc
activation, rename/failure/no-op, fair permit admission, original stamp, identity
replacement, paused-time expiry/cancellation and actual clamped restore results.
The new literal peer initially lost its retained PTY slave through precise async
capture; retaining the entire PTY inside its task fixes the test fixture.
Final-awaiter review reproduced two additional behavioral failures: stale ready
capture could begin mutation, and rejected restoration could leave active status.
Reuse the existing pure snapshot acceptance after capture/pulse/restore and route
restoration rejection through terminal cleanup. Both new regressions and all
fourteen focused tests pass. Timer priority precedes status traffic in dwell.
The first twelve-control run gives eleven intended REDs and one WRONG-REASON:
removing capture preflight is caught by the physical-mutation assertion after
post-write readback still refuses the command. Inspect the exact mutant output,
correct its expected assertion text and rerun that row: intended RED (9 s). All
**12 current controls** now have named RED evidence; historical diagnostics are
retained. Logs: `/tmp/dali-identification-controls.log`,
`/tmp/dali-identification-capture-mutant-output.log` and
`/tmp/dali-identification-capture-control-final.log`.

Restored acceptance passes **530/530 native tests** across 11 binaries (execution
54 s, wall 102 s), Pi library/all four binaries, doctests (none), changed-file
format, all 24 affected control plans, whitespace and wait validation (zero
findings). Strict Clippy initially flags only `on=!on` spacing; correct it to
`on = !on` without a behavior change and rerun the affected lint/format checks.
Logs: `/tmp/dali-identification-acceptance` and
`/tmp/dali-identification-lint-restored` (strict Clippy/format/plans/whitespace pass).
Frozen `31bb543` is approved by both sequential source gates against `758b0d2`,
with no material findings (205 and 99 seconds). A provisional missing-token
concern was withdrawn after checking the complete writing=true resolver guard;
no change was needed. Both reviews were read-only/source-only and same-family,
with no tests rerun. P7 subsequently supplies real browser acceptance; physical
fixture acceptance remains pending. See
[operator guide](identification.md) and
[slice design](superpowers/specs/2026-10-22-dali-v2-p5g-identification.md).

## P6c — actual scalar values and live input/job activity

Extend HTTP jobs with individual apply_output/read_output/read_input, reusing the
same atomic scalar intent parser, 2/4/6/8-second budgets, typed physical ownership
and latest-snapshot acceptance as MQTT. Shared pure input acceptance preserves
newer native values over an already-ready query. Complete snapshot states carry
actual values/quality and original generations, with unknown/stale clearing.
The HTTP budget grows to 4 MiB; MQTT inventory remains 512 KiB. A dense valid
model measures 522,331 inventory bytes and 1,054,621 HTTP bytes for 1,824 initially
unobserved scalar states, demonstrating the real 1 MiB boundary.

SSE adds complete jobs, native input actions with exact Notice::current receive
fencing, and explicit loss followed by a current snapshot. One pending notice/
loss plus dirty flags and a three-kind cursor prevent snapshot/job/input traffic
starving one another. Recheck authorization before each emission; no passive
renewal, action replay, detached producer, SDK/MQTT schema or UART policy change.

Both sequential design reviews approve (about 50 and 47 seconds, same-family,
source-only); the first requested the simultaneous-ready fairness regression.
Initial HTTP regressions fail before implementation on missing observations and
native SSE. The native test's nested equal timeouts initially reported its inner
wait rather than outer named criterion; shorten the outer test oracle. Thirty-
nine focused tests pass before the loss-conversion consolidation. These include
all colour families, real actual/quantized values, exact 255-bit inputs, physical
preflight, reads resumed after rename/newer native evidence, ready-source fairness,
lag, receive cutover, busy-stream expiry and real running/completed job updates.
Two new test assumptions were corrected against existing producers: compare
device state without the advancing emulator clock, and xy raw/65536 units.

The first 17-control run produces 16 intended REDs and one GREEN: an await-side
loss conversion is masked by the ordinary snapshot notification before native
prefetch. Remove the duplicated conversion in favor of one InputEvent::receive
used by both paths, and replace the two conversion controls with one shared-rule
control. The affected stream tests pass; the new shared control is RED (10 s).
All **16 current observation controls** now have intended behavioral RED evidence.
The initial GREEN remains in its historical log/ledger; it is not counted as a
successful control. One reanchored metadata control is RED (14 s), and the two
updated baseline SSE revocation/expiry oracles remain RED (30 s).
Logs: `/tmp/dali-web-observation-controls.log`,
`/tmp/dali-web-observation-shared-loss-control.log`,
`/tmp/dali-web-observation-reanchored-control.log`,
`/tmp/dali-web-observation-sse-controls.log`. Restored acceptance passes
**516/516 native tests** across 11 binaries (four processes; execution 52 s,
wall 98 s), scoped strict Clippy (9 s), Pi library/all four binaries (4 s),
doctests (none), changed-file format, affected control plans, whitespace and
wait validation (zero findings). The registry initially needed explicit
"on expiry end the stream" wording; its generated waiter now names event_stream.
Logs: `/tmp/dali-web-observation-acceptance`.

Frozen `f4501ec` is approved by both sequential source gates against `fe7c30d`,
with no material findings (67 seconds each). Both reviews were read-only,
source/test-only and from the same model family; they reran no checks.
Browser workflows and physical HAT validation
remain pending. See [web access](web-access.md) and
[slice design](superpowers/specs/2026-10-22-dali-v2-p6c-web-observations.md).

## P6b — authenticated configuration and commissioning jobs

Add strict POST `/api/jobs` and protected list/result routes. Preparation reuses
the MQTT command parser and shared concrete resolvers, preserving the client's
generation/revision and original 2/10/18-second deadline. Execution calls typed
service paths without an SDK command completion or publication bypass. Explicit
HTTP trace metadata, including absence, crosses the queue. Reauthorize after
JSON upload before preparation/admission; polling never renews a session.

Four bounded command futures live inside the existing Server task; no detached
job task, retry, UART journal, SDK change or new physical owner. Keep 32 admission-
ordered process-local records, evicting completed entries only, with safe codes,
sanitized free-text reasons and a 32 KiB whole-result limit. HTTP disconnect/logout
does not cancel admitted work. Service stop drops its receivers under the existing
shared deadline, preserving interrupted-save uncertainty. Existing structured
partial outcomes remain visible; 202 and a succeeded report do not promise full
physical success. Output apply/read, observations/events and Preact screens follow.

Both sequential design reviews approve (77 and about 40 seconds, same-family,
source-only). The first review's explicit trace-extension detail is incorporated.
The first HTTP metadata regression fails with missing route before production;
the companion absent-route upload setup timeout is not counted as auth evidence.
Initial compilation needed an owned String result annotation; one emulator test
corrected its expected existing group outcome from programmed to verified.
Twenty-eight focused web/facade tests pass; the final suite also exercises a
command with a broker endpoint that never sends CONNACK. Real socket/emulator
tests exercise metadata persistence/shared
MQTT publication, stale authority/revision, strict parameters, slow-upload logout,
input/output assignment, real-instance discovery, physical groups, safe save
failure/uncertainty/recovery, trace continuity, bounded records/futures and stop
while an incomplete HTTP upload drains. No >5-second physical timing measurement
is claimed; original-deadline and disconnected-request ownership are tested
independently using the current emulator's available timing seams.

All **13 new fault controls** detect their declared behavioral failures (2m36s),
then restore source. The affected control-plan sweep found one preexisting stale
unknown-property anchor after colour parsing moved; re-anchor its actual guard
and its repaired row is RED (12 s). Other existing rows pass plan checks.
Restored acceptance passes **505/505 native tests** across 11 binaries (four
processes; 92 s wall clock), scoped strict Clippy (10 s), Pi library/all four
binaries (3 s), doctests (none), changed-file formatting, affected control plans,
whitespace and wait validation (zero findings). Wait lint initially identified
the test observer's inline lock and the prepared-operation await; isolate the
test-only observer and explicitly register the existing request deadline.
No production timing/policy changed. Logs: `/tmp/dali-web-job-controls.log`,
`/tmp/dali-web-job-reanchored-control.log`, `/tmp/dali-web-job-acceptance`.
Both sequential source gates approve frozen **d2f6828** against **df3e924**,
with no material findings (82 seconds and 2m40s). Reviews were read-only,
same-family/source-only and ran no verification or mutations. Browser and
physical hardware validation remain outstanding. See [web access](web-access.md) and
[slice design](superpowers/specs/2026-10-22-dali-v2-p6b-web-commands.md).

## P6a — authenticated HTTP foundation

Implement the independent API/lifecycle foundation while remaining P4/P5 actions
stay open. Add optional Axum server owned by DaliService, explicit startup
password/origin, fixed-size verifier and cookie sessions, Origin/CSRF gates,
protected public-inventory/status snapshots and pull-based SSE. Idle expiry is
30 minutes, absolute 12 hours; explicit user activity alone renews inactivity.
No URL/Bearer auth, persisted sessions, default password, SDK or recovery change.
At this historical boundary P7 screens and mutation jobs were still pending;
P6b/c and P7 subsequently add those paths and replace `/login` with sign-in HTML.

Design gate 1 found raw credential retention in the SDK's Debug Settings; consume
the option and strip separate/inline password arguments before Bridge composition.
Both sequential gates approve the amended design. Cached Axum source showed
outer-task abort does not own its spawned connections: accepted IO now carries
forced cancellation and a permit, with actual-drop accounting at close. Share
the existing startup and absolute cleanup ceilings; no second daemon/framework.

The first process regression fails at its missing-password preflight assertion
before production. Initial compile-only module-path failure is not behavioral
evidence. Early socket tests catch URI parsing accepting a nonnumeric port;
explicit authority-suffix validation fixes it. Fifteen focused tests pass before
the added standalone password-verifier control test: direct routes, origin/CSRF,
cookie-only/secure flags, rate/cap/rotation, idle/absolute expiry, live logout,
latest inventory, occupied/expired bind, broker-without-CONNACK usability,
actual managed stop, incomplete-upload forced cleanup and canonical CLI/environment
credential privacy. New dependencies preserve existing futures/tokio-util lock
versions rather than upgrading their graph incidentally.

All **19 behavioral controls** fail at their declared assertions after restoration.
The first stream-cap control was WRONG-REASON: the test helper waited for an
incorrectly accepted infinite response body before asserting status. A captured
diagnostic confirms that timeout. Assert response headers first; the corrected
control is RED. Five additional password/privacy/cookie/upload controls are RED.
Logs: `/tmp/dali-web-controls.log`,
`/tmp/dali-web-stream-control-diagnostic.log`, `/tmp/dali-web-additional-controls.log`.

Restored acceptance passes **492/492 native tests** across 11 binaries (four
processes), scoped strict Clippy, Pi library/all four binaries, doctests (none),
changed-file format, whitespace, control plan and wait-registry validation (zero
findings). Logs: `/tmp/dali-web-acceptance`. The wait helper was initially invoked
from the bridge checkout and inherited its Pi target; rerunning from its native
SDK checkout succeeds. No SDK source or production runtime change resulted.
Both sequential read-only source gates approve frozen **70afa696** against
**d46cb812**, with no material findings (76 and about 70 seconds). Reviews were
same-family/source-only and ran no verification or mutations. Browser and
physical hardware acceptance remain outstanding.
An isolated headless Chrome probe opens about:blank at phone
viewport, confirming local Playwright capability; no feature/browser pass claimed.
See [operator/API guide](web-access.md), [design](superpowers/specs/2026-10-22-dali-v2-p6a-web-foundation.md)
and `v2-web-controls.toml`.

## P5f — one unaddressed input-module assignment

Extend the existing owned commissioning flow with explicit output/input address
space, pure Part 103 frames and an independent input addressing emulator.
`assign_next_input` uses strict controller authority, raw short addresses,
two ordinary 24-bit exchanges under one fixed 100 ms pair deadline and a 200 ms
randomisation allowance. The 16-bit repeat profile remains output-only. Preserve
the original stamp, permit and command completion through search/save/program.
Save a fresh empty InputN module before PROGRAM, then verify/query the raw address
and require fresh status. Separate addressed discovery fills real instance types
under the same module identity. No SDK, quiet recovery or UART journal change.

Both sequential design gates approve, with no material findings. The first MQTT
test failed with unknown command before production; the independent emulator
literal test failed before model wiring. Fifty focused tests pass, covering all
six random-address boundaries, raw short addresses 0/63, range versus exact
collision/invalid replies, one shared pair deadline/no retry, randomise readiness,
initialization expiry, native event handling, cancellation/held/failed/uncertain
saves, bus cycles during save, metadata-only recovery, strict payloads/authority,
saved/occupied slots, empty reservations and separate real-instance discovery.
An independent consumer preserves existing intent/module identity and refuses
pre-discovery generations for new children. The save-cancellation fixture was
corrected to expect the existing Uncertain phase and explicitly retry recovery
while the released disk worker finishes; no production recovery change.

All ten new behavioral fault controls and four re-anchored shared commissioning
controls fail at their declared assertions, then restore source. Full acceptance
passes **475/475 native tests** across 11 binaries (four test processes), scoped
strict Clippy, Pi library/all four binaries, doctests (none defined), changed-file
formatting, wait registry, whitespace and both control-table plan checks.
Logs: `/tmp/dali-input-assignment-controls.log`,
`/tmp/dali-input-shared-controls.log`, `/tmp/dali-input-assignment-acceptance`.
Both sequential read-only source gates approve frozen **f016463** against
**29ccd37**, with no material findings (about one minute and 45 seconds).
Reviews used the same model family and ran no verification or mutations.
Physical I4/fixture timing remains unverified. Full bus
readdress/restore, identification and browser orchestration remain unfinished.
See [commissioning](commissioning.md), [design](superpowers/specs/2026-10-22-dali-v2-p5-input-assignment.md)
and `v2-input-assignment-controls.toml`.

## P5e — per-fixture group membership

Implemented saved-group creation and strict MQTT/typed APIs for captured reads,
single-bit assignment, reviewed hardware adoption and explicit full-mask repair.
Reuse existing group reducers and the common physical owner. Assign saves desired
intent before programming only the selected bit; adopt changes metadata only;
repair can explicitly clear unnamed slots. Report captured halves, quality,
staleness, expected/drift masks and confirmed revision/partial outcome. Preserve
the original stamp, permit and command completion across save/program. Group
membership changes rotate only affected group contracts; fixture identity and
intent continue. Read-class dispatch precedes mutation completion capture and
supports absent MQTT generation/trace headers. No SDK or UART recovery change.

Both sequential design gates approve; the first noted the Read-class completion
seam, now implemented. The initial two MQTT regressions failed with unknown
command before production changes. Local tests cover literal boundary frames,
FFFF and incomplete/collision masks, both known repeat counts, strict payloads,
create/rename/no-op, preservation of unnamed bits/other fixtures, adoption/repair
review, physical mismatch/partial failure, held/failed/uncertain/canceled saves,
bus changes through save, cancellation/expiry/native input and independent
consumer continuity. A literal peer initially captured only its master descriptor;
moving its whole PTY owner into the task fixes that fixture. The native-input
assertion initially used a target-specific topic; actual events carry target in
the controller-topic payload. Expiry tests distinguish request settlement from a
late terminal: admission can release after backend retirement while UART readiness
still refuses new work. Their local retry pace is shortened to fit the fixture's
bound; production's existing five-second recovery pace stays unchanged.

The first ten fault controls produced nine declared RED outcomes; the save-time
bus-cycle case stayed GREEN when the original-stamp guard was removed because
another receive-epoch check still refused programming. A focused already-observed
cycle handoff test isolates that admission boundary. An additional adoption test
checks stale evidence after confirmed save. Final control/restored/source-review
results are recorded below once run. Browser/pattern/bus-wide group orchestration,
group light/color operations and physical I4 validation remain unfinished.

The first restored gate passes 462 native tests, scoped strict Clippy, Pi
library/all four binaries, doctests (none), changed-file format/wait/whitespace.
Self-review then identified that a correlated group-setting collision was
classified as UART stream loss. A fail-first regression checks immediate link
usability and explicit readback after partial repair. Treat that valid bus terminal
as an operation failure, preserving the ready UART and saved intent without retry.
No change to transport ordering/recovery is required.

The next full run passes all group tests and 461/462 overall; an unchanged
atomic-intent test observes level 64 after its invalid request sequence. It passes
in isolation. Inspection finds its loop waits for one new Error, although one
rejected map reports one Error per property and pending Desired values coalesce.
Move its existing worker-order refresh barrier inside the loop, so each invalid
complete map finishes before submitting the next, and assert both errors from the
first two-property rejection. No SDK or output-routing code changes. Repeated
focused and restored results follow before source acceptance.

The corrected atomic-intent test passes **20/20 stress iterations**. Final
restored acceptance passes **462/462 native tests** across 11 binaries with four
test processes, scoped strict Clippy, Pi library/all four binary compilation,
doctests (none defined), changed-file formatting, wait registry and whitespace.
All **12 new behavioral controls** and **3 affected activation/reply/input controls**
are RED at their declared assertions. Logs: `/tmp/dali-group-controls.log`,
`/tmp/dali-group-additional-controls`, `/tmp/dali-group-collision-control.log`,
`/tmp/dali-groups-restored-gate`, `/tmp/dali-groups-final-gate` and
`/tmp/dali-groups-acceptance-gate`. The middle final-gate log retains the
461/462 test-synchronization failure; the acceptance log is the corrected result.
Both sequential read-only source gates approve frozen **88656fe** against
**c9fc2b3**, with no material findings. Reviewers ran no tests or mutations;
same-family review and absent physical I4/fixture validation remain explicit
limits. Review learning: distinguish per-property errors from whole-request
completion, preserve bus collision versus UART-loss scope, and isolate an
authority guard when another guard masks its removal.

See [operator/API guide](groups.md),
[design](superpowers/specs/2026-10-22-dali-v2-p5-groups.md) and
`v2-group-controls.toml`.

## P5d — explicit Remove from list

`remove_from_list` exposes existing `Edit::RemoveOutput` / `RemoveInput` through
thin metadata/proxy/item-authority mappings. It removes saved fixture lifetimes,
including all instances of a selected input module, and preserves hardware
addresses, levels, scenes, physical groups and input configuration. Output removal
cleans saved group references; continuing group identity remains stable while
membership binding tokens rotate. Original MQTT reply metadata survives deleting
the selected target. Unknown payloads/unsupported targets and stale revision/item
authority are refused. Rediscovery creates fresh lifetimes; consumers retire old
intent rather than transferring it by address/name. Store Home equipment handling
remains P8 work.

Both sequential design gates approved with no material findings. The first real
MQTT regression failed with unknown command before production changes. Seven new
tests and the existing explicit-removal consumer regression pass: output removal
and State retirement, physical output/input preservation, same-address rediscovery,
input subtree/queued-action retirement with continuing output 0, strict payloads,
item/controller token distinction, held/failed/uncertain saves and explicit
recovery, and independent-consumer identity/intent continuity. No SDK, UART
recovery or new persistence machinery was needed. All **5 new behavioral controls**
and **3 affected identity/activation controls** fail at their declared assertions.
The restored gate passes **444/444 native tests** across 11 binaries with four
test processes, scoped strict Clippy, Pi library/all four binary compilation,
doctests (none defined), changed-file rustfmt, wait registry and whitespace.
Logs are `/tmp/dali-removal-controls.log`, `/tmp/dali-removal-affected-gate` and
`/tmp/dali-removal-restored-gate`. Both sequential read-only source gates approve
frozen `cc2b986` against `ff75fca`, with no material findings. Reviewers performed
no tests or mutations. Same-family review and absent physical I4/fixture evidence
remain explicit limits. Review learning: expose existing reducers through thin
entry points, and prove target deletion preserves the original command completion
while retiring every other use of that lifetime.

See [operator/API guide](remove-from-list.md),
[design](superpowers/specs/2026-10-22-dali-v2-p5-remove-from-list.md) and
`v2-removal-controls.toml`.

## P5c — first-install bus setup

`configure_bus` / `MetadataEdit::ConfigureBus` upserts an explicitly named physical
bus through the existing durable owner. New buses have no fabricated fixtures;
updates preserve saved children, descriptions, memberships, color selections and
Origins. Enable-only changes preserve child bindings, rename rotates descendant
bindings, and identical setup is a no-op. Committed enabled buses update live HAT
polling without restarting the process. Actual enabled membership is distinct from
the internal bus-0 health fallback. Disabled caches/listeners are retired; native
notices carry private receive stamps checked against the latest snapshot before
SDK reporting, so disable/re-enable cannot revive old actions. Asynchronous bus
health keeps its existing best-effort semantics, without a new freshness or UART
recovery framework.

Both sequential design gates approved after clarifying queued-event lifetimes and
removing an unnecessary fresh-health promise. The first real MQTT regression
failed with unknown command before production changes. Nine focused tests pass:
empty-install bus creation and nonzero-bus discovery without restart, strict MQTT
and service authority, model validation/no-op/preservation, queued press retirement
and fresh release after re-enable, held/failed/uncertain saves, owned stale health
helper cancellation and independent-consumer continuity through rename/reopen.
The held-save test initially attempted recovery before its released filesystem
worker finished; a bounded retry of the explicit worker-busy response fixes that
test race. All eight original mutation controls and three affected recovery/
completion/native-helper controls fail at their intended behavioral assertions.
The first restored gate passes 436 tests, scoped strict Clippy, Pi library/all
four binaries, doctests (none), changed-file rustfmt, wait registry and whitespace.
Self-review then identified an overly strict event phase guard: a queued valid
action must survive an unrelated save while the old model is still active.
A new fail-first regression reproduces that loss; allowing Writing while retaining
listener/stamp validation fixes it and preserves the disable/re-enable rejection.
The ninth control also fails at its intended assertion. The next full run passed
436/437 tests and exposed the other side of the held-save fixture race: Uncertain
phase can be published just before its canceled owner releases the global permit.
The test now retries that explicit Busy response under the same original bound,
as well as the worker-busy response; the focused recovery test passes. Production
strict Clippy, Pi compilation, doctests and formatting passed that run unchanged.
The subsequent default-concurrency run passed all new bus tests but missed two
unchanged CLI startup tests' 2 s deadlines. Both pass in isolation; the complete
suite with four concurrent test processes passes **437/437** in 39.5 s. This
supports test contention as an explanation, not a proven host-level diagnosis;
no startup code or existing deadline changed. Logs are
`/tmp/dali-bus-setup-restored-gate`, `/tmp/dali-bus-setup-final-gate`,
`/tmp/dali-bus-setup-acceptance-gate` and
`/tmp/dali-bus-setup-bounded-concurrency-gate`. All nine new controls and three
affected controls are RED at their declared behavioral assertions; logs are
`/tmp/dali-bus-setup-controls.log`, `/tmp/dali-bus-setup-writing-control.log`,
`/tmp/dali-bus-setup-affected-controls.log` and
`/tmp/dali-bus-setup-event-control.log`. The final explicit original-correlation
assertions pass in all nine focused bus tests; changed-file format/whitespace
also pass. Both sequential read-only source gates approve frozen `c461bb2`
against `fea28ef`, with no material findings. Reviewers ran no checks or mutations;
same-family review and absent physical I4/fixture validation remain explicit
limits. Review learning: keep unchanged identity separate from receive lifetime,
check the event predicate during unrelated saves too, and avoid inventing a
health-freshness framework for the owner's accepted best-effort bus behavior.

See [bus setup guide](bus-setup.md),
[design](superpowers/specs/2026-10-22-dali-v2-p5-bus-setup.md) and
`v2-bus-setup-controls.toml`.

## P5b — one unaddressed output assignment

`assign_next_output` searches only unaddressed outputs, skips saved/missing and
physically occupied addresses, commits a fresh named reservation before PROGRAM,
then verifies its programmed short address and fresh status. Search/save/program
retain one permit. The continuation uses the original captured physical stamp
and a fresh private target generation while the MQTT reply retains its original
token/trace. Physical failure after confirmed reservation returns structured
reserved_but_unconfirmed; disk/CAS uncertainty retains existing freeze/recover.
No current light values or automatic retry/resume are invented.

The daemon exposes existing RepeatPolicy through
`--uart-repeat-terminals unverified|1|2`, default unchanged. Unknown profiles
refuse commissioning before the sequence; both explicit counts are tested with
independent literal peers. Hardware profiles remain unverified. Cleanup stays
within active helpers; an abandoned between-phase initialization uses its standard
timer or the next initial Terminate, without a cleanup job or UART journal.

Both sequential design gates approved after clarifying structured partial
outcomes and narrowing cleanup to the executor that can perform it. The first
MQTT test failed with unknown command before production changes. The implemented
test assigns a third unaddressed fixture to free address 2 while preserving
physical 0/1, input 0, existing names/groups and one original-context reply.
Local tests also cover the full search boundaries, both terminal counts,
exact/range collision distinctions and invalid replies, saved/physical capacity,
strict admission and CLI arguments, deadline/cancellation/native events,
save failure/recovery and save-time bus cycles, programming shutdown, failed
post-write queries and independent-consumer continuity after reopening the saved
reservation. A CLI fixture reused retained inventory as a startup signal;
fresh private brokers per accepted launch fixed that test race.
All **8 new mutation controls** and **2 affected metadata controls** fail at
their declared behavioral assertions. Logs are
`/tmp/dali-commissioning-controls.log` and
`/tmp/dali-commissioning-affected-controls.log`. Restored checks and frozen
source-review evidence follow before this slice closes. The initial restored
run passed 427 tests and failed only its wait checker: an internal `send`
wrapper collided with the checker's channel-send method name. Renaming it to
`exchange` preserves execution and the existing tagged leaf transaction wait;
the corrected acceptance run is in `/tmp/dali-commissioning-final-gate`.
That final gate passes **428/428 native tests**, scoped strict Clippy, Pi
library/all four binary compilation, doctests (none defined) and whitespace.
Changed-file rustfmt also passes. Both sequential read-only source gates approve
frozen `e05ea13` against `e2ed360`, with no material findings. Reviews ran no
verification or mutations; same-family and absent physical validation remain
explicit limits. Review learning: put post-commit partial outcomes in the public
type and promise cleanup only where an executor still owns it.

Input addressing, full reassign/restore, identification controls, group/scene
workflows and the protected browser UI remain unfinished. See
[operator/API guide](commissioning.md) and the
[slice design](superpowers/specs/2026-10-22-dali-v2-p5-unaddressed-output.md).

## P5a — addressed discovery

One bounded `discover_address` operation probes an output or input module and
durably adopts verified additions with default names. Existing names, Origin,
binding generations, missing items and smaller saved input lists are preserved.
New input instances receive their own child lifetimes. Full output multi-type
enumeration uses 254 termination and preserves incomplete prefixes. Input
discovery retains unknown valid types and empty-module metadata.

The physical permit spans queued work, actual settlement and durable save.
Original MQTT completion survives controller generation rotation; repeat scans
are no-ops. Captured `probe` reports never create current fixture availability.
Save-time hardware disappearance leaves confirmed additions reserved. Existing
uncertain-save recovery is reused; no UART journal, outage retry or SDK change.

Both sequential design gates approved after clarifying the 254/255 distinction
and explicitly historical probe semantics. The first real MQTT regression failed
with unknown command before production changes and passes after implementation.
Focused local tests cover literal full/malformed type sequences (including more
than nine types), missing/collision/partial input evidence, both address spaces,
common/unknown/zero input instances, child expansion and independent-consumer
continuity, type mismatch/smaller counts, strict requests/retired authority,
native events during cancellation, permit retention, physical epoch loss and
held/noncommitted/uncertain saves. A literal peer initially lost its PTY slave
through Rust field capture; retaining the whole PTY fixed that test fixture.
All **7 new mutation controls** and **2 affected metadata controls** fail at
their intended behavioral assertions. The restored gate's 411 behavioral/native
checks passed, with one registry failure: the new owned `discover` wrapper needed
an explicit local-method declaration. After that registry-only correction,
the complete suite passes **412/412 tests**. Scoped strict Clippy, Pi library/all
four binary compilation, doctests (none defined), changed-file rustfmt and
whitespace pass. Logs: `/tmp/dali-discovery-controls.log`,
`/tmp/dali-discovery-affected-controls.log`,
`/tmp/dali-discovery-restored-gate` and `/tmp/dali-discovery-final-gate`.

Both sequential read-only source gates approve frozen `a180167` against
`af3648c`, with no material findings. Reviewers inspected source/test contracts
and ran no verification or mutations. Same-model-family review and absent
physical hardware evidence remain explicit limits. Keeping captured probe
semantics separate from current State and reusing the existing durable owner
held through both reviews; retain those boundaries for addressing work.

This is discovery of already addressed devices. Unaddressed assignment, complete
readdress/restore, identification controls, group/scene workflows and the approved
web UI remain future increments. No physical I4 or fixture evidence is claimed.
See [operator/API guide](discovery.md) and the
[slice design](superpowers/specs/2026-10-22-dali-v2-p5-addressed-discovery.md).

## P4c4 — current input readback

Saved input instances now query physical type, resolution and all value/latch
bytes through the same UART owner/permit as light operations. Exact 1..255-bit
values use canonical bytes/hex; contact/button/occupancy values are interpreted
only with compatible resolution/type. Missing/colliding/changed-type evidence
clears old State rather than inventing released/off. Instance and aggregate MQTT
refresh now query inputs, retaining the original aggregate ceiling/fair cursor.
Native events remain distinct; polling generates no actions, and a newer native
observation wins over a latched query. The independent emulator has optional
current-value profiles and explicit value controls without changing old scenarios.

Verified locally on 2026-10-22:

- The first real MQTT quiet-button refresh test failed before production changes.
  **399 native tests passed**, including 17 added cases for exact/padded/wide
  values, common types, independent latches, collision/missing/type mismatch,
  merged State clearing, original traces, newer native events, cancellation,
  rename, ready-reply expiry, final acceptance and managed shutdown.
- A 255-bit literal peer paced at 80 ms/reply passes the fixed 4 s MQTT budget;
  changing it to 2 s fails the intended completion assertion.
- **12 new fault controls and 8 affected existing controls** all fail at their
  declared behavioral assertions. Old anchors now reference shared operation
  names and the current aggregate budget path. No recovery behavior changed.
- Scoped strict Clippy, Pi library/all four binary compilation, changed-file
  rustfmt/whitespace, wait registry and doctests (none defined) pass. Logs:
  `/tmp/dali-input-readback-controls`, `/tmp/dali-input-readback-restored-gate`
  and `/tmp/dali-input-readback-final-gate`. The registry's missing explicit
  expiry wording was corrected before the final full-suite pass.
- Both sequential design and source gates approve frozen `5c485172` against
  `b3a72b9`, with no material source findings. Reviews inspected source/tests
  without running verification or mutating files; same-family limits apply.

Remaining input work: hardware discovery/addressing, event configuration,
feedback, unresolved group/source routing, calibrated units and physical device
validation. Native absolute/light events retain their existing ten-bit raw
meaning; full-resolution/Boolean contact projection comes from a query. No live
UART, production broker, Store, physical fixture or firmware validation is claimed.

## P0 — configuration boundary implemented

The library `src/config.rs` and development binary `mqtt_dali_v2 --check-config`
select a valid existing v2 first, otherwise import v1 into a separate file or
initialize an empty inventory. They preserve legacy bytes and names, addresses,
ordering and group membership. New files are private, size-bounded and published
atomically without replacing another creator's destination. Invalid v2 never
falls back. Synchronous filesystem work and diagnostic output run on detached
workers with bounded caller waits. This is a check command; daemon startup remains
legacy until P3 integrates the new service and promotes the executable.

Verified on 2026-10-21:

- Native full suite: **72 passed**, including 27 migration/library/process cases
  and the existing legacy regressions; all use isolated fakes, never the live bus
  or broker. The wait registry passes.
- Strict Clippy for library, v2 binary and migration process tests: passed with
  warnings denied. New Rust files pass rustfmt; `git diff --check` passed.
- Raspberry Pi `armv7-unknown-linux-musleabihf` library/v2 compile check: passed.
  This does not verify linking, deployment or physical hardware.
- Tests were observed failing before implementation. The full-output-pipe
  regression was also observed failing before its fix.
- Five reproducible negative controls all failed for their expected reasons;
  see `v2-migration-controls.toml` and its recorded ledger. They remove alias,
  no-follow, no-replace, output-deadline and worker-detachment protections.
- Separate sequential design/contract reviews and a fresh code review completed.
  The code review requested bounded diagnostics, stronger alias tests and an
  explicit default Cargo binary; all were fixed and reviewed again with an
  approval for P0. Available reviewers are from one model family; cross-family
  review remains unavailable.

Limitations: permission checks skip root; check output is best effort after
200 ms; directory ownership is trusted. Crash-during-write/retry is tested, but
power-loss durability and malicious directory replacement are not. Input, scene
and capability fields will extend the inventory in their owning phases; this
pre-release schema rejects unknown fields rather than discarding them.

Review learning: process-exit tests must include full unread output pipes as well
as stalled workers; path-safety tests must start with absent files, since an
unrelated parse error can otherwise hide a missing guard. No shared skill files
were changed during bridge implementation.

## P1a — typed wire, monitoring and serial substrate implemented

`src/atx/wire.rs` preserves bus, width, collision and version fields; its
incremental framer bounds storage and partial-line lifetime without discarding
idle events. `monitor.rs` owns one continuous reader and serializes byte writes
under caller deadlines. Subscribers get explicit lag, persistent lifecycle status
and error/EOF/panic notification. Cancellation during a possibly partial write
prevents further writes. Close cancels an active writer and releases both stream
halves even when client handles remain.

`serial.rs` uses nonblocking Unix descriptors with raw 19200/8N1 settings on both
Pi UART and desktop PTY. Opening/configuration runs on a detached worker under a
2 s caller deadline. A local PTY test checks actual terminal settings, idle input
events and exact outgoing bytes through the production adapter.

Verified on 2026-10-21:

- Full native nextest suite: **92 passed**, including 20 new wire/monitor/PTY
  cases and all 72 previous regressions. Tests and close/write-failure regressions
  were observed failing before their corresponding implementations/fixes.
- Strict Clippy for the library, v2 binary and config process tests: passed;
  Raspberry Pi library/v2 compile check: passed. Neither is a Pi runtime test.
- Six reproducible UART negative controls all failed for their expected reasons:
  line size/lifetime, write poisoning, close cancellation/resource release and
  raw mode. See `v2-uart-controls.toml` and its recorded ledger.
- Fresh source review requested descriptor release and EINTR handling. Fixes were
  reviewed again and approved for this substrate scope. EINTR retry was inspected,
  not fault-injected; physical UART teardown remains unverified.

Review learning: resource ownership must be tested after close while public
handles remain alive. Tests also distinguish canceling a queued write from
canceling a potentially partial write, since only the latter poisons admission.

The raw `write` receipt means only bytes accepted by the kernel. P1b supplies the
exclusive transaction owner above it.

## P1b — transaction dispatcher implemented

`Transactions` privately owns the monitor and a continuously running dispatcher.
Capacity-one admission covers queued and active work. The same reader fences old
buffered/partial lines; framing and the first accepted-write sequence boundary
share the split-stream lock. Explicit drain emptiness is distinct from retry
Pending. Async input events, other-bus replies and echoes cannot complete a query.
L bit 16 distinguishes commands from input events.

Fully written canceled/timed-out exchanges retain their remaining terminal count.
Corruption, lag, interference or a potentially partial write requires a later boot;
an arbitrary reply or version message cannot reopen admission. Initialization is
explicit. Unverified j/t completion is refused; verified/emulated repeat profiles
can declare one or two terminals. Close resolves an active caller and releases
the stream. No transaction result reports physical state confirmation.

Verified on 2026-10-21:

- Full native nextest suite: **118 passed**, including 46 ATX cases and all
  migration/legacy regressions. Production Serial plus Transactions exchanges
  actual bytes through a local PTY with interleaved input and other-bus traffic.
- Strict scoped Clippy passed; Pi library/v2 compile check passed; diff whitespace
  and Rust formatting checked. The wait registry passes after its required tags,
  local-method declaration and explicit expiry outcomes were completed.
- Six new negative controls all failed for their expected assertions: retry/empty
  confusion, incomplete-line boundary, caller and actor expiry, lost late slot,
  and unverified repeats. See `v2-transaction-controls.toml` and recorded ledger.
- Sequential design review restored a dedicated actor to keep idle events drained.
  Fresh code review found drain/first-write and ready-result deadline races; fixes
  and focused regressions were reviewed again and approved. Cross-family review
  remains unavailable. Physical HAT firmware behavior remains unverified.

Review learning: generic AsyncRead Pending may mean retry/cooperative yield rather
than an empty buffer. Correlation requires a stronger explicit drain contract.
Deadline tests must separately exercise actor completion and a caller resuming
after a ready result, since either check can hide a missing guard in the other.

## P2 — independent standalone ATX emulator implemented

`src/emulator` owns strict physical scenarios independently of bridge inventory,
with separate UART parsing/formatting and literal vectors. Its base gear model
supports levels/MASK, steps, limits, selected status/configuration queries, DTRs,
groups/scenes and commissioning address selection/search/program/withdraw. Model
time can be realtime or frozen/advanced; seeded randomization is reproducible.
Duplicate addresses affect all selected fixtures and conflicting replies collide.

`dali_hat_emulator` creates a private PTY and optional private Unix JSON-control
socket. It uses tracing-init, bounded detached startup/output workers, bounded
queues/timers/clients, persistent UART write offsets and independent transport
deadlines. Controls inject events/faults, inspect actual fixture state and simulate
address loss, bus power changes and HAT reboot. Reboot preserves fixture DTR and
initialization state and delimits truncated output. Both PTY descriptors are
close-on-exec and released before successful close.

Verified on 2026-10-21:

- Full native nextest suite: **146 passed**, including 24 emulator library tests
  and four standalone process tests, plus all 118 previous regressions. Real PTY
  tests use production Serial/Transactions and verify independent snapshots.
- Process checks cover private socket permissions, refusing existing paths,
  invalid/oversized/unfinished controls, and SIGTERM with full unread output.
  Model tests cover range versus exact selection, binary search, repeat windows,
  expiry, RESET retention/isolation and unsupported operation behavior.
- Strict scoped Clippy passed. Pi library/v2/emulator compile check passed.
  Wait registry and formatting passed. Cross-linking and Pi runtime remain P9.
- Six emulator negative controls failed for their declared assertions: repeat
  gating, exact selection, reboot delimiter, retired controls, timer bounds and
  partial-line lifetime. See the control table and recorded ledger.
- Sequential design/code reviews completed. Code findings for truncated-record
  recovery, mixed-width repeat invalidation and STEP UP/RESET semantics were
  observed failing in regressions, fixed and reviewed again with approval.
  A further two-fixture RESET test closes the review's nonblocking coverage gap.
  Reviews are source-only and from one model family; physical behavior unverified.

Limits: immediate direct/scene levels, no timed UP/DOWN/fade physics yet. DT8,
semantic inputs, emergency and memory/data models belong to P4. The synthetic
version/repeat profiles are not real-firmware evidence. Unix socket cleanup is
best effort after exit. Usage and capability boundaries are in `emulator.md` and
`support-matrix.md`. Browser emulator controls follow the protected HTTP phases.

Review learning: track UART record boundaries from accepted bytes across frames,
including deliberate faults; HAT reset must not reset separately powered gear.
Independent models need adjacent opcode/default tests as well as roundtrips.

## P3a — qualified target codec frozen and implemented

`src/targets.rs` provides typed construction/parsing for output/input/group/scene
targets and input instances, with a separate inventory route. Uppercase UTF-8
percent escaping preserves names exactly while keeping dot-separated targets
inside one MQTT topic segment. Strict canonical decoding rejects aliases and
invalid names; the 4096-byte encoded bound counts separators and namespaces.

Verified: four focused tests cover literal/collision/Unicode/metadata vectors,
invalid forms and exact 4096/4097-byte output/instance boundaries. Three meaningful
cases were observed failing before implementation. Three negative controls caught
removed canonicality, dot escaping and encoded-size guards. Separate design and
code reviews approved the slice; review additions cover lowercase valid-name
escapes and the exact instance bound. Full native suite: **150 passed**; strict
scoped Clippy and Pi compile checks passed. No MQTT/Store caller uses this codec
yet; complete topic/packet validation belongs to integration.

The remaining P3 work includes shared managed-service lifecycle, authoritative
inventory/request-generation fencing, the DALI service and canonical v2 daemon.
The managed-service design is grounded in current BridgeKit's run/start split;
it retains the single shared logging/signals/watchdog policy.

## P3b — managed process service implemented in BridgeKit

Shared checkout: `Projects/tracing-init-dali-v2`, branch
`feat/dali-managed-services`, commit `a3db09e` (local, unpublished).
`ProcessService`, `ServiceContext` and managed run/start APIs keep process-owned
resources independent of MQTT/device link lifetimes. Readiness validates the
proxy once; an independent heartbeat watcher closes admission after service
death. Startup cancellation retains cleanup, and all component teardown shares
one absolute deadline. Success confirms future, local-task and runtime-resource
destruction; blocked helpers/destructors cannot claim a clean exit.

Verified in the shared checkout: **160 kit tests** and **174 conformance tests**
passed, strict all-target Clippy for both crates passed, and the armv7 Linux musl
library compile check passed. Conformance used explicit `RUST_LOG=info`; the
inherited `warn` filter hid INFO in two existing log assertions. Six negative
controls detected their declared removed protections, and the restored full kit
suite passed. Design and fresh core reviews approved after readiness/admission,
runtime-teardown and absolute-deadline fixes. Reviews are source-only, same model
family; physical hardware remains unverified.

The bridge has not adopted this dependency yet. Portable dependency pinning is
required before daemon integration is final. Inventory reconciliation and stale
request fencing are the next P3 contract boundary. The shared spec and its
verification record are mirrored in `superpowers/specs/2026-10-21-dali-v2-p3b-services.md`.

## P3c1a — pure inventory/reconciliation layer implemented

`inventory::Model` validates complete schema-1 wire snapshots, initializes the
legacy schema-2 baseline, replays immutable named history and proposes atomic
edits. It distinguishes continuity from replacement, preserves semantic group
membership through address moves, and rotates dependent group fences on address
reuse. Revisions allocate above the private high-water value. Present null or
incomplete ledgers fail validation. Imports preflight canonical targets and the
whole snapshot before creating v2, with exact document/encoded-target bounds.

A separate consumer algorithm and literal wire fixture verify offline rename
chains, rename-back, swaps, removal/reuse, preserved device record ids, intent
continuity, old-generation observation rejection and atomic rejection of invalid
or partial documents. These are independent-consumer contract tests, not actual
Store migration/binding tests. The latter remain P8.

Verified: **164 full native tests passed**, six declared negative controls caught
their removed guards, and the armv7 library/v2/emulator compile check passed.
Scoped strict Clippy is part of the checkpoint gate. New behaviors were observed
failing first, including identical-text replacement and rename-back intent
continuity. Two sequential design reviews cleared this slice after uncertain
save, transition-chain and runtime-bypass refinements. Fresh source review found
missing intermediate bus-name/index consistency and incomplete metadata checks
in the reference consumer. Three regressions reproduced those gaps; fixes were
reviewed again and approved. The complete gate below includes those regressions.
Guidance and current limitations are in `inventory-contract.md`.

This remains a pure proposal/snapshot API; the durable owner below applies its
proposals. Generation-context enforcement and runtime
state retirement ordering require the P3c2 gate before mutable daemon integration.

## P3c1b — durable configuration owner implemented

`config_store::ConfigStore` owns one runtime blocking worker and an adjacent
private lifetime lock. Save admission is nonwaiting and serialized. Both candidate
documents are preflighted before an old-mapping revision reservation is saved,
then the new mapping is atomically replaced and synced. Every public revision,
including descriptions, is reserved; v1 is never created or modified by this owner.

Timeout/cancellation preserves the active worker's slot and lock. Uncertain
replacement stays frozen until explicit recovery reloads a recognized candidate
and confirms file/directory durability. Cancellation and successful finalization
share one state transition. Startup confirms durability too. Close releases file
ownership across retained handles; a blocked worker returns false under the
caller deadline and remains part of managed-runtime teardown.

Verified on 2026-10-21:

- **182 full native tests passed**, including 15 persistence tests and three
  inventory review regressions. Local fault injection covers before/after both
  replacements, cancellation, queued/startup cancellation, failed sync/recovery,
  revision burning/restart, unknown external edits and lock/path safety.
- Eight reproducible negative controls all detected their declared removed
  guards; see `v2-persistence-controls.toml` and its recorded ledger. The restored
  full suite and wait registry passed. Strict scoped Clippy and the armv7
  library/v2/emulator compile check passed; changed Rust formatting/diff checked.
- Fresh source review found a finalization/cancellation race and cleanup armed
  before temporary-file ownership. Deterministic regressions were observed
  failing first; fixes passed re-review. Review was source-only, same model family.

This is a library checkpoint. Process-level stalled-filesystem SIGTERM tests
belong to canonical daemon integration; physical power-loss behavior and Pi
runtime are unverified. Operator-owned directories and ordinary local Unix
filesystems are assumed. No new daemon MQTT/hardware routing is admitted yet.

## P3c2 — shared request-generation fencing implemented

Shared dependency commit: `273062f05c1f0df05cb35f19bad458f2691d8dea` on
`feat/dali-managed-services` in `Projects/tracing-init-dali-v2`. The reviewed
contract and verification record are mirrored in
`superpowers/specs/2026-10-21-bridge-target-generations.md`.

Fixed Bridge instances (including managed startup) can opt into complete target
generation tables. MQTT metadata, queued work, configuration, commands, reads,
reports/removals and sealed publications retain their original context. Atomic
admission/table retirement prevents old intent replacing fresh queued work;
late observations cannot enter a replacement's model or conflict history.
Default unfenced behavior remains compatible. Dynamic factory fencing is
explicitly unsupported and rejected before connection.

The full kit gate passed 194 tests and the existing black-box conformance suite
passed 174 tests. Strict all-target Clippy, three rustdoc tests and the armv7
library compile check passed. Sixteen guard-removal controls each failed at its
intended behavior assertion, including admission/post interleaving, stale direct
and proving reads, original output metadata, late removals and legacy retry/cache/
partial-state compatibility. The scheduling test is controlled coverage with a
200 ms window, not an exhaustive interleaving proof. Two sequential source
reviews approved after four reproduced compatibility findings were fixed;
cross-family review is unavailable. No live broker, HAT or Pi runtime was tested.

The shared runtime spec, README, tutorial and canonical bridge skill were
synchronized. The dependency commit is still local/unpublished. Actual DALI
service routing, durable-save/retirement/publication ordering, and canonical
daemon integration remain next; this bridge has not adopted the new dependency
yet. `mqtt_dali_v2` remains the configuration-check development entry point.

## P3c3a — managed inventory service implemented

`service::DaliService` now owns the durable inventory and routing phase in a
managed BridgeKit service. Owned snapshots and bounded notifications feed
frontend handles; one nonwaiting operation permit spans each admitted save or
recovery. Public edits are metadata-only. Hardware replacement/readdress/group
membership require later verified commissioning jobs.

Reporter attachment is independent of broker/HAT availability. A durable change
retires the complete old runtime table, queues Inventory and only then exposes
Ready. Uncertain saves and failed runtime CAS remain frozen. Explicit recovery
compares the full public model, including recognized mappings recovered with
`changed: false`; runtime-only retry retains the original exact CAS precondition.
The proxy checks both persisted and actual runtime controller identity and
preserves original caller deadlines. Version and UTC build time are in snapshots.

The bridge pins all tracing-init repository dependencies to local development
commit `259d963fd448ea2512e36a00e2f22850af0bc1f7`. This includes a reviewed
single-use command-completion capability: a metadata MQTT command can return one
original-token success across its own inventory commit; all unrelated/queued old
work still retires. The dependency is unpublished; a portable fresh clone/release
still requires publication. Tokio 1.50 in the bridge lock resolves the older
lock's missing OwnedNotified API.

The independent consumer now buffers bounded State-before-Inventory observations,
separately from identity and intent. Exact-token retractions, accepted/ignored
inventory outcomes and total key/payload cache budgets are exercised, along with
an actual MQTT inventory rename preserving consumer records and owned intent.

Verified on 2026-10-21:

- **201 complete native tests pass**, including actual output/group/bus/description
  MQTT success replies, canceled-save recovery via MQTT and direct handles,
  original generation/correlation, stale requests, directory-sync/CAS failure,
  absolute caller deadlines, instance checks, bounded update lag and actual
  blocked-filesystem incomplete stop. Existing migration/UART/emulator/legacy
  regressions and the wait registry remain green.
- Nine service/consumer controls each failed at the intended assertion; the runner
  restored source and recorded `v2-service-controls.ledger.json`. Strict scoped
  Clippy, armv7 library/v2/emulator compilation and changed-file formatting/diff
  checks passed. The shared completion boundary separately passed 200 kit tests,
  174 existing unfenced conformance tests and four controls.
- Sequential source reviewers approved after three reproduced integration findings
  were fixed: own-command reply retirement, ready-after-deadline acceptance and
  actual runtime instance mismatch. Review is source-only and same model family.
  The identity rejection test has a controlled 500 ms absence window; it is not
  an exhaustive scheduling proof.

See `service-development.md`, `inventory-contract.md` and the P3c3 contract.
This is still a service-library checkpoint. UART ownership/health and canonical
daemon startup come next. Physical capabilities, web UI, Store persisted migration,
live Logmon delivery, hardware validation and release/deployment remain pending.

## P3c3b — managed UART health implemented

Historical checkpoint: the general Boot-only recovery prerequisite below is
superseded by the owner's measured quiet-period decision in P4a.

The optional `with_uart` builder starts one Serial/Transactions owner inside the
managed service runtime. Version and enabled-bus status queries use one original
setup deadline; metadata remains available during initialization, MQTT/HAT loss
and poisoned-response waits. Hardware snapshots preserve raw version/status
facts independently of saved routing phase, including Connecting and actual
Closed. Bus power faults do not masquerade as a disconnected HAT.

Late/reset states keep the same owner. Reopen retains a reset latch and a
no-write receive fence excludes already-readable complete/partial Boot records.
Only qualifying Boot plus the transaction actor's Uninitialized state admits
initialization. This does not attribute upstream-delayed data to a physical reset.
Before fixture Send is enabled, P4 must also preserve unresolved transport
ambiguity across process crashes/startup; health queries do not claim that wider
guarantee. Firmware fields never select repeat policy implicitly.

Open/configuration uses a runtime-owned blocking worker. Open timeout is terminal
for the UART owner; actual runtime teardown still includes a blocked opener.
Owned helper results survive canceled selects, and panic results are inspected
before recovery. Hardware/configuration cleanup shares one absolute local budget.

The focused PTY/emulator cases cover late Version retirement, poisoned-owner
Boot recovery, actual transport replacement, complete/partial buffered Boot,
quiet retry after failed boundary, completed-helper panic during Boot/loss,
completed-boundary/fresh-Boot ordering, proactive observation ordering, metadata
during unanswered setup and actual descriptor release with retained handles.
The blocked-opener case confirms incomplete managed stop and no retry eligibility.

Source reviews reproduced and fixed three recovery/ownership issues: a missing
retry timer, a completed panic discarded by a simultaneous hardware event, and
a successful boundary discarded before a qualifying Boot was evaluated. Connecting
and Closed publication were corrected too. These are source-only same-family
reviews. Final verification on 2026-10-21: **215 full native tests passed**,
scoped strict Clippy and armv7 library/v2/emulator compilation passed, and changed
Rust formatting/diff checks passed. All eight controls in
`v2-uart-service-controls.toml` failed at their intended assertions; restored
source passed the full gate. No physical I4, live broker/Logmon or Pi runtime is
implied. The UART slice remains a library integration; canonical daemon startup
is next.

## P3c3c — canonical daemon and original request traces

The canonical `mqtt_dali` and `mqtt_dali_v2` alias now start the managed v2
service through BridgeKit. Explicit `--check-config` preserves the isolated
offline operation. The former main is byte-preserved apart from its test-helper
path in `mqtt_dali_v1`, a regression harness; deployed legacy fixes remain on
the separate v1 maintenance branch/tag. Package/retained Version reports
2.0.0-dev.0 and actual UTC build time. No install scripts or live services changed.

The common CLI parser preflights options and renders usage through bounded
output, then valid arguments pass unchanged to BridgeKit. The standalone offline
flag is recognized only in flag position; even a controller named --check-config
works with split or inline option values. Saved identity is checked before UART.
First startup migrates v1, restart uses v2, and deletion of v2 reimports with a
new inventory epoch while preserving v1 bytes.

Real canonical process tests cover MQTT metadata replies, clean/incomplete stop,
held logging/config/save startup, actual broker-outage-independent UART health,
full unread output pipes, idle CPU and private local OTLP export. The shared
trace defect exposed by that test was corrected in reviewed local tracing-init
commit `96a03bcb9f932d8d49d479a38cc95f9ad9e08ff6`, consistently pinned across all
shared dependencies. Validated owned causes cross queue boundaries independently
of generation authority; coalesced callers retain their own traces. The DALI
actor exports an operation span, and committed Inventory uses the new generation
with the original cause. The exact success reply retains original generation,
correlation and trace. Actual decoded OTLP JSON proves worker and actor spans
under the incoming trace at INFO, rather than an ingress-only match.

Shared restored verification passed 211 kit tests, 174 existing conformance
tests, strict all-target Clippy in both crates, rustdoc and armv7 compilation.
Eight shared controls failed at their intended assertions; README/tutorial,
runtime design and canonical bridge skill were synchronized. Two sequential
source reviews approved after reproduced attribution gaps were fixed. An
existing proving-read test race was corrected with an actual connected barrier.

Daemon source review approved after fixing an Inventory-publication wait, a CLI
mode/value ambiguity and a displaced Markdown separator. The new daemon controls,
changed recovery control and relocated legacy/migration controls are recorded
beside their ledgers. Final restored daemon verification passed **225 native
tests**, scoped strict Clippy, armv7 library/canonical/alias/emulator/legacy-harness
compilation, changed Rust formatting, wait registry and diff checks. The explicit
test-feature logging-start case also passed. All **60** controls in this batch
failed at their intended assertions: five daemon, one changed recovery, five
relocated migration and 49 relocated legacy cases. Restored source passed after
all mutations. No physical behavior or Pi linking is implied by compilation.

See `running-v2.md` and `service-development.md`. The dependency commit is local;
portable upstream clones still require publication. HTTP/capabilities, Store
migration, physical I4/Pi runtime and live Logmon validation remain pending.
Fixture Send is still disabled pending the P4 capability engine. The earlier
crash-evidence prerequisite is superseded by the owner's P4a recovery policy.
Available source reviewers are same family; no cross-family independence is claimed.

## P4a — measured quiet recovery implemented and verified locally

The owner chose measured UART silence instead of the proposed persistent
clean/dirty journal. No journal, state directory or first-start reboot requirement
was implemented. One continuous Tokio AsyncFd reader still processes raw
19200/8N1 input and publishes complete records during recovery. The new
`--uart-quiet-ms` option accepts 1..5000, default 250, before opening resources.
Every nonempty read restarts quiet acquisition, including partial/malformed
records; its total budget is the interval plus the existing setup allowance.

Exclusive actor recovery uses a guarded reader reset with original deadline,
cancellation identity and expected raw activity, then checks the sealed boundary
before entering Uninitialized. It retires complete-write timeouts/receive loss;
V/D initialization is still explicit. Known partial-write poison remains
Boot-only and is captured after actual teardown before a port reopen. Real joins
and lifecycle-owned panic cleanup release descriptors even with retained handles
or canceled close callers. Journal-only clean receipts/history were removed.

Behavioral RED→GREEN tests cover the timer, raw input, complete-write/corruption
recovery, automatic startup/reconnect, invalid CLI values and teardown bugs.
Controlled reader/actor tests cover canceled/expired reset, new activity after
proof, input visibility and partial poison. A canonical process test feeds input
before launch through inventory readiness, measures the configured interval,
kills an unanswered process and restarts with an old version buffered, without
Boot. The partial-write EOF test delivers owner closure before actor cancellation
and proves post-join poison survives into Boot-only reopening.

Two sequential design reviews approved the replacement. First full source review
approved production behavior and requested removing an independent-MQTT timing
race from the subprocess test; the feeder now starts before process launch.
Final source review additionally required caller cancellation to reach the reader
without actor polling. A shared synchronous authorization guard and final actor
check now enforce it; deterministic regressions were observed RED before each
fix. A final stress probe exposed starvation of the quiet timer by ready event
traffic. The actor now checks its deadline before each select, independently of
caller polling; that regression was also observed RED before the fix. Both
reviewers approved the corrected source. Reviews use the same model family.

Verified on 2026-10-22:

- Full native nextest: **246 passed**, including legacy and canonical process
  regressions. Strict scoped Clippy and Raspberry Pi library/all four binary
  compile checks passed. The feature-gated blocked-logging SIGTERM test passed;
  doctests passed (none defined). The wait registry passes.
- **16 new recovery negative controls** all failed for their intended behavioral
  assertions. The 25 existing UART, transaction, UART-service and daemon controls
  also behaved as declared. The affected transaction controls were rerun after
  the final deadline guard. Tables and ledgers preserve the actual evidence.
- Source formatting and whitespace passed. The full restored native suite passed
  all **246 tests** after the guard-removal mutations.

Physical I4 timing, Pi runtime, live Logmon and upstream publication remain P9.

Review learning: a cancellation flag must reach the resource owner directly,
actual task joins must precede resource-release claims, and a biased event loop
must check its total deadline independently of lower-priority timer polling.

## P4b1 — typed base-output codec implemented and verified locally

Two sequential design reviews approved a pure Part 102 codec. It validates
short/group addresses, normalizes ordinary levels within 0..254, creates basic
write/query frames and preserves query-specific MASK/type/status meanings plus
no-response/collision outcomes. Twelve literal-vector tests pass; the initial
new-API build could not compile until its types were implemented. This was not
a behavioral regression RED. Thirteen guard-removal controls subsequently failed
for their intended behavioral assertions, establishing sensitivity to removed
bounds, address/control bits, max/MASK distinctions, positive-level handling,
intent validation, no-response meaning and status polarity. The ATX integration
vector uses the production encoder.

Verified on 2026-10-22: all **258 native tests passed**, including the full restored
suite after controls; scoped strict Clippy and Pi library/all four binaries compile
checks passed. Formatting, whitespace, wait registry and doctests (none defined)
passed. Two sequential read-only source reviews approved the final codec; the
only requested cleanup was a stale design-status sentence. Reviews remain
same-family, and physical fixture behavior is unverified.

No MQTT fixture writes, configuration changes or emulator codec sharing are
introduced. Physical execution, original-generation admission, readback and
fair collection of jobs/events remain the next integration boundary. Reference:
[dali output contract](dali-output-contract.md).

## P4b2 — bounded dispatch and helper collection

Deterministic fail-first probes reproduced queued admission being stranded by
continuous input, completed HAT helpers being starved by renewed observations,
and normal EOF becoming fatal when its query result preceded public delivery.
Transaction dispatch now prefers queued requests after stop; ordinary command
drain/fence/deadline checks remain intact. The HAT owner consumes a bounded prefix
of 129 public receive attempts, then actually joins a completed helper and retires
the stored handle without a suspension between those steps. Queued bus facts keep
priority over earlier query snapshots. Processing work returns promptly even
when it leaves the hardware snapshot unchanged.

A Closed query or status starts owned teardown; its actual join determines
whether a paced retry is allowed. Late EOF cannot abort an owned Close helper.
Actual helper/actor failures remain fatal and post-join partial-write poison is
preserved. Five new tests cover cancellation/expiry, renewed input plus completed
panic/newer bus data, actual PTY EOF ordering, late EOF, status ordering and an
actual aborted actor with delayed public evidence. Six new guard-removal controls
all failed for their intended assertions. The test fixtures use private streams
and PTYs and remain bounded.

The full native suite passed **263 tests**; strict scoped Clippy,
Pi library/all four binaries compilation, wait registry, formatting, whitespace
and doctests (none defined) passed. All 30 existing transaction, UART-service and
quiet-recovery controls failed for their intended assertions. After restoration,
the full native suite again passed **263 tests**. Both sequential source reviews
approved without material findings. Reviews use the same model family.

This closes scheduling prerequisites; physical property execution and readback
remain pending. No live HAT, broker, Store or deployment changed.

## P4b3 — owned output operations and actual fixture readback

The shared service now reads an individual saved output and applies validated
power/brightness intents through its sole UART owner. Metadata and physical
operations share admission. A frontend timeout does not release that admission:
the owner keeps it until actual actor retirement or transport teardown. One
original deadline reaches transaction admission, receive fences and every new
write poll; cleanup has a fixed ceiling. No new frontend owns a UART clone.

Post-write actual-level/status queries provide observed results. Missing fixtures,
MASK, collisions and partial samples remain typed evidence, not an invented off
or echoed request. UART and per-bus counters invalidate stale work even when a
watch value returns to Ready before the owner polls. Mapping changes retain only
exact surviving tokens. Cache and reply share the same final validated sample.

Two source-review findings were reproduced behaviorally before correction:
an expired cleanup could accept a ready permit, and a bus transition could leave
a stale cache paired with successful current readback. Both regressions now pass.
Both sequential source reviews approved; they use the same model family.

The initial full native gate passed **282 tests**, plus strict scoped Clippy,
Pi library/all four binary compilation, wait registry and whitespace checks.
All **19 new guard-removal controls** detected their intended behavioral faults.
All **42 existing affected controls** also behaved as declared: transaction,
UART monitor, service lifecycle, quiet recovery and dispatch progress. The
status-ordering control now targets the current early-close detection site;
its original failure assertion is preserved. After restoration, all **282 native
tests** passed again; strict Clippy, Pi compilation, formatting, whitespace and
doctests (none defined) passed. Hardware validation remains pending.

This is a library execution boundary. Physical MQTT properties, group execution,
color and inputs remain subsequent P4 work. See [the service API](output-service.md).

## P4b4 prerequisite — explicit request context for SDK reads

Shared commit `153d38051f5197b060fa4281efe6136cffee7419` adds the optional
`read_state_with_request_context` hook. Queued refresh and individual conflict
reads now pass their owned original context; compatible defaults preserve
existing contextual/plain drivers and independently captured observation tokens.
Four focused tests include actual ephemeral-broker State/reply traces and
explicit absence across a driver-owned task without OTel. Both dispatch
regressions failed behaviorally before the worker fix.

Both sequential design/source reviews approved; reviews remain same-family.
The restored shared gate passed **215 kit tests** and **174 full conformance
tests**, all-target strict Clippy, Pi library compilation, formatting and wait
registry. Three new and ten affected existing controls detected their intended
faults. All four DALI dependencies now pin this exact commit; DALI's full
**282 native tests**, strict scoped Clippy and Pi library/all four binaries
compile checks pass with the new pin. Physical MQTT property execution is next.

The revision remains local and unpublished. The bridge-development skill and
shared README/tutorial describe the new method; no Store code changed.

## P4b4 — MQTT base output properties

Individual saved outputs now expose PowerState/Brightness through BridgeKit's
Desired/State interface. Actual level/status readback supplies the result, including
fixture clamping. State has explicit Available/Quality; retained last-known scalar
values are unusable while Available=false. MASK and missing fixtures never become
invented off values. Missing fixtures do not block healthy output control.

Physical publication stays on the SDK worker. Driver completions revalidate the
latest service snapshot after their final await; a LinkContext-owned helper reads
latest snapshots and lets the SDK coalesce reports. Source generation, UART/bus
evidence, original cause and explicit unsolicited trace absence remain separate.
Even unchanged post-write readback is reported before acknowledgement, so the
runtime's inferred intent cannot replace observed conflict/reconnect evidence.

Aggregate polling uses a persistent cursor, 15 s total ceiling and 2 s per target.
Cursor advancement precedes the cancellable await and survives disconnect. Final
validation covers early samples after the last fixture await. Disabled buses skip
I/O; polling without a ready UART rejects before taking physical admission, keeping
metadata configuration accessible.

Fifteen new regressions exercise the real managed runtime, independent emulator or
literal PTY HAT, and isolated FakeBroker. They include delayed read/outage ordering,
A-to-B-to-A publication, incomplete aggregate reads, cancellation and reconnect,
actual clamped readback, full-map rejection, missing devices, generation admission
and original request trace with no OTel dependency. The helper-cache and no-UART
admission bugs were reproduced before their fixes. Removing the direct unchanged
readback report also fails the real reconnect/conflict oracle as intended.

The initial complete gate passed **297 native tests**, strict scoped Clippy,
Pi library/all four binary compilation and whitespace checks. All **15 new
guard-removal controls** and **8 affected existing metadata/trace controls**
failed for their intended behavioral assertions. After restoration, all **297
native tests**, strict scoped Clippy, Pi library/all four binary compilation,
formatting, whitespace and doctests (none defined) pass. Both sequential source
reviews approve the documented bounded-rejection contract with no additional
material source finding. Reviews use the same model family. Implementation is
committed as `5943268`, with the explicit outage limitation in `1dee55e`.

Source review confirmed an outage policy boundary: the SDK abandons
Rejected state requests, so HAT recovery alone does not retry Desired consumed
during an outage or before UART startup readiness. This slice explicitly uses
target rejection to preserve metadata and healthy fixtures. The owner explicitly
accepted dropping commands during rare bus outages and stopped the proposed
deferral work on 2026-10-22. No production deferral code was implemented; the
exploratory test scaffolding was removed. The guide states the accepted behavior.
Proceed with the requested switch/color capabilities. Assess likelihood, impact
and simpler alternatives before expanding infrastructure for another edge case.
See [the MQTT output guide](mqtt-outputs.md) and the
[slice spec](superpowers/specs/2026-10-22-dali-v2-p4b-mqtt-outputs.md).

## P4c1 — common input protocol codec

The pure `dali::input` module decodes all five standard input-source schemes,
preserves device/instance type ambiguity and the full 10-bit information, and
distinguishes commands from events. Native Part 301 actions, Part 303 flags and
raw Part 302/304 data are typed without invented gestures or universal lux units.
Read-only device/instance query builders use validated independent input addresses.
This adds no UART reader, scheduler, saved schema or MQTT interface.

Eight literal-vector tests pass. Before implementation, seven tests failed for
missing decode/query behavior while the API skeleton's bounds checks passed. The initial
full gate passed **305 native tests**, strict scoped Clippy, Pi library/all four
binary compilation and whitespace checks. All **7 focused protocol controls**
failed at their intended behavioral assertions. After restoration, all **305
native tests**, strict scoped Clippy, Pi library/all four binary compilation,
formatting, whitespace, wait registry and doctests (none defined) pass. Both
sequential source reviews approve with no material finding. Reviews were
source-only and used the same model family; they did not independently rerun tests.

Both source-grounded design reviews approved this limited boundary; source
reviews remain same-family. Named discovery, persisted input identities, current
state, MQTT actions/configuration/feedback and semantic emulator controls remain
required P4 integration work. See [the codec guide](dali-input-contract.md).

## P4c2 — named input inventory and durable rename

Saved configuration now has separate input modules and nested typed instances.
The complete inventory gives both levels targets and mapping revisions. Empty
input arrays are omitted, preserving existing output-only documents and v1
import behavior. Input/output short addresses are independent; unknown native
instance types remain metadata. Input metadata uses the existing durable save,
complete-table runtime activation and MQTT rename command.

Module/bus rename preserves descendant lifetimes and rotates affected mappings.
Module replacement/removal retires all descendants even with unchanged text.
Producer and independent consumer validate hierarchy after every batch and
refuse a continuing child attached to a replacement parent lifetime. Instance
type changes retire mapping authority and prevent unsafe binding-intent carry.

The first JSON/projection test failed on unsupported inputs before implementation.
Nine new tests cover validation, literal nested projections, rename/replacement,
invalid history, independent consumer behavior and actual MQTT rename/persistence
with retired-token refusal. The initial gate passes **314 native tests**, strict
scoped Clippy, Pi library/all four binary compilation and whitespace checks.
All **6 new focused controls** and **6 affected existing identity controls**
failed at their intended behavioral assertions. The restored gate passes all
**314 native tests**, strict scoped Clippy, Pi library/all four binary
compilation, changed-file formatting, whitespace and doctests (none defined).
The existing wait registry test passes; this boundary adds no waits. Final
sequential source reviews approve frozen `c87bde8` with no material finding.
Reviews were source-only and used the same model family. Hardware discovery,
current input State, native action events/configuration/feedback remain required.
See [the input inventory spec](superpowers/specs/2026-10-22-dali-v2-p4c-input-inventory.md).

## P4c3 — live input State and native events

The existing UART reader/HAT owner now resolves enabled named input instances
and publishes observed State plus native non-retained events through BridgeKit.
Exact device/instance sources and unique saved device/type sources route;
ambiguous/unassociated sources do not receive guessed targets. Native button
actions remain native; absolute/light values remain raw 10-bit fields and
occupancy flags are decoded. Unknown/stale values explicitly clear applicable
properties to null, including a previously pressed button. Input Desired remains
unsupported. Refresh uses current observed projections without another hardware
query; aggregate refresh projects inputs after its final output await.

Per-route generation/UART receive boundaries reuse the existing same-reader drain
and partial-line-aware minimum sequence. Inline rename cannot attach a queued or
partially received old event to the new target token. Unchanged routes retain
their boundary. Metadata does not wait for UART, and no P4a quiet-recovery behavior
was reopened or changed. Snapshot input_listeners reports established routes,
separately from physical presence. Frozen routing suppresses action publication.

Native actions use a capacity-64 broadcast plus bounded HAT batch; loss is
explicit and never reconstructed from retained State. Only SDK-owned worker
helpers publish. Original source generations and absent traces survive the queue,
including events while a traced light command awaits its acknowledgement.
Broker outages preserve the existing SDK's bounded queued-event delivery policy;
there is no new SDK extension or broker-offline discard guarantee.

Three behavioral tests failed against the API skeleton before implementation.
Ten new tests cover source ambiguity, native actions and sensor fields, nullable
state, bounded notices, rename-back, actual MQTT during pending light commands,
old partial-line rename cutover, the existing ATX PTY emulator, cached refresh and
a controlled aggregate-read/input-change ordering. The first literal-peer test
exposed a test-owner capture bug: its retained PTY slave was dropped before the
bridge opened it. Keeping the whole owner in the task fixed the test fixture;
no production UART workaround was added.

The corrected initial gate passed **323 native tests**, strict scoped Clippy and
Pi compilation; the final aggregate regression then passed with the focused set.
All **11 new controls** and **6 affected existing helper/progress controls** failed
at their intended behavioral assertions. After restoration, **324 native tests**,
strict scoped Clippy, Pi library/all four binary compilation, changed-file
formatting, whitespace, wait registry and doctests (none defined) pass. The first
full attempt caught only wait-registry classification/tag issues; those were
corrected before the passing gates. Both sequential read-only source reviews
approved frozen `c0951fb` with no material finding. Neither reviewer ran tests or
controls; the parent ran the verification above. Physical-device validation
remains outstanding.
Both source-grounded design reviews completed; their nullable-state and SDK
broker-delivery clarifications are incorporated. Reviews use the same family.

Hardware input discovery/full-value reads, event configuration/feedback,
unresolved group/source mapping and semantic emulator controls remain required.
See [MQTT inputs](mqtt-inputs.md) and the
[live-input spec](superpowers/specs/2026-10-22-dali-v2-p4c-live-inputs.md).

## P4d1 — typed DT8 colour protocol

The pure colour codec now validates temperature/mirek, fixed-point xy and
normalized RGBWAF channels separately from power/master brightness. Complete
short/group/broadcast write plans load the correct DTR bytes, enable DT8 before
each extended command and ACTIVATE once. Unrequested channels remain MASK.
Individual read plans distinguish byte/word selectors, refresh colour reporting
and obtain low bytes only for word values. Raw bitmap FF, byte-value MASK and
full-word MASK remain distinct; no response/collision fabricates colour data.
Feature/status decoding preserves reserved counts and ambiguous active types.

Both sequential source-grounded design reviews approved this boundary with no
material finding. Seven focused tests pass, including all valid raw coordinate,
temperature and channel conversion round trips, literal command/query sequences,
transport failures and independent address spaces. Three command-plan tests
failed behaviorally against the empty API plans before implementation; initial
test-only type/ownership mistakes were corrected before that evidence was taken.
All ten fault-injection controls failed at their intended assertions in 55 seconds.
The restored gate passes **331 native tests**, strict scoped Clippy, Pi
library/all four binary compilation, changed-file formatting, whitespace,
wait registry and doctests (none defined). Both sequential read-only source
reviews approved frozen `64c05f9` without material findings. Reviews were
same-family, source-only, and did not run tests. No emulator/SDK/Store/runtime
path was changed. Physical validation remains outstanding.

Service discovery, physical limits/mode handling, actual colour readback, MQTT
representation properties, independent emulator semantics and colour scenes
remain subsequent implementation work. See [colour protocol](dali-colour-contract.md)
and the [slice specification](superpowers/specs/2026-10-22-dali-v2-p4d-colour-codec.md).

## P4d2 — independent DT8 HAT emulator

The existing standalone emulator now accepts optional strict colour profiles and
models selected Tc/xy/RGBWAF staging, activation, capability/status/value queries,
Tc limits and MASK. Whole triples retain unchanged actual channels; staged MASK
replaces earlier pending numeric values and activation clears temporaries. Base
master brightness stays independent. Existing addressing, group selection, reply
collision aggregation and DTR storage remain the one physical model path.

Primary gear-source inspection corrected the draft's overly broad frame rule:
the modeled DT8 selection is consumed by standard 16-bit commands before address
filtering/repeat-first return, while special DTR/24-bit input traffic preserve it.
No input-event invalidation hook was added. The combined design gate approved the
amended MASK lifecycle/selection contract. This uncertified reference is a
cross-check, not formal standards/firmware proof. The emulator remains independent
of the production codec. `colour-i4.json` supplies a separate four-bus example.

Four literal semantic tests failed first against the old unsupported path; valid
profile validation already passed. Six new model tests plus one real standalone
PTY process test now cover capabilities, unsupported modes, staged/actual state,
clamping, byte widths, group/broadcast selection, native input traffic and master
brightness. A missing generic parameter in the new process-test helper was fixed
before the passing run. All **11 new controls** and **2 affected existing base
controls** failed at their intended assertions. The restored gate passes **338
native tests**, strict scoped Clippy, Pi library/all four binary compilation,
changed-file format/whitespace, wait registry and doctests (none defined).
Fresh source review found that word colour queries left DTR1 stale. A literal
regression reproduced AA instead of the returned MSB; the model now updates both
registers. The added test covers normal/full-MASK words and known/MASK byte
queries preserving both registers. Its new control and two affected word/byte
controls failed as intended. The corrected restored gate passes **339 native
tests** and the same build/lint/format/wait/doc checks. Final read-only source
re-review approved frozen `2b60104`, confirming the finding resolved with no
remaining material issue. Review was source-only/same-family and ran no tests.

No service/MQTT/SDK/Store or recovery behavior changed. The emulator explicitly
does not yet model alternate RGBWAF control modes/channel assignment, physical
gamut/photometry, fades, colour scenes, automatic activation or DT8 reset/settings.
These remain subsequent work rather than inferred from base command success.
See [emulator colour profiles](emulator.md#colour-fixtures) and the
[slice specification](superpowers/specs/2026-10-22-dali-v2-p4d-colour-emulator.md).

## P4d3 — owned colour service

The individual output service now discovers DT8 capability (including bounded
multiple-type enumeration), physical RGBWAF assignments and active native values.
`read_colour`/`apply_colour` use the existing owned HAT job, permit, context,
deadline, stamps and final acceptance. Setters require fresh manual-activation
confirmation and supported representation; RGBWAF also requires normalized mode
and the requested physical colour assignments. Automatic activation and combined
base/colour ordering remain required before general MQTT colour mutation.

Actual readback retains clamping/range/limit evidence and must match the requested
representation to confirm a colour write. Fresh incidental colour samples have
their own quality and cannot fail successful base control. Whole-output epoch
invalidation marks colour stale. There is no new UART recovery, SDK queue or Store
dependency. The independent emulator now answers physical channel assignments.

The first actual-colour test failed behaviorally against the API skeleton before
execution was connected. Eleven new focused tests pass, including Tc/xy/RGBWAF,
untouched WAF/master independence, type enumeration, unsupported preflight, failed
word bytes, wrong active type, native inputs during cancelled colour work and
final epoch invalidation. Rust reply ownership errors were fixed before the
passing run. All **12 new controls** and **6 affected existing output controls**
failed at their intended behavioral assertions (91 s and 43 s). The restored
gate passes **350 native tests**, strict scoped Clippy, Pi library/all four binary
compilation, changed-file format/whitespace, wait registry and doctests (none
defined). The first source gate found a practical timing regression: known-colour
base operations still used the proxy's 2 s budget despite 73 correlated exchanges
for a six-channel RGBWAF+Tc sample. Three paced UART/MQTT regressions reproduced
expiry before a bounded correction: select 6 s for same-generation known-colour
targets before admission, retain 2 s for plain targets, and preserve the original
15 s aggregate ceiling and explicit service caller deadlines. All three paced
regressions and the original aggregate-clipping test pass. Their three new timing
controls and three affected existing MQTT controls fail as intended (28 s/24 s).
The corrected restored gate passes **353 native tests** and the same lint, Pi,
format/whitespace, wait and doc checks. Both sequential read-only source gates
approve corrected frozen `96424cc`, with no remaining material finding. The first
gate narrowly re-reviewed its timing correction; the second reviewed the full
`c050907..96424cc` change. Both were same-family, source-only, with no tests run
by reviewers. Physical fixture/firmware validation remains outstanding. Evidence logs include
`/tmp/dali-colour-service-controls`, `/tmp/dali-colour-service-restored-gate`,
`/tmp/dali-colour-paced-controls` and `/tmp/dali-colour-paced-restored-gate`.

See [colour operations](dali-colour-contract.md#owned-service-operations) and the
[slice specification](superpowers/specs/2026-10-22-dali-v2-p4d-colour-service.md).

## P4d4 — automatic activation and combined colour/base operations

The typed service now accepts both confirmed manual and automatic activation.
`apply_colour_with_output` retains one owner, deadline and final acceptance for
both intents. All colour preflight precedes either mutation; off commands precede
colour, while positive base commands follow the complete colour plan. Pre-write
actual evidence is cleared before the first mutation. Success requires actual
base plus requested-representation colour readback. Partial effects remain
Uncertain, with no retry/rollback or persistent gear-setting change. Command
order does not promise darkness or completed fades.

The independent emulator adds an optional automatic-activation flag and applies
staged colour through its existing activation routine for supported immediate
power commands, including DAPC MASK. Manual scenarios preserve their default
behavior and serialized shape; fades/colour scenes/settings remain unfinished.

Both design gates approved after clarifying OFF versus zero-DAPC fade semantics.
Four new tests failed behaviorally before implementation (feature bit, physical
automatic activation, service acceptance and combined master write). Eighteen
focused tests now pass, covering seven new tests plus prior colour/paced/native
event coverage; an async result type annotation was corrected before that run.
The existing cancelled-write/native-input test now uses the combined request.
All **8 new controls**, **5 affected colour controls** and **2 affected base
readback controls** fail at their intended behavioral assertions (53/34/16 s).
The restored gate passes **360 native tests**, strict scoped Clippy, Pi
library/all four binary compilation, changed-file format/whitespace, wait registry
and doctests (none defined). Both affected control tables have no stale anchors.
Both sequential read-only source gates approve frozen `3d6cc06` against
`80f358b`, with no material findings. Reviews were same-family/source-only and
ran no tests, builds or controls. Physical fading and firmware behavior remain
unverified. Evidence logs are in
`/tmp/dali-combined-colour-controls` and `/tmp/dali-combined-colour-restored-gate`.

The manual-only policy control is intentionally replaced by unknown-feature
rejection and known-enabled acceptance controls. A moved base-readback control
anchor was also repaired; its intended actual-versus-echo assertion is retained.
MQTT scalar/profile choice is pending and does not block this native increment.
See the [slice design](superpowers/specs/2026-10-22-dali-v2-p4d-combined-colour.md).

## P4d5 — durable public colour representation

The approved one-family-per-fixture choice is now saved as optional
`color_representation` (`temperature`, `xy`, `rgbwaf`). The metadata service and
MQTT `set_color_representation` command use existing durable activation, original
output-token admission and single command completion. Selection changes preserve
physical Origin/name/address and consumer object ID, rotate the output binding
generation and retire old contract intent/observations. Repeating a value is a
no-op. Groups and unrelated outputs retain their bindings. No gear settings or
UART recovery behavior change. Scalar MQTT colour and supported-format State
remain the next boundary.

Six initial focused tests executed before production edits: four failed on
rejected valid selection/continuity or unknown command, while two malformed-input
checks passed. The completed nine-test selection set passes, including durable
save/reopen, original-token reply, stale request refusal, combined rename/format
change and illegal metadata in past history. All **8 new controls** detect their
declared behavioral fault; the omitted-projection control initially tripped the
generation consequence first, so assertion order was corrected and that row
rerun successfully. Two affected existing reply/input-type controls also remain
RED. Restored native suite: **369 passed**. Scoped strict Clippy, Pi library/all
four binary compilation, changed-file format/whitespace and doctests (none
defined) pass. Logs: `/tmp/dali-colour-selection-controls`,
`/tmp/dali-colour-selection-affected-controls` and
`/tmp/dali-colour-selection-restored-gate`.

Both sequential design reviews approved with no material blocker; the old
strict-reader compatibility clarification is incorporated. Updated readers
accept old files, while new-field documents need updated strict consumers.
Source reviews follow the frozen implementation. Same-family/source-only review
limits remain explicit. See the [selection specification](superpowers/specs/2026-10-22-dali-v2-p4d-colour-selection.md).

First source review found two concrete validation gaps: externally tagged enum
objects could pass the promised string-only setting, and current input/instance
metadata could be ignored by the independent consumer. Both were reproduced
behaviorally and fixed with strict shared-string parsing and two current-record
guards. Ten selection tests and all **370 native tests** pass after correction;
all **11 selection controls** detect their intended behavioral faults. Scoped
strict Clippy, Pi compile, format/whitespace and doctests pass again. One new
injector required a locked-API correction; its compile failure was not evidence.
Logs: `/tmp/dali-colour-selection-review-controls` and
`/tmp/dali-colour-selection-review-restored-gate`. Corrected frozen-source
acceptance follows; no UART/SDK/physical setting behavior changed.

Corrected frozen `1a2a472` is approved by both sequential source gates against
`dfd10de`, with no remaining material findings. Reviewers inspected source/tests/
controls only and ran no verification; same-model-family limitation applies.

## P4d6 — selected-family scalar MQTT colour

Saved individual DT8 outputs now accept Kelvin temperature, scalar xy and
normalized RGBWAF channels for their one configured public family. Requests can
combine colour with PowerState/Brightness through the existing owned combined
operation; all validation/preflight precedes mutation. Colour-only requests leave
master power/brightness unchanged. Partial xy explicitly masks its other setter,
and RGB subsets preserve other channels. No new SDK, gear settings or UART
recovery policy was introduced.

State reports actual selected-family values, separate availability/quality,
active format and nullable supported-format/channel/limit capabilities. Inactive,
invalid, missing or stale colour values are explicit nulls, including in actual
merged MQTT State; base availability remains independent. Initial configured
reads discover colour immediately. Budgets are 2 s plain, 6 s colour read/base
write and 8 s colour/combined write, inside unchanged 15 s All/20 s SDK limits.

Three initial MQTT/emulator tests failed behaviorally before enabling the path.
All twelve focused cases now pass, including clamping, original traces, partial
channels/coordinates, malformed requests/evidence, native assignment order,
invalidation and retained-family retirement through reconnect. A test sent an
invalid and valid request in one coalescing window; actual broker errors proved
the expected whole-batch refusal, so it now waits for rejection before testing
the next valid scalar. No runtime workaround was needed.

All **13 new controls** and **3 affected old timing controls** detect their
declared faults. The paced six-channel+Tc MQTT write proves 8 s success and 6 s
failure; first configured reads also detect 2 s regression. Restored native
suite: **382 passed**. Scoped strict Clippy, Pi library/all four binary compile,
changed-file format/whitespace, wait registry and doctests (none defined) pass.
Logs: `/tmp/dali-mqtt-colour-controls` and `/tmp/dali-mqtt-colour-restored-gate`.
Both sequential design reviews approved; frozen source reviews follow. Review
coverage is same-family/source-only; physical firmware and live deployment remain
unverified. See the [MQTT guide](mqtt-outputs.md#colour-controls) and
[slice specification](superpowers/specs/2026-10-22-dali-v2-p4d-mqtt-colour.md).

Frozen `5e7dc2e` is approved by both sequential source gates against `96c818a`,
with no material findings. Both were read-only/source-only/same-family and ran
no verification. Physical hardware validation remains outstanding.

## Remaining phases

| Phase | Status | Next concrete boundary |
|---|---|---|
| P1 UART | Implemented and verified locally | Physical firmware transcripts remain a hardware-validation item; conservative unverified policy is the default |
| P2 ATX emulator | Core, persistent TCP connector/listener, configurable web UI and linked browser tabs implemented and verified locally | P4 extensions and physical validation remain explicit |
| P3 BridgeKit/service | Codec, lifecycle, inventory, persistence, fencing, managed service/UART and canonical daemon implemented | Capability paths; local shared dependency publication before portable clones/releases |
| P4 capabilities | Core ready locally — extensions pending: base and group outputs, fixture settings/status, DT7, DALI-2 inputs/events/settings, DT8, Parts 251–253 data and common DT1 emergency operations | Extensions: feedback parameters/source routing, specialist settings and extended color/fade support. Hardware: physical HAT/fixture validation remains pending. |
| P5 commissioning | Discovery/assignment, bus setup, saved removal, groups/patterns/verification, Blink/Stop, scenes/private settings, full reassignment, lost-address adoption and individual clearing with saved-identity retention accepted locally | Remaining capability workflows |
| P6 authenticated HTTP | Session/status, command jobs, scalar observation/apply and native/job SSE accepted | Remaining P4/P5 actions |
| P7 browser UI | Current-backend screens accepted locally: phone/desktop workflows, restored checks and both sequential source gates pass | Remaining P4/P5 workflows as their backends are implemented |
| P8 Store integration | In progress: foundation and request lifetime complete; 108 color/binding checks and linked emulator path pass; final color review running | Finish color review; native input components, opaque hardware scenes and scene-first mixed presets; combined acceptance |
| P9 release/docs | Pending | Release cross-link/build, SDK release pinning, deployment/rollback guides and physical acceptance; Pi compile checks already pass locally |

No v2 release, production installation, live Store migration, push or merge has
been performed. The legacy master checkout and v1 maintenance branch retain their
existing application source.

## 23 October — UI corrections and Blink active-color restoration

The owner requested the eye icon for password visibility, American color spelling,
and capability-aware Color controls after observing a dimmer offered Color. Both
row/menu entries now require confirmed DT8; real Chrome phone/desktop checks pass
for dimmer/relay absence, DT8 presence, reload and row layout. Eye controls and
spelling checks also pass actual phone/desktop sign-in and dialogs.

The owner approved restoring original active color when Blink ends. The previous
base-only DT8 activation policy in P5g is superseded by this extension, preserving
its historical evidence. A literal-peer regression first failed with original
Tc250 becoming/staying Tc350 after Stop. New capture/restoration reuses the output
owner and Combined ordering, exact native values/control, original identity/stamp
and fixed deadlines; unconfirmed/unsupported captures refuse before pulsing.
Design review GPT-6.1 Sol/high approves with two implemented requirements:
preflight captured intents, and query RGBWAF control again after writes. Source-only,
same-family coverage. Native controls, final checks and source review are ongoing.

Acceptance on frozen `e45975d`: **783/783 native tests pass** across11 binaries
(one explicitly ignored browser test;111s execution,251s including build/cache).
The separate complete real Chrome phone/desktop TCP-HAT browser workflow passes
(148s execution;149s wall). Native Tc/xy/normalized RGBWAF cases include original
off/dimmed states, pending activation changing family/control mode, exact channel
selection, missing capture evidence, ignored restoration and paced50ms UART
responses. Four new controls and two reanchored existing controls each produce
named RED failures, then restore their source. All89 frontend tests pass.

Scoped strict production Clippy, Pi library/four-bin compile, wait validation
(zero findings), changed-source formatting, both embedded asset byte checks and
all affected control plans pass. GPT-6 Astra/high approves `8fb7d04..e45975d`
with no substantive blocker. Read-only/same-family source review; no independent
model-family or physical HAT verification. Nonblocking coverage note: direct
initial unsupported RGBWAF-mode/Primary-N Blink refusal cases are inspected in
code but are not separate native test cases. Their modes remain unsupported.
Combined browser runtime/CSP collection covers the bridge page; linked emulator
functional assertions do not imply independent emulator console collection.

Private demo updated and left running. Actual phone/desktop checks confirm no
Color entry for dimmers/relays, DT8 entries retained through reload, compact row
alignment, and Stop restoring native levels/active colors for ordinary, DT8 and
relay fixtures. An initially-off fixture returns off; all demo fixture levels
and active colors are restored after checks. No production broker, HAT or Store
configuration was used or changed. Local UI source commit `158b520` also contains
the approved password eye icon and American Color spelling.

Implementation lesson: the paced test found that direct MQTT identification and
prepared HTTP jobs had separate budget call sites. Both now use Operation::budget;
future timing edits should trace both callers. The design gate's captured-preflight
and post-write-control requirements prevented inferred restoration success while
retaining the existing owner and native plans. No new recovery framework, memory
write or skill change was needed.
