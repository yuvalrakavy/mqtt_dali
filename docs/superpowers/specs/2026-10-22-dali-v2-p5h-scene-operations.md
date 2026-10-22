# P5h2b1: owned stored-scene reads and native recall

## Purpose and boundary

Named slots now have durable identity. The next useful increment connects those
slots to the existing UART owner: inspect one fixture's stored brightness/colour
and recall a named native slot on its bus or a group. This establishes the shared
physical operation boundary before private desired contents, editing, capture,
verification, copy/clear (P5h2b2) and the approved Scenes screen (P5h3). The approved
architecture and continuation authorization apply; no new user policy is needed.

Use one concrete `physical::Work::Scenes` under existing service admission,
Transactions, deadlines, settlement and teardown. A separate scene UART client or
task would race DTR/report registers and the input reader. No scene operation
persists an active flag, infers fixture levels from a recall, or changes stored
contents in this increment. No new recovery policy, timer, journal or retries.

## Grounding and cross-subsystem invariants

| Shared datum | Codec/emulator | Service | MQTT/HTTP |
|---|---|---|---|
| Slot | checked 0..15, scene report queries | same checked slot | strict numeric slot on a fixture read; named scene supplies recall slot |
| Brightness | 0 Off, 1..254 level, FF MASK | distinct from missing/collided evidence | tagged response, never unknown treated as MASK |
| Colour | scene query selects report; actual-level query replaces it | discover before stored queries; one owned operation | native Tc/xy/RGBWAF data and unsupported type retained explicitly |
| Recall | ordinary short/group/broadcast command | one send, no invented physical confirmation | momentary command reply says sent, no retained trigger/active flag |
| Authority | physical bus/session stamps | original request plus final mapping/stamp acceptance | original generation/trace/reply context survives |

Existing anchors: `dali/scenes.rs` supplies Slot/ReportValue/report decoders;
`service/groups.rs` demonstrates requests/resolution/physical results and permits;
`service/physical.rs` owns concrete work; `service/hat.rs` admits/settles and stamps
it; `service/owner.rs` validates final results and publishes complete snapshots.
`service/output/colour.rs` discovers DT8/types/features; expose the existing
capability discovery to sibling service operations rather than reimplementing
device-type enumeration. Its actual-value reads happen before selecting the
stored scene report, so they cannot replace that report midway through a word.
`service/proxy.rs` and PreparedCommand share MQTT/HTTP parsing/admission/deadlines.
`identification.rs` owns a claim while releasing the permit between pulses.

## Public operations

| Command | Target | Strict parameters | Class/budget |
|---|---|---|---|
| `read_scene` | Individual output | `slot` | Read; 8 s |
| `recall_scene` | Named scene | optional `group` (0..15), `expected_revision` | WithReply; 3 s |

A fixture read may inspect an unnamed native slot. Resolve it through the current
output mapping, allowing the existing optional-generation read convention; if a
generation is supplied it must match. HTTP continues requiring its envelope
generation. Scene recall requires the original scene generation, enabled saved
bus and exact complete inventory revision. The optional group is a native numeric
scope; it does not create a second slot space or require saved group metadata.
No parameter defaults a missing slot, accepts coercion, permits unknown fields,
or lets an output/input/group target act as a named scene.

Expose corresponding typed ServiceHandle methods through the same request/permit
path. `read_scene` dispatch must happen before requiring CommandCompletion, as
with `read_groups`. Recall does not save inventory, so it keeps its original
completion without retokening. Both normal MQTT dispatch and PreparedCommand
return the same serialized report. No new HTTP endpoint or browser source.

## Stored read and report

The physical read first obtains existing DT8 capability evidence, then queries
the selected scene level. Known non-DT8 fixtures need no extended queries. Unknown
DT8 support stays unresolved; do not present colour as ignored. Confirmed DT8
queries stored colour type then the selected supported family: Tc, both xy words,
or all six RGBWAF bytes and control. Preserve masked type separately from a
selected family whose attributes are all masked. Unsupported/Primary-N type is
reported explicitly; no conversion or successful supported-family claim.

Use ReportValue plans under the single permit. Stop a plan on missing/collision
and never combine one successful high byte with a stale DTR low byte. Preserve
raw transport outcome and MASK distinction in serializable service evidence.
Keep complete-word FFFF as MASK while FFxx is numeric. Report fields identify
output target/generation, bus/address, slot, observed level/colour, quality and
staleness; they are stored configuration evidence, never current fixture State.
Partial/missing/collided results remain inspectable and unresolved. Transport
ordering loss still follows the existing physical recovery path.

Do not publish this report as scalar output State or cache it as actual colour.
Final owner acceptance checks original output mapping and physical stamp after
settlement. Metadata cannot change while this request owns the existing permit.
No additional scene observation cache is needed.

