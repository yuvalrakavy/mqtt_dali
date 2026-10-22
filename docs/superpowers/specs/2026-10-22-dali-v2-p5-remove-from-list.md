# P5d: explicitly remove a saved fixture from the list

## Approved behavior and grounded scope

The approved design section 10 distinguishes a missing fixture (keep it saved)
from an explicit Remove from list action (remove saved inventory, keep the hardware
address). The current pure `Edit::RemoveOutput` and `Edit::RemoveInput` already
implement removal and identity retirement. Neither has a public MetadataEdit or
MQTT entry point. Reuse those edits through the existing durable owner. No new
filesystem writer, hardware operation or Store dependency is needed.

The selected shape is an explicit per-fixture command. Automatic removal during
discovery would lose saved missing fixtures; clearing the hardware address would
change the approved operation. T3: destructive inventory/identity transition, using
the established save/CAS/completion path. Standing mission approval applies.

## Contract and effects

Add public `MetadataEdit::RemoveOutput { bus, address }` and `RemoveInput` with
the same fields. Expose WithReply `remove_from_list` with a current output or
input-module `target` and exactly one required integer `expected_revision`.
The existing target parser identifies the physical location. Reject controller,
inventory, group and input-instance targets; a module removal includes all its
instances. Unknown fields/locations, stale revision or retired/missing item token
are refused. Use the removed item's token, not the controller token. The typed
API follows the same exact item/revision authority. Disabled/offline hardware
does not block metadata removal.

Persist through the existing exclusive metadata request and absolute deadline.
Capture the original CommandCompletion so removing the target can still return
exactly one original-token/trace/correlation reply after durable activation.
Return the new revision using the standard metadata response. A repeat against a
removed target is refused; it cannot remove a successor at a reused address/name.
No new idempotency store or operation token is added.

Output removal retires only that fixture Origin. Existing `RemoveOutput` also
removes it from saved group memberships; affected groups retain Origin but rotate
binding tokens for the changed membership. Other fixtures remain unchanged.
This changes the desired saved group model, not the physical gear's membership;
the later group verification/fix workflow resolves hardware differences. Removing
an input module retires the module and every child lifetime together, leaving
outputs at the same numeric address untouched.

Normal activation retires removed target State/request authority through the SDK,
drops cached observations/listeners, syncs input routes before more UART frames,
and rejects queued notices whose targets/lifetimes no longer exist. Leave the
publisher's Desired/Config cleanup to that consumer. This bridge does not delete
logical Home equipment; P8 will surface its unresolved binding. Do not claim that
Store behavior is implemented here.

The hardware keeps its address, levels, memberships and event configuration.
Rediscovery may add the same physical address/default name with a fresh Origin
and binding revision, rather than resurrecting the removed lifetime or intent.
Failed/uncertain save and explicit recovery keep their established semantics;
recovery adopts confirmed saved metadata only, never programs hardware.

## Implementation, tests and acceptance

1. Fail-first real MQTT removal of a saved output; assert final original reply,
   durable absence, current target State retirement and rejection of old requests.
   Independent emulator still has the same physical address/level. Rediscover it
   and require a new lifetime; preserve other fixture identities and saved groups.
2. Add thin metadata/proxy/routing mappings to existing pure edits, strict payload
   tests and a typed service test proving item-token authority (controller token
   must not authorize it when the item binding revision differs).
3. Input-module test with multiple instances and a queued native action: removal
   retires all descendants, keeps output address 0, suppresses old action and
   rediscovery creates fresh child/module Origins. Independent consumer keeps
   continuing objects/intent and retires only removed lifetimes.
4. Existing save hooks cover held/failed/uncertain save and recovery; no physical
   PROGRAM, address clear, level write or group mutation occurs. Keep the global
   permit until the existing metadata request has completed.
5. Behavioral controls skip the actual edit, use controller authority, clear only
   instances instead of removing the module, accept unknown payload fields and
   retire the original MQTT reply. Run relevant existing identity/activation
   controls, restored full suite with
   four test processes (short CLI deadlines showed contention at default), scoped
   strict Clippy, Pi compilation, doctests, format/wait checks and diff review.
6. Update operator/API and inventory docs plus progress/catalog pointers. Freeze
   a source commit and run two sequential read-only source gates. Same-family
   review and no physical I4/fixture evidence remain explicit limits.

Design self-review checks the existing removal bodies, admission token resolution,
group membership side effects, subtree retirement and original-command completion
across target deletion before requesting the design gates. No new user decision
is implied by exposing the already approved operation.
