# DT7 virtual scene capture

Tier T2. Continue the approved comprehensive bridge implementation. Design budget:
two bounded self-reviews, one combined fresh-context design gate; implement inline,
serial fault controls and two final source-only gates. Same-family review is the
available limitation; no implementation delegation, live hardware or publication.

## Problem and chosen shape

`scene_settings::execute::candidate` currently copies `actual.level()` for every
fixture. Part208 QUERY ACTUAL LEVEL returns contact0/254; STORE ACTUAL LEVEL IN
DTR0 instead copies the virtual level. Capturing contact254 when the virtual
level is100 changes the scene on recall. The independent relay model already
implements this distinction. Scene rows already accept numeric0..254.

Choose a private capture helper under the existing scene operation. Reuse captured
type-query replies to classify DT7 conservatively, read its virtual level with
the standard repeated command, and pass that level explicitly into candidate
construction. Keep public actual output observations binary. Rejected shapes:
using contact0/254 loses information; applying STORE ACTUAL to all fixtures
would newly require known repeat profiles for working ordinary/DT8 captures;
adding a new owner/command/config field duplicates existing facilities.

No unresolved user choice: Capture continues to create a reviewed candidate,
without saving or programming a scene. Keep current private opaque scene rows,
existing HTTP/MQTT envelopes, authority, eight-second deadline, bus ownership,
cancel/stale settlement and identification exclusion. No polling, inventory save,
SDK/Store dependency, retained metadata or new recovery behavior.

## Classification and native read

New private `scene_settings::capture` helper takes the observed output, link,
route, stamp and deadline. It builds no candidate unless base output quality is
Current and the needed actual colour evidence is valid. A pure DT7 classifier
consumes the existing `colour::Observation.device_types` replies:

- single7 positive; any other ordinary single type (including254) negative;
  silence/collision unknown;
- first255: at most nine subsequent replies, strictly ascending values<254.
  Stop when7 is reached (positive) or a valid value>7 proves absence (negative).
  End254 proves absence only after at least two ascending types; malformed,
  exhausted/unterminated or failed prefixes remain unknown. Evidence after an
  already decisive prefix is irrelevant. Do not use `dt8==false` as proof of no7.

Unknown DT7 returns no candidate and a reason, never a contact-level fallback.
Negative DT7 uses the current actual level/active-colour candidate unchanged.
Positive DT7 sends, under the same permit/stamp/deadline:

1. DTR0=FF (poison; FF is not a virtual control level).
2. Addressed STORE ACTUAL LEVEL IN DTR00x21, repeated by the existing ATX policy.
3. Addressed QUERY CONTENT DTR00x98, accepting only correlated numeric0..254.

No DT7 enable is required for these base commands. Reuse `dali::scenes::Step`
for a pure `capture_level(address)` plan, independently asserting literal words.
No new transaction API: unverified policy rejects repeated STORE before it goes
on the wire, after the harmless scratch-register poison. It returns an explicit
operation error, no candidate; ordinary/DT8 captures still need no repeat policy.
Silence/collision/FF at the DTR query returns no candidate with a reason. Transport
errors retain existing operation error and quiet-recovery semantics. Stop the
plan on a bad Send terminal; do not continue and consume stale DTR data.

The candidate builder accepts the separately sampled level explicitly, never
rewrites `actual_level`, updates caches or sends activation/direct-output commands.
Actual contact/virtual/stored reports are sequential samples, not a coherent
instant or guaranteed future contact. Stored scene read still follows capture.

## Grounding and verification

Primary sources already inspected at pinned python-dali
`4b21c18e79d02d2a97f91d135f0eee450e8c2fb9`, `dali/gear/general.py`:
StoreActualLevelInDTR0 opcode0x21/sendtwice=True and QueryContentDTR0 opcode0x98.
IEC62386-208:2009 public preview printedp13 specifies the virtual-level override:
https://cdn.standards.iteh.ai/samples/15977/473f21f6cbf04867a4035e9424f134a9/IEC-62386-208-2009.pdf
Reference commands:
https://github.com/sde1000/python-dali/blob/4b21c18e79d02d2a97f91d135f0eee450e8c2fb9/dali/gear/general.py

