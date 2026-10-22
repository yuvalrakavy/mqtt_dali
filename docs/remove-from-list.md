# Remove a fixture from the saved list

Use explicit removal when a fixture should no longer be part of this controller's
saved inventory. A missing fixture stays saved until you remove it; discovery
does not delete missing devices automatically. Removal is available through the
typed API and MQTT. The browser UI remains under development.

Read the current complete inventory from
`DALI/State/Controller/__inventory`. Take its revision and the selected fixture's
binding generation. Publish this example to `DALI/Command/Controller`:

```json
{"command":"remove_from_list","target":"Bus1.Outputs.Kitchen","expected_revision":12}
```

Attach exactly one MQTT v5 `target-generation` property containing the selected
item's token, plus your response topic and optional correlation data. The
controller token can differ from an unchanged item's token; it does not authorize
removing that item. The reply preserves the original token, trace and correlation
even though the target disappears. Success returns the new inventory revision.
Unknown fields, missing/stale revision and unknown/retired targets are refused.

Targets can be an individual output or input module, for example
`Bus1.Inputs.WallPanel`. Removing a module also removes every saved instance below
it. Groups, controllers, `__inventory` and individual input-instance targets are
not accepted by this command. Rust callers use
`handle.edit(revision, MetadataEdit::RemoveOutput { bus, address }, context, deadline)`
or `MetadataEdit::RemoveInput` with the same fields. The expected revision and
current item authority still apply. Hardware need not be connected or enabled.

Removal changes saved configuration and publishes the complete new inventory.
It retires the selected lifetime, cached values, State/request authority and input
listeners. Queued actions for a removed input cannot become actions for a later
device. Other fixture identities remain intact. Removing an output also removes
its references from saved group memberships; affected groups retain identity but
receive new binding tokens. Physical group membership remains unchanged until an
explicit [group repair](groups.md) programs it.

The physical fixture keeps its short address, level, groups, scenes and input-event
configuration. It can still respond on the bus. [Discovery](discovery.md) can add
it again, with a default name, fresh Origin and fresh binding token. A repeated
old removal request cannot remove that new lifetime. Consumers must not copy old
intent or observations into the new device merely because its address/name is the
same; follow the [inventory contract](inventory-contract.md).

The consumer owns cleanup of its retained Desired/Config topics. Logical Home
equipment is distinct from this physical device list; the planned Store adapter
will preserve that equipment and show its unresolved binding. That Store behavior
is outside this completed bridge operation.

If saving or runtime activation is uncertain, the service freezes changes. Inspect
inventory and use explicit recovery before retrying. Recovery may adopt a confirmed
saved removal; it never clears a physical address or resumes hardware commands.
Local validation uses private brokers/PTYs and an independent emulator; physical
I4 validation is pending. Tests and behavioral controls are in
`src/service/tests/removal.rs`, `tests/inventory_contract.rs` and
`docs/v2-removal-controls.toml`.
