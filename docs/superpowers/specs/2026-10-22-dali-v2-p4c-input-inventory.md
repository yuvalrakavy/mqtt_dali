# P4c2: named input modules and instances

Tier T3: additive persisted inventory and MQTT identity contract. The approved
architecture already selects named physical modules/instances and continuity on
rename. This boundary implements that decision before event routing; it does not
open another UART reader or redesign transport recovery. Target budget: about
30 minutes for implementation/verification, with two short sequential design
and source reviews. Explain any material overrun or blocker to the owner.

## Problem and shape

Confirmed in current code: Target::input_instance already encodes
Bus1.Inputs.WallPanel.Instances.EntranceKey, but config::Bus, inventory::Location,
ledger::catalog and the service rename parser currently support outputs/groups
only. Raw L frames cannot be associated with durable named input instances.
The existing Model/edit/ledger, ConfigStore and complete-table activation are the
analogue to extend. Ad-hoc names outside the ledger would lose rename continuity;
reusing output addresses would conflate independent DALI address spaces.

Add a default-empty inputs array to Bus. Each Input has short_address, name,
optional description and instances. Each InputInstance has number, name,
optional description and instance_type (raw u8, including unknown types).
Addresses 0..63 are unique among input modules on a bus; instance numbers 0..31
and names are unique within their module. Input names are unique among inputs.
Output/group/input namespaces are independent: matching address/name text across
them is legal. Existing v2 and v1 import get empty inputs without a schema bump.
Missing inputs remains compatible; explicitly null inputs remains invalid.

Location adds Input {bus, short_address} and InputInstance
{bus, short_address, instance}. Every module and instance gets a projection.
Projection adds optional instance_type, required exactly on InputInstance and
absent elsewhere. Include it in binding-revision comparisons so a changed type
cannot retain an old endpoint generation. Preserve unknown types as metadata,
not as an assertion of supported behavior. No sensor units/resolution invented.

## Identity and editing

Catalog uses the existing Target codec for complete encoded-length validation.
Complete Inventory State includes targets/binding_revision on modules and nested
instances; retain the existing 512 KiB complete-document cap and 4 MiB saved cap.
History validates after the baseline and every atomic batch: each input instance
must have an active same-bus/address parent with the same encoded module name.
This catches orphan descendants and partial ancestor renames even when the final
configuration would be valid. Physical locations remain unique by enum variant.
Across a batch, a continuing child origin must retain the same parent origin.
A same-text parent replacement cannot carry unchanged child origins: all old
children must retire or restart. Compare replay records before/after each batch;
the independent consumer makes the same check without producer replay helpers.

Add pure Edit::AddInput, RemoveInput, ReplaceInput, RenameInput and
RenameInputInstance. Add/remove/replace affect module plus all descendants
atomically. Replacement starts fresh lifetimes for all of them, even with
unchanged text/addresses. Rename preserves origins and rotates the affected
mapping revisions; renaming a module rotates all descendant targets. Bus rename
continues to rotate every descendant across all namespaces. Existing immutable
proposal, revision reservation, durable save and runtime table CAS remain the
only mutation path; no direct file writer is introduced.

Expose only the two metadata rename variants through ServiceHandle and the
existing MQTT rename command. Address/type changes remain internal verified
commissioning work. Input Desired remains unsupported; module presence in the
inventory does not claim hardware availability. Future state/actions must use
these exact endpoint generations, and actions must not use retained replay.

## Verification and affected files

Change config.rs; inventory.rs, ledger.rs and edit.rs; service.rs, routing.rs and
proxy.rs; independent tests/support/inventory_consumer.rs. The independent consumer
continues to replay producer wire records without calling producer validation,
then independently checks input hierarchy, metadata and completeness. Add tests
in inventory/input_tests.rs and tests/inventory_contract.rs; service integration
uses the existing managed runtime/FakeBroker and private config directory.

Fail first on JSON acceptance/projections with literal vectors. Verify legacy
compatibility, separate address spaces, bounds/duplicates/target lengths,
rename/module/bus continuity, replacement lifetime isolation, malformed orphan
or split-parent history, parent-only replacement with surviving children,
independent consumer rename and retired-token refusal.
Verify actual MQTT rename persists to disk and complete Inventory carries the
new nested names. Keep existing output controls/regressions passing. Use focused
behavioral controls for input validation, hierarchy, descendant retirement,
replacement lifetimes and independent-consumer validation. No unrelated UART
fault campaign.

Run native suite, strict scoped Clippy, Pi compile, formatting/whitespace and wait
registry. No new waits planned. Update inventory guide, contributor instructions,
support matrix and progress with the exact boundary; common input discovery and
actual state/actions remain the following integration step.

## Self-review

Pass one checked the current target codec, all exhaustive Location matches and
the independent consumer, plus metadata admission/rename parsing. No new SDK
contract is needed. Pass two checked the opposite cases: missing vs null arrays;
same address in different namespaces; modules without instances; unknown types;
parent renamed in a partial batch; child retired while the parent remains;
same-text replacement; long percent-encoded child names. Per-batch parent checks
and optional projection type are explicit above. Existing output/group wire
documents serialize identically when inputs is empty (skip empty inputs), and
all new projection metadata is omitted on old address spaces.

Both sequential source-grounded design reviews completed. The first found the
parent-only replacement hole; the cross-batch parent-origin rule and regression
above incorporate it. The second approved that amendment and requires the
independent consumer to include instance_type in safe intent migration checks,
including a combined rename/type change. Reviews use the same model family and
did not execute tests. No user decision is outstanding.

## Implementation evidence

The named-input JSON/projection test failed before implementation on the unknown
inputs field; the other initial validation test passed. Nine new tests now pass,
including actual MQTT rename/reply, durable nested names, retired descendant
admission and independent malformed-history acceptance checks. The completed
implementation follows the file list above and introduces no new wait or task.
Both normal and post-control native gates pass 314 tests, strict scoped Clippy
and Pi library/all four binary compilation. All six new focused behavioral
controls and all six affected existing identity controls produce their intended
RED. Restored formatting, whitespace, wait registry and doctests (zero) pass.
The parent reviewed the complete production/test/doc diff; the first design
finding is covered by both producer and consumer controls. Final source reviews
will read the frozen implementation commit. No hardware or production broker
was contacted, and no shared SDK or Store code changed.
