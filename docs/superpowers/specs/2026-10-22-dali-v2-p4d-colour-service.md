# P4d3: owned colour discovery, writes and actual readback

## Problem and boundary

The bridge has a pure DT8 codec and an independently tested HAT model, but no
service operation executes colour plans. Extend the existing owned output path;
do not expose the transaction session to another frontend or create a second
physical queue. This is a T3 extension because shared mapping/physical admission
and observation authority are involved. Standing implementation approval applies.
Budget: one owned-service batch with targeted fail-first tests, two sequential
design/source reviews, controls and restored checks. The cheaper direct-proxy
path loses cancellation ownership and is rejected. Existing output ownership
avoids rebuilding a generic job framework.

This slice supplies typed library/service colour operations and observations.
MQTT colour properties, startup automatic capability discovery, browser controls,
group writes, alternate RGBWAF control modes, colour scenes and Store components
follow separately. Existing base MQTT operations and saved configuration remain
compatible. No UART recovery or shared SDK change is needed.

## Shared owner and APIs

- Keep the existing output request, service permit, HAT helper, route/context,
  UART/bus stamps, absolute deadline, settlement and teardown. One physical job
  owns all DTR/enable/value steps; health polls and metadata cannot interleave.
  Native input monitoring continues through the existing owner.
- Preserve `read_output` / `apply_output`. Add `read_colour` / `apply_colour`,
  returning the same output observation with optional typed colour evidence.
  Internal write intent is a Copy enum of base intent or colour intent. There is
  no combined base+colour write API in this slice, avoiding unanalysed fade/auto-
  activation ordering. Future MQTT integration must explicitly design that case.
- A request explicitly asking for colour always reads it. After colour has been
  observed at a target, later base requests include fresh colour readback as well.
  Capture that flag from the latest same-generation snapshot while taking the
  existing capacity-one permit. Never merge old colour fields into a newer
  observation or stamp them with the newer operation's trace. Plain targets that
  have never requested colour retain their existing two-query read path.
  The refresh flag includes previously unsupported/partial colour evidence; it
  requests fresh sampling and is never a requirement that colour must be current
  for an otherwise successful base write. Base writes retain their existing
  base-current success predicate. A normal NoResponse/collision in an incidental
  colour query degrades colour quality independently. Transport/epoch uncertainty
  still follows existing whole-observation settlement/invalidation rules.
- Every resulting base/colour value belongs to the same operation context/stamp.
  The existing owner publishes/replies only with its final accepted snapshot;
  stale output invalidation also invalidates colour evidence. Metadata rename,
  address reuse and shutdown retain existing exclusion and retirement behavior.

## Protocol discovery and execution

- Read base level/status first for a colour operation, giving truthful base
  evidence even if a colour write is rejected during preflight. Discover DT8
  with QUERY DEVICE TYPE. For a multiple-type reply, query ascending NEXT DEVICE
  TYPE until finding 8, reaching a greater type/end, invalid progression or the
  original deadline. Because lower types are 0..7, finding/passing 8 takes at most
  nine next-type queries. No speculative DT8 write on absent/unknown discovery.
- Add the base NextDeviceType query to `dali::gear`. Add typed DT8 intent
  inspection (representation and optional channel values) and individual
  QUERY ASSIGNED COLOUR plans with validated channel indices 0..5 to the pure
  codec. Preserve unknown/reserved assignment bytes rather than guessing RGB.
  Extend independent emulator FC handling for its explicitly ordered profile
  channels; multiple-type discovery is exercised by an independent literal HAT
  peer rather than requiring another emulator architecture change.
- Read gear features/status, colour feature/status, RGBWAF control and supported channel assignments. Preserve
  raw replies, absent responses, collisions, invalid feature counts and ambiguous
  active types. Presence of DT8 alone never means RGB support. Type absence is
  unsupported; failed/invalid discovery remains unknown/unavailable.
- Before colour writes, validate the requested representation against features.
  Each requested RGBWAF colour must appear in the discovered assignment replies;
  unknown/unsupported/unassigned channels cannot be guessed from channel count.
  Colour mutation in this first internal service slice requires a fresh successful
  gear-features reply with auto-activation bit 0 clear. Enabled/unknown auto
  activation is rejected before staging, with raw capability evidence retained;
  reads still work and base-write success is unaffected. Do not implicitly change
  persistent gear features. Automatic activation support must be completed as
  part of the deferred combined base/colour ordering/configuration work before
  claiming general DT8 mutation support through MQTT. This keeps abandoned staged
  values from our own failed manual-activation operation from activating during
  a later ordinary base command, without new recovery/cleanup infrastructure.
  This first service slice accepts normalized colour control (control-type bits
  2, as in the emulator 0x80 profile); other modes are explicitly rejected before
  mutation, pending their configuration/control extension. Never change a gear's
  operating mode implicitly. Unsupported/invalid preflight produces Rejected
  without colour setters or ACTIVATE, retaining the actual base read evidence.
