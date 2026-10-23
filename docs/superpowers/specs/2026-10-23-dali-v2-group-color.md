# DT8 hardware group color

Status: implementation under the approved v2 architecture and the owner's
instruction to continue. Tier T2: optional saved group representation plus
read-only State and strict color commands. No Store/schema/PanelScript changes.
The approved design §9 puts split-address RGB/RGBW composition in Home equipment;
the bridge already supplies the individual/group brightness endpoints it binds.

## 1. Intent and chosen shape

Extend existing native group ownership and individual DT8 evidence. A hardware
group receives a native group-addressed DT8 staging/ACTIVATE sequence; individual
short addresses establish membership, support and actual color. This preserves
group behavior and the existing transaction owner. Rejected: per-fixture write
fan-out changes hardware group semantics; a second group/color worker duplicates
ownership and acceptance. No new polling task, journal or retry mechanism.

Use existing group generation/revision, single permit, physical stamp/deadline,
bus-wide invalidation, original RequestContext and SDK publication path. All
support/values are driver observations, never inferred from saved group names.
Coverage remains explicitly saved_inventory: unconfigured devices outside the
saved roster cannot be certified. No invented aggregate lamp-failure flag.

## 2. Saved representation and public contract

Group gains optional color_representation with the existing strict string enum:
temperature, xy, rgbwaf. Absent/None preserves old v2/v1 import behavior and omits
the field. set_color_representation extends to saved groups with the same original
generation and expected_revision checks. Metadata-only selection changes no
hardware. Representation changes rotate that group's binding token/intent in the
existing identity ledger, retain its Origin, and do not change output identities.
Ledger validation permits the representation on Group projections as on Output.

Group scalar/atomic Desired accepts the selected family's existing Color* fields
and optional PowerState/Brightness through the existing strict output intent
parser. Wrong-family, read-only, unknown, null/non-numeric/out-of-range and empty
maps still fail before wire activity. Base-only requests keep Action::Set and
the existing native power/dimming behavior. Color/Combined requests use a new
color action under the same owner; no new SDK contract or dependency revision.

Commands:

- read_group_output: existing parameters/result and base operation remain. A
  selected group representation may additionally collect color on reads.
- read_group_color: no parameters; explicitly inspects native color support even
  before a representation is selected.
- set_group_color: strict {expected_revision, values:{Color* fields and optional
  PowerState/Brightness}}; requires at least one selected-family color value.
- Existing set_group_output stays strict power/brightness only.

Typed service exposes corresponding read/color-write methods, reusing output
color/base typed intents; grouped Observation retains native color samples.
Existing command Report remains the membership/base evidence envelope. Group
State is the canonical serialized color result, available through MQTT and the
protected live snapshot/SSE. UI color controls never use a job result as State.

Individual-group State adds existing ColorRepresentation, ColorAvailable,
ColorQuality, ActiveColorRepresentation, ColorCapabilities and nine Color*
scalars, plus per-scalar Color*Mixed and ActiveColorRepresentationMixed.
Fresh complete membership is required. Unknown membership/absent color is
partial/unavailable/unobserved; no members is empty; absent selection is
unconfigured. Invalid lifecycle/retired samples are stale. Any known unsupported
member prevents collective availability. Active-format mismatch is explicit.
Base and color availability remain independent, including level MASK.

For each selected common scalar: all numeric/equal => value, Mixed=false;
all numeric/different => null, Mixed=true; not readable/assigned on every member
=> null, Mixed=null. Non-selected fields and non-current observations explicitly
clear scalars/mixed flags with null. Active representation has analogous uniform/
mixed/unknown evidence. ColorAvailable=true permits different actual channel
values; it means complete evidence, not a uniform requested result.

Capabilities report only common evidence for actual known members: DT8 support,
intersection of supported representations, common physical RGBWAF assignments,
and intersection of temperature limits when complete. Missing membership or
required capability evidence yields null, never optimistic support. Do not invent
one shared physical device/control byte for heterogeneous members. The selected
group format is independent of individual outputs' saved selections.

## 3. Native execution and failure behavior

Read membership over the complete frozen saved bus roster. Read level/status and
color by short address only for actual hardware members, not just saved expected
members. Retain both expected membership and observed membership diagnostics.
Color preflight completes for every known member before the first color/level
mutation; missing membership, empty groups, unsupported/unknown DT8, unconfirmed
gear features, unavailable requested channel assignment or unnormalized RGBWAF
refuses the operation. Reuse output::colour::discover/preflight; no codec clone.
Scratch DTR queries during preflight are not light-output/configuration writes.

Use existing Intent.frames(Destination::Group), which explicitly activates.
Combined Off sends group OFF before color; non-Off base is sent after color,
matching individual ordering. Pre-write samples cannot confirm a mutation:
clear/invalidate color samples and base level/status before the first mutation,
then query fresh actual membership/levels/status/color. Fresh RGBWAF control is
part of read_values. No requested-value echo; clamped actual values remain actual.
Unknown/partial post-write evidence is unconfirmed and never retried implicitly.