## Recall and existing observations

Resolve the named scene to its bus/slot, then send one ordinary native Recall to
broadcast or the explicit group. Correlated NoResponse is the expected terminal
for this command; data/collision and transport errors do not establish success.
Return `sent` only after current mapping and physical-stamp acceptance. It means
the command was sent; it does not prove which fixtures participated, actual final
levels/colours or completed fades. If cancellation/expiry/loss follows a possible
write, retain the existing uncertain physical-operation semantics.

Recall changes actual values without individual readback. Invalidate cached
output observations on the affected bus, including colour, at owner admission
before any possible send and again before completing the settled request. The
caller deadline may return Uncertain before settlement, per existing conventions;
admission publication ensures old values are already stale during that interval.
Publish the existing `Available=false`/`Quality=stale` contract and null stale
colour values. Base PowerState/Brightness may remain in merged State as the
existing contract documents; consumers must gate them on availability/quality.
Invalidate the bus even
for group recall: physical membership may differ from the saved catalogue.
Invalidate after every admitted recall that might have reached the wire,
including missing observations on cancellation/timeout or an error after send.
The completion's request retains its bus even when no usable sample survives.
Do not fabricate resulting levels or silently issue a bus-wide read or writes
of previous desired values. Subsequent ordinary refresh obtains actual values.

Refuse recall while identification is active on that bus, at submission and
owner admission. The shared permit alone cannot prevent a recall during Blink
dwell and later restoration overwriting it. Reuse claim identity/location and
continuing Origin rather than trusting a stale name; reading stored settings can
coexist with Blink. Do not change identification lifecycle or UART recovery.

## Implementation parts and verification

| Part | Files | Required proof |
|---|---|---|
| Typed operations/evidence | new service/scenes.rs (split read helper if useful), output/colour visibility | literal basic, Tc/xy/RGBWAF/MASK/unsupported and partial reply cases; nonactivating read |
| Concrete owner | service.rs, physical.rs, hat.rs, owner.rs; identification claim helper | owned permit/stamp/deadline/settlement/cancellation; both recall scopes; reject Blink conflict; invalidate actual cache |
| Shared wire path | service/proxy.rs and focused parser/operation helper | strict classes/parameters, original context, stale tokens/revision, correct target namespace; real MQTT/HTTP |
| Independent physical evidence | literal scene peer and real standalone emulator cases | correct native opcodes and report order; selected group/bus behavior; no Store/Remove/ACTIVATE while reading |
| Regression/sensitivity | focused scene operation tests/controls | intended REDs for read absence before code, MASK vs missing, no report reselection/word mixing, scope/authority, stale acceptance, cache invalidation |
| Documentation/catalog | scene/service/web guides, matrix, CLAUDE/progress | clearly distinguish observed stored values, sent recall and actual State; mark editing/UI pending |

Before code, complete self-review and two sequential design lenses. Build
fail-first, run focused literal/PTY/MQTT/HTTP cases and deliberate fault controls,
then freeze and run source reviews. Finish with restored native suite, scoped
strict Clippy, Pi lib/four-bin check, doctests, changed-file formatting, affected
control plans and wait validation (write generated references then validate).
No new loop, task or daemon requires an idle-CPU campaign. No browser source
changes or physical certification are claimed. No Store schema/memory or general
BridgeKit skill changes are necessary for this local operation extension.

## Self-review

Both sequential design reviews APPROVE without blocking findings. They cover
implementer/buildability and correctness/reader completeness against `a03ffee`
code, with `0780df2` acceptance documentation. Reviews are same-family and
source-only; no cross-family or physical validation is implied.

As-built parts: typed service/evidence and literal query plans are in
`service/scenes.rs` and `scenes/read.rs`; concrete Work/Run/Sample/Completion,
owner and claim checks are integrated; shared MQTT/PreparedCommand paths expose
both commands. Eight library cases plus one canonical/standalone process case
cover the planned surfaces, including all returned colour attributes. Thirteen
scene fault controls and the existing headerless group-read control produce
intended behavioral REDs. Contributor/operator docs and capability catalog are
updated. Full restored regressions and frozen-source review remain pending.

Check sibling cases: output read versus scene recall authority; optional read
token versus required mutation token; bus versus group scope; Off versus MASK
versus no response; masked type versus masked attributes; base versus confirmed
DT8 versus unknown; selected unsupported type; high FF byte versus full-word MASK;
report queries versus actual queries; post-send current versus stale stamp;
recall during Blink starting/dwell/restoring; all-bus cache invalidation despite
saved group drift. Private desired content persistence and mixed HA preset
precedence remain in their assigned later phases.