- Execute the pure complete colour plan under the existing owner. Sends must
  return normal NoResponse completion; collisions/missing correlated terminals
  do not establish success. No command retries are added. After mutation, re-read
  base level/status, colour status and actual active representation values, plus
  supported Tc limits. Do not return intended values as observations. Tc clamping,
  range bits and MASK remain explicit. Fade-running readback is a current sample,
  not proof that the final desired colour has been reached.
- Value queries use explicit widths and stop assembling on a failed byte. Raw
  bitmap FF, byte MASK and full-word MASK remain different. A read may return
  partial evidence. A colour write requires a supported active kind matching its
  requested representation, usable required post-write values and current base
  readback; another complete active representation cannot confirm that operation.
  On mismatch/incomplete required evidence, retain actual observations and return
  Uncertain. Device clamping is allowed; this does not assert exact desired-value
  equality or completion of a running fade. Incidental colour quality does not
  change base-write success. Preflight rejection precedes mutation. Caller cancellation,
  expiry and epoch loss retain existing owned settlement rather than admitting a
  new operation early. No extra wait/permit/queue is introduced.

## Observation contract

Add optional colour evidence to `output::Observation`, retaining raw device-type
evidence, known type prefix/DT8 presence, feature/status/control replies, assigned
colours, actual xy/Tc/RGBWAF native values and four temperature limits. The colour
evidence carries separate current/partial/unsupported/unavailable/stale quality
and a reason; base availability continues to depend on actual base samples.

Only the reported active supported representation is read as current colour.
Inactive fields remain absent/MASK evidence, with no RGB/xy/Tc conversion. Raw
values and normalization helpers never invent lux, flux, gamut or a new power
state. Malformed/ambiguous status and missing bytes cannot become a confirmed
current colour. Limits are descriptive; real device clamping is authoritative.
Primary-N may be reported but remains unsupported for control in this slice.

## Files, proof and documentation

Touch `service/output.rs`, a focused `service/output/colour.rs` helper and tests,
minimal HAT request plumbing, pure gear/colour codec inspection/query additions
and the independent emulator FC branch/tests. Reuse existing final snapshot,
retention and progress paths. Do not add a parallel service or frontend cache.

Fail-first tests use private ATX emulator PTYs/FakeBroker plus a literal UART peer:
Tc/xy/RGBWAF actual readback and clamp/master independence; retained colour after
subsequent base writes; read_colour unsupported then successful base apply;
known DT8 base apply with unavailable colour-only query results; auto-activation
enabled/unknown preflight rejecting before any setter/ACTIVATE; a peer ignoring
ACTIVATE and reporting a valid different active kind must return actual evidence
and Uncertain; unsupported representation/assignment/mode with no colour
writes; multiple-type DT8 discovery; missing/collided/partial values; original
token/trace after rename/cancellation; input events during an unfinished colour
transaction; same permit held through actual settlement. Extend affected owned
output/progress controls rather than weakening them. Codec literal vectors and
emulator assignment replies use independent expected constants.

Run focused tests after the batch, meaningful guard-removal controls, then full
native regression, scoped Clippy, Pi library/all binary compilation, changed-file
format/wait/whitespace and doctests after restoration. Update service/colour
guides, support matrix, contributor catalog and progress; no shared skill or Store
contract changes are necessary before the MQTT-facing slice.

## Self-review

Round 1 inspected existing request admission, HAT physical helper/settlement,
owner final acceptance, proxy/state publication and primitive codecs. Round 2
traced base reads following colour, token/trace preservation, unsupported versus
unobserved capability, multi-type enumeration order, incomplete word reads and
physical RGB colour assignments. Keeping the entire observation fresh avoids a
field merge with mixed causes; reusing one owned path preserves cancellation
exclusion. No new real-hardware or certification claim is introduced.

## First design gate amendments

The first gate requested explicit base-versus-colour success predicates and
requested-kind confirmation; both are stated above with direct test cases.
It also identified the normal interaction of auto activation with temporary
values after an interrupted operation. The bounded initial mutation policy is
manual activation only, read-validated before staging. Beckhoff's primary
[gear-features command](https://infosys.beckhoff.com/content/1033/tcplclib_tc2_dali/1012421771.html)
states that enabling bit 0 makes lamp-power commands trigger colour transitions.
The pinned python-dali gear-features bitmap identifies that bit, while the
independent emulator explicitly reports it clear. No gear-setting change or
persistent UART/temporary-state recovery mechanism is inferred from this slice.
Automatic-activation mutation remains required before the broader capability is
declared complete; this is a scoped internal implementation boundary.

Both sequential design reviewers approved the amended P4d3 contract. The second
confirmed the shared permit/HAT job/deadline/stamps/settlement fit, bounded type
discovery, independent success predicates, requested-kind confirmation and the
explicit manual-activation/normalized-mode initial mutation boundary. Reviews
were source-only and from the same model family; no independent-family coverage
or runtime verification is implied by those approvals.
