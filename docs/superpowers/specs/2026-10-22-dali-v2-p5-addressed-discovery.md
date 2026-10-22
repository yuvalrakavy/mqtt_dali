# P5a: addressed-device discovery and default inventory adoption

## Problem and shape

The approved bus page needs detected devices, default names and types, preserving
existing names and missing fixtures. The bridge currently queries only saved
targets; neither output nor input discovery can populate a new inventory.
`inventory::Edit` already supports AddOutput/AddInput and durable identity
creation. `physical::Work` now owns whole input/output query sequences, and
`Owner::activate` preserves an active metadata command's original completion.

Implement one addressed endpoint per operation, reusing those owners. A browser
bus scan will iterate the 64 output and/or 64 input addresses and display progress
and rows on the approved single screen. Each operation is independently bounded
and commits its verified additions before replying; Stop cancels further requests.
This avoids a new background-job store or an operation exceeding BridgeKit's 20 s
bound. A monolithic bus query can exceed that bound on modules with many instances.
Raw frontend UART access would lose physical ownership and admission fencing.

T3: inventory creation and a new public command. Standing implementation approval
applies. This increment reads existing addresses and adopts verified new items;
it does not assign addresses, remove missing items, infer replacements, configure
events, alter groups/scenes, change UART recovery or implement the web UI. The next
commissioning increments add unaddressed/reassign/restore workflows. A fresh
unaddressed fixture cannot answer a short-address discovery probe yet.

## Public operation and authority

Add public `service::discovery::{Space, Address, Observation, Result}` (names may
be refined to avoid shadowing Rust Result) and
`ServiceHandle::discover_address(bus, space, address, expected_revision, context,
deadline)`. Space is Outputs/Inputs; validate bus 0..3 is saved/enabled and short
address 0..63. Adoption is an inventory mutation: require the original controller
generation and expected current revision at frontend and owner admission. Do not
accept a fixture token as controller authority. Phase must be Ready.

Expose WithReply `discover_address` at the controller command target (no target
field), with `bus`, `space` string `outputs`/`inputs`, `address`, and
`expected_revision`. Reject unknown/missing fields, object-valued enum spellings,
wrong JSON types, invalid ranges and disabled/unknown buses before UART traffic.
Extend the private proxy operation parser with Metadata/Discovery variants;
existing metadata parsing/wire semantics remain unchanged.

MQTT uses a 10 s absolute operation deadline. Hardware gets at most 8 s clipped
to that original deadline, leaving normal MQTT calls time for durable adoption.
Typed callers retain their explicit deadline; short calls may expire without
adoption. Never renew per query. The same one permit covers queued hardware,
actual join/settlement, final evidence checks and any durable inventory commit.
Pass the existing opaque CommandCompletion for MQTT so the command causing a new
controller generation can still receive its original-token reply. HTTP/library
calls use ordinary activation. No new shared SDK capability is needed.

Return a discovery report whose `probe` field contains captured bus/space/address,
presence and quality (`complete`, `partial`, `missing`, `collision` or
`type_mismatch`), raw status,
known output type list and completeness or input instance type list/completeness,
current inventory revision, resulting qualified target if saved, and added-item
count. Raw diagnostics remain distinct from claimed complete capabilities.
The probe is historical evidence from the owned query, not a claim of current
presence after saving. It never has Available/current State semantics. The
command reply keeps its original context/trace; fixture State is not
manufactured from a controller-scoped probe. A subsequent ordinary fixture read
uses the newly committed mapping token. Inventory publication remains complete.

## Physical sequence and result truth

Outputs: query STATUS (90) at the individual address. A valid byte establishes
presence, including 00/FF; no response means missing, collision means unresolved.
For a present output query DEVICE TYPE (99): 254 means no extended device type,
0..253 a specific type, 255 starts NEXT DEVICE TYPE (A7). Keep the entire sequence
owned with no interleaving. Read an increasing list of 0..253 until 254 terminates
it, bounded by 255 replies and the original hardware deadline. Missing/colliding,
255 or non-increasing replies preserve only an incomplete known prefix;
never advertise absent capabilities from it. Presence may still permit adoption
of a basic output when type information is partial.

Inputs: query module STATUS (30 with FE selector), then NUMBER OF INSTANCES (35).
For a count 0..32 query TYPE (80) for every instance number 0..count-1 using
24-bit frames. Zero represents a module without input instances and is valid
metadata; counts >32 and type values >31 are incomplete/invalid evidence.
Preserve valid unknown types such as 31 without assigning button semantics.
Only a complete consistent instance list permits creation/expansion of a module.
No response, collision or partial data may fabricate instances or a count of zero.

Both address spaces coexist independently at short address 0. Native L events
continue through the same reader while a probe waits. No query creates a native
action. Use the existing captured UART/bus stamp on every transaction and final
settlement; stamp loss, transport errors, expiry or closing observed before
saving refuses adoption.
No stable physical identity is inferred from a reply at an already saved address.