Current code grounding: `service/scene_settings/execute.rs` Capture/candidate;
`output/colour.rs` retained raw type prefix; `dali/scenes.rs` Step/Slot;
`emulator/model.rs`0x21/0x98; `hat.rs` writing-only repeat guard;
`scene_settings/cases.rs` existing unverified DT8 capture; owner/scene_changes
fresh authority and physical stamp acceptance. Memory descriptions/filenames/full
text found no DT7/virtual-scene ruling; separator-aware BridgeKit control matched.

Behavioral RED first: real managed relay at actual254/virtual100 must capture100,
preserving config/native contact/slot. Then strict literal PTY checks exact
poison/repeat/query, both terminal profiles, missing/collided/FF query, ignored
STORE returning poison, bad Send terminal, conservative multi-type boundaries,
unverified ordinary/DT8 success versus DT7 refusal. Assert no scratch capture plan
for unknown/negative type. Existing final-stamp/caller/permit tests remain; add
capture-specific final-stamp rejection. Generic MQTT must return virtual candidate
under original context and leave public actual State binary.

Deliberate controls target contact fallback, missing poison, once-sent STORE,
wrong query/address, invalid-query fallback, unknown-type acceptance and stale
candidate acceptance; each must fail its named behavioral assertion, not compile.
Run serially, then restore before builds/reviewers. Keep literal peers strict.
Wall-clock target: focused cases seconds, controls~2min, one restored full native
suite~2min, scoped Clippy/Pi/format/wait/assets/control-plan checks. No web source
change is planned; prior1177-command browser acceptance remains applicable.

## Delivery and self-review checklist

- [x] Two source-grounded design self-reviews and combined design gate.
- [x] Behavioral RED; pure plan, conservative classifier and owned capture read.
- [x] Literal/model/MQTT/stale regressions; controls detected and restored.
- [ ] Full native and scoped build/lint/format/wait/assets/control-plan acceptance.
- [ ] Update scene/DT7 guides, support matrix, CLAUDE and progress with exact
  actual/virtual boundary and physical/timed/fault/retained-metadata limitations.
- [ ] Frozen native/contract and regression/compatibility source reviews.

No PanelScript, Store preset precedence, live Store data, SDK, memory/skills,
release, deployment, push or merge work. Existing scene UI receives the same
candidate shape; general scene-level terminology remains a later presentation
follow-up alongside retained relay classification, not a new capture flow.

Self-review1 left the document: current model0x21 copies `control_level()`,0x98
returns DTR0, existing DT8 capture explicitly uses Unverified, and Hat guards
only writing scene actions. Therefore a global repeat guard would reject valid
existing capture. No transaction preflight API exists; keep unsupported-repeat
refusal after scratch poison rather than adding transport infrastructure.
Self-review2 opened the neighboring final-acceptance/physical owner paths and
type-discovery failure arms. Type7 may be proved before colour support is known;
candidate construction still requires its independent colour evidence. Positive
DT7 cannot promote partial base evidence. No pending actual colour is activated
by STORE ACTUAL. Existing stored-scene reads select their own report afterward.
Capture-specific stamp test is required even though Set already covers that gate.

Combined fresh-context design gate APPROVE without blockers. It checked local
primary command/IEC sources, retained type replies, unverified ordinary/DT8
compatibility and original permit/stamp/final-acceptance paths. Source-only,
same-family; no verification or network operations by the reviewer.

First behavioral regression returned candidate254 instead of virtual100. Nine
focused native cases now pass in4.015s, including literal both-profile reads,
invalid/unknown/partial evidence, native MQTT/contact separation and reviewed
candidate programming/recall, old unverified DT8 capture and final stale proof.
An initial compile needed service-internal visibility for the existing correlated
`output::frame` helper; no new transaction API or duplicate query executor.
The strict peer also checks its worker remains alive before teardown, so an
unexpected command cannot panic in a detached task and masquerade as an expected
operation error. Mid-checkpoint diff review found no new owner, persistence,
wire-schema or actual-state changes; native capture remains the sole new path.

All11deliberate controls are RED for their named assertions,199.9s latest-row
sum. The eleventh specifically disables Send-terminal validation: the strict
peer's unexpected-command failure is observed by its liveness assertion, proving
an expected operation error cannot conceal continued wire traffic. Source was
restored before freeze and final readers. The serial controls took3m20s rather
than the~2min estimate, primarily per-mutation lib recompilation.