Reads and mutations retain the original fifteen-second group deadline and SDK
operation budget. A large/slow roster can return incomplete/unconfirmed evidence
within that bound, not a success or an unbounded scan. No extra timer/poller.
Existing ReadAll cursor/fairness and per-bus invalidation remain. Base-only writes
need not perform color probes; they invalidate cached color instead of asserting
that DAPC could not activate staged color. Configured ordinary group reads may
sample color; explicit color inspection supplies discovery on unconfigured groups.
Final owner and proxy acceptance still require the exact accepted Observation,
original generation/revision, deadline and physical lifecycle/stamp. Completed
captures retain decoded partial/colliding replies through existing acceptance.
Transaction, stamp or deadline aborts may discard partial diagnostics: prior
evidence remains stale/unknown, the existing unconfirmed report is returned and
an explicit fresh read is required. No new failed-observation cache/plumbing.
Pure color confirmation requires ColorAvailable independently of base MASK;
Combined requires both ColorAvailable and base Available; base-only is unchanged.
Both new command names use the existing bounded dense group HTTP report budget.

## 4. Operator UI

Reuse Groups' existing selected-group card and common Modal styling. Add a compact
color section below power/dimming: Inspect color support, observed common support,
saved representation selector, and Color controls only for configured/confirmed
support. Known non-DT8 members show an unsupported message and inspection action;
they do not offer editable color values. The selector lists common representations.
Selecting/clearing saves metadata and re-reads with the new generation.

Color modal uses the existing individual labels/units: temperature K, normalized
xy, percentage RGBWAF. Show actual values or Mixed/Unknown from current SSE State;
send changed fields only with original group authority/revision. Partial xy/RGBWAF
writes preserve unrequested attributes. No power write is invented for pure color.
Read current values is explicit, drafts survive same-generation SSE refresh and
retire on target/representation changes. Disable mutation during matching-bus
Blink; reuse existing busy/ready controls and original context. Phone/desktop
modal, selection menus, layout, real native writes and restoration are verified.

## 5. Grounding and verification

| Claim/extension | Reached from current code | Verification seam |
|---|---|---|
| Native group addressing already exists in color codec | Intent.frames/staged_frames -> Destination::Group | literal group vectors, independent emulator |
| Group helper already owns roster, stamp, permit and result | proxy -> group_evidence -> Hat -> group_output::execute/accept | managed PTY/TCP service tests, delayed final acceptance |
| Individual support/preflight/value sampling is reusable | output::execute -> colour::discover/preflight/read_values | mixed/unsupported native profiles, literal peers |
| Representation already rotates output contract/intent | inventory::Edit -> projections/ledger/edit reconciliation | old config/import, group generation/Origin/intent tests |
| Color projection is currently output-wrapper-specific | proxy::colour::project | extract sample-only projector preserving existing guards/tests |
| Group MQTT adaptation currently rejects color | validate/apply/read -> proxy::group | fail-first native TCP -> Desired -> actual State |
| UI uses protected live State and generic Modal | values(snapshot,row), current physical row | real Chrome phone/desktop grouped color workflow |

Fail-first tests: old group color metadata/Desired refusal; basic native group
temperature set/read; xy partials and RGB/RGBWAF assignment/mode masks; both native
terminal policies; differing limits and clamped actual readback; uniform/mixed/
empty/unknown/unconfigured/stale/retired State; status-vs-level independence;
all-member preflight with outsider, actual-vs-saved membership and missing-member
cases; no color/base mutation on refused preflight; post-write loss cannot reuse
old sample; original revision/generation/stamp/deadline/cause; existing group base
and individual DT8 regressions. Config/import/ledger compatibility is explicit.

The old historical Group-color rejection fixtures become legal group-history
cases; preserve wrong-space/past-history rejection using Scene/Input fixtures.
Update the independent inventory consumer and prove group identity continuity,
old intent/observation retirement and output independence without producer-only
acceptance helpers. Check dense HTTP results for both new command names.

Meaningful negative controls target all-member preflight, invalidation before
mutation, unknown/mixed clearing and original acceptance fences. Observe named
REDs, restore source, validate affected existing control plans. Browser acceptance
drives real protected UI controls against the independent TCP emulator/private
broker: group writes affect members only, current/mixed readback and refreshed
State, metadata selection/clear, clipping/scroll, and restoration after tests.
Run targeted then one final full native suite, frontend checks/build/reproduction,
strict scoped Clippy, Pi check, normal wait lint, formatting and diff review.
No hardware/home broker/Store data; no publication/main merge/release.

## 6. Implementation phases and review

1. Config/ledger authority, strict parser and shared color projection; meaningful
   failing tests first, then focused compatibility tests.
2. Owned native group preflight/write/readback, independent aggregation and MQTT
   adaptation. Mid-checkpoint and self-review before UI work.
3. Protected group UI and actual phone/desktop emulator/browser checks.
4. Dedicated docs/skills phase: groups, MQTT outputs, support matrix, running/UI
   guides as appropriate, CLAUDE and v2-progress. Correct split RGB placement and
   stale phase-summary entries in the progress record. No Store skills/memory edits
   are needed; ways-of-working and staged-design-review apply to this bridge.
5. Scoped controls, freeze, fresh depth review and final verification. Same-family
   models available; no independent-family or physical certification claimed.

Self-review: native group frames avoid per-fixture composition; readonly aggregate
State is bounded by actual membership and selected common support. Metadata is
optional, generation-fenced and independent of physical identity. No new worker,
poller or durable state outside the existing config/Observation paths. User intent
and architecture are already approved; no new product decision is required.

Fresh GPT-6 Astra/high design verification: APPROVE. The reviewer confirmed
native group DT8 emulator routing and existing shared ownership. Three seams were
explicitly resolved before coding: component-dependent success, dense HTTP budget
classification, and narrowing the aborted-diagnostic promise to existing stale/
unconfirmed + refresh behavior. The last is a spec correction, not a new outcome
mechanism; the reviewer separately approved that simpler disposition. Same-family
read-only review only; no runtime or physical certification claimed.