The [primary device-type query](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/5627818251.html)
requires the sequence to remain uninterrupted. Existing `gear::Query` and
`input::DeviceQuery/InstanceQuery` provide the frames. The current colour helper
stops its type prefix after DT8 can be decided; do not copy its nine-query bound
into discovery, which reports a complete general list. Input query opcodes also
match the pinned python-dali Part 103 definitions already used by P4c.

## Durable adoption, names and existing items

Create new present outputs with the first unused Light1, Light2, ... name in that
bus's output namespace. A missing type reply does not make a responding light
disappear. Create new input modules only from a complete instance list, with the
first unused Input1, Input2, ... module name and unique Input1, Input2, ... child
names. No name/address is normalized or copied from a different address space.
No automatic colour-representation selection or type-specific Home equipment.

Existing outputs keep names, Origin, address, colour selection and group meaning.
Existing input modules keep names and continuing child identities. Add verified
new instance numbers only when all overlapping saved/reported types agree. Add
`Edit::AddInputInstance` for those children, creating a fresh child Origin under
the unchanged module. A changed existing type is reported as a mismatch; do not
rewrite/replace it or append new children to a possibly different module. Smaller
reported counts never delete saved instances. Missing items remain saved.

Build all additions for one observed endpoint into one immutable Model edit and
the existing ConfigStore commit/activation flow. Recheck expected revision,
original controller token, original deadline and current physical evidence before
saving. Candidate preflight failure or confirmed noncommit leaves old inventory;
uncertain filesystem/runtime activation uses the existing frozen recovery path.
If a device/bus disappears while disk is being awaited, a committed addition
remains committed and reserved. The report still contains the explicitly
historical probe, and current fixture availability comes only from separate
same-generation State/ordinary reads. Do not roll back or claim the device is
currently available from that older probe. This keeps save-time presence changes
out of recovery policy and needs no extra physical freshness timer/framework.
No-op discovery leaves revision/tokens unchanged. Repeated discovery does not
duplicate items, rename user choices or resurrect a retired lifetime.

Caller cancellation before save skips adoption. Cancellation during a save has
the existing inspect-latest-inventory/uncertain outcome. It never undoes an already
committed addition. Do not let dropping a frontend release the physical permit.
When a device disappears between probes, report missing without removing its
reserved address. Full address reassignment needs separate durable reconciliation
before hardware mutation; this read-only hardware increment does not invent that
policy or a UART journal.

## Implementation and proof

Add `service/discovery.rs` plus focused submodules/tests as needed. Add a concrete
Discovery variant to physical Work/outcome/completion. Keep its hardware executor
data-only; the service owner performs adoption after actual settlement. Factor
the existing durable-save outcome handling only enough to share it with discovery,
preserving metadata cancellation, CAS and original-command behavior. Extend
inventory edits for AddInputInstance and independent consumer tests for its
unchanged parent/fresh child ledger. No shared SDK or protocol codec dependency
is introduced into the emulator. Existing emulator status/type/count queries cover
common fixtures; extend independent multi-type scenarios only if needed for the
full enumeration regression, retaining existing files/defaults.

Test first through real MQTT: discover an unsaved addressed light, publish one
default-named item and return exactly one original-token/trace reply despite the
controller generation changing. Discover/re-discover and rename without duplicate
items or changed existing lifetimes. Independently cover input modules/unknown and
zero instances, new child addition, overlap mismatch and missing saved children;
separate same-address spaces; missing/collision/partial evidence; malformed and
retired requests produce no UART/config effects; metadata capacity/noncommit/
uncertainty; queued cancellation/deadline, native events during a held probe and
permit retention through settlement/save; full/malformed multi-type enumeration
with a literal peer: [6,8,254] is complete while [6,8,255] is malformed. Existing
output/input/generation suites remain green. The pinned QueryNextDeviceType
reference explicitly distinguishes its 254 terminator from QueryDeviceType's
255 multiple-type marker.
Hold a save, power the emulator bus off, then release the save: the committed
revision/addition must remain consistent, the reply must identify captured probe
evidence, and the new fixture must not acquire Available=true or a current value
from discovery. Current State becomes known only through ordinary readback.

Run meaningful mutation controls, restored native suite, strict scoped Clippy,
Pi compilation, changed-file format/whitespace, waits and doctests. Update the
operator/service/emulator/inventory docs, support matrix, contributor catalog and
progress. Gate the frozen implementation twice; reviewers remain same-family and
physical I4 evidence remains unavailable. Do not call assignment or the full P4/P5
surface complete after this increment.

## Self-review and gate verdict

Round 1 checked current resolve restrictions, durable Add edits, complete command
completion, config zero-instance validity, emulator independence and primary
uninterrupted type enumeration. Round 2 checked reply retirement after adoption,
short deadlines, no-op generation behavior, partial metadata, same-address input/
output spaces, child-parent lifetime continuity, cancellation and reused names.
One endpoint per operation gives concrete bounded progress without a new job
framework. All owner-facing workflow choices remain as approved. Design gate:
surely yes, sequential buildability and correctness reviews before production.
