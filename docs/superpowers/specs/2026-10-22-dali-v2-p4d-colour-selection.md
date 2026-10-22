# P4d5: durable public colour representation

## Intent and chosen shape

The owner approved one configured public colour representation per physical
fixture, with all observed supported formats reported separately. This prevents
retained desires from unrelated colour families competing after reconnect and
gives consumers a stable component contract. A format change preserves the
fixture's physical lifetime and public name/address, but changes its binding
generation so requests made for the old component contract cannot execute.

Alternative: expose all writable families simultaneously. Rejected because the
SDK coalesces scalar desires and does not retain per-family ordering; adding a
second coalescer or SDK conflict-group machinery is unnecessary for this owner
choice. Alternative: represent each family as another device. Rejected because
they are one physical fixture with one identity. Alternative: infer the public
contract from every active-mode read. Rejected because external masters can
change that mode independently of the user's configured component.

This is T3 (persisted and consumer-visible identity contract). Existing standing
implementation and architecture approval applies. The two sequential design and
source reviews are read-only; no implementation delegation. This bounded slice
implements the persisted setting and its configuration command; scalar MQTT
colour values and capability State follow using it. No shared SDK, UART recovery,
physical gear-setting, Store, commissioning or Web UI change belongs here.

## Data and identity contract

Add a typed optional `color_representation` field to configured Outputs and
identity Projections: `temperature`, `xy`, or `rgbwaf`. Absence or null means no
public colour family, and serialization omits absent values. Unknown strings,
objects, numbers and arrays are invalid. Existing v2 documents and literal
inventory fixtures keep their shape; v1 imports explicitly initialize absence.
Choosing this metadata is not evidence that physical gear supports the family.
Compatibility is backward-reading: the updated reader accepts old documents.
Older strict readers may reject new documents containing this field; v2 remains
in development and must update its consumers together, without changing v1.

Project the setting into the complete observed Inventory and every applicable
ledger baseline/transition. It is legal only for output projections. A changed
setting participates in binding-revision comparison exactly like location or
input instance type: preserve Origin, rotate the output binding token, append a
from/to continuity edge and keep the same target/address. Group member Origins
and unrelated binding tokens stay unchanged. A repeated identical setting is a
no-op. Changing away and back still records both changes; stale generations do
not become current again. Manual config changes that disagree with a persisted
ledger fail validation rather than silently rewriting its history.

The independent example consumer must validate current output settings against
history, forbid colour metadata on other address spaces, preserve object ID and
retire old observations/colour desires on a contract change. Its existing policy
already clears intent for same-target/same-location semantic changes; explicitly
include colour changes so an atomic rename plus format change also clears intent.
Ordinary same-format renames continue to preserve consumer intent.

## Mutation path

Add `Edit::OutputColorRepresentation` and matching `MetadataEdit`, carrying bus,
short address and optional typed representation. Apply through the existing
owned config transaction and generation-CAS path. Route admission against the
original output binding token, not the controller token. Reuse the original
command-completion token so a successful self-retiring command still replies once
under its initiating generation. There is no hardware operation or mode change.

Expose command `set_color_representation` on an individual output target, with
required `expected_revision` and required `color_representation` (null clears).
Reject missing field, unknown fields/formats, non-output/absent target and stale
revision/generation before mutation. Return the normal committed revision.
The service API is also the future Web UI path; do not write config directly.

## Files, evidence and execution

Grounded owners: `config.rs` Output and legacy importer; `inventory.rs` Projection
and Edit; `inventory/ledger.rs` project/shape/match_current; `inventory/edit.rs`
binding comparison/apply; `service.rs` MetadataEdit; `service/routing.rs` admission;
`service/proxy.rs` command declaration/parser. Direct configured Output literal
constructors in `inventory/tests.rs`, `service/tests.rs` and the legacy importer
need the new default (enumerated with `rg -n 'Output \{' src tests`).
The example consumer deliberately shares wire record types but not producer
replay or config validation (`tests/support/inventory_consumer.rs`).

Before production edits, add fail-first deserialization/roundtrip and consumer
contract tests using literal JSON histories; do not count missing-type compiler
errors as red evidence. Test continuity, generation retirement, unchanged group
membership, same-value no-op, reload, tampered-history rejection and rename plus
format change. Then add the typed metadata path and a private FakeBroker MQTT
test for durable change/reply, old-token rejection, clearing and parser errors.
Use the existing fixture/owner and no real UART or broker.

Run focused tests, meaningful negative controls for projection/generation/
consumer/command admission, then restored native suite, scoped strict Clippy,
Pi library/all binaries compile, changed-file format and whitespace. No leaf
waits are added. Update the colour/config/inventory guides, contributor catalog
and progress with exact checks and limitations. No reusable skill change needed:
these are DALI-specific data and wire semantics.

## Self-review and gate verdict

Checked the complete owning structs, output constructors, all metadata variants,
projection construction/shape/replay, independent consumer continuity and current
snapshot validation, and existing self-retiring command tests. The risk is loss
of identity or stale intent on a normal format change; use the current ledger
mechanism rather than inventing another ID or request queue. Missing physical
support is a later operation preflight failure, not a configuration-write failure.

Design gate: surely yes. Two distinct sequential reviewers cover buildability,
identity/wire safety, accepted valid old files, rejected malformed histories and
an audit of the architect choice. Same model family; no independent-family or
physical hardware coverage is claimed.

Design gate 1 approved with no material blocker. Its compatibility clarification
is incorporated above: old strict readers need an update for new-field documents.
The review was read-only and same-family, with no builds, tests or mutations.

Design gate 2 approved with no material blocker, checking producer/consumer
representation equality and original-generation single command completion.
It was also read-only/same-family, with no builds, tests or mutations.
