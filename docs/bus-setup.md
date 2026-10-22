# Bus setup

A fresh v2 controller starts with no saved buses. Configure each connected bus
before discovering or assigning its fixtures. This works without restarting the
bridge and without a Store server. The protected browser UI is still being built;
the current entry points are the typed API and MQTT command below.

Physical bus numbers are **0..3**. The first bus is 0, commonly named `Bus1`.
Names are editable; they do not select the HAT's physical port.

Read `DALI/State/Controller/__inventory` to obtain its `revision` and the MQTT v5
`target-generation` on that publication. Publish to `DALI/Command/Controller`:

```json
{"command":"configure_bus","bus":0,"name":"Bus1","enabled":true,"expected_revision":1}
```

Attach that controller generation as exactly one MQTT v5 user property, plus your
response topic and optional correlation data. All four JSON fields are required;
unknown fields and a `target` are rejected. A successful reply contains the current
inventory revision. The reply preserves the original command generation, trace
and correlation metadata even when the edit publishes a newer inventory generation.
Use the latest inventory revision/controller token for the next edit.

The equivalent Rust operation is
`handle.edit(revision, MetadataEdit::ConfigureBus { bus, name, enabled }, context, deadline)`.
It uses the same exclusive durable save and runtime activation as other metadata.
It is available with `--uart none` or a disconnected bus. When UART is connected,
new enabled buses join health polling in the current process. After health becomes
available, use [addressed discovery](discovery.md) or
[unaddressed output/input assignment](commissioning.md).

If the bus already exists, setup changes only its name and enabled flag. Outputs,
input modules/instances, short addresses, descriptions, groups, memberships and
color selections are preserved. An identical repeat is a no-op. Duplicate bus
names, invalid names and physical numbers outside 0..3 are rejected.

Disabling a bus keeps all saved hardware reservations and child identities. It
refuses fixture operations, retires cached light/input values and removes live
input listeners. Re-enable establishes new input receive boundaries; old queued
button actions cannot return as new actions. Actions already handed to BridgeKit
retain its normal bounded delivery behavior. New fixture State still requires
actual hardware evidence. Bus health itself remains a best-effort observation,
not a presence guarantee. With no enabled bus, the internal bus-0 health fallback
does not create a saved bus or authorize fixture operations.

Enable-only edits preserve child Origin and binding tokens. Renaming a bus keeps
Origins but moves its child targets and rotates their binding tokens. Consumers
should follow the complete [inventory identity contract](inventory-contract.md).
For example `Bus1.Outputs.Kitchen` becomes `Upper.Outputs.Kitchen`; it represents
the same saved fixture lifetime.

Save or runtime-activation uncertainty freezes changes until explicit recovery;
inspect the current inventory before retrying. Recovery can adopt a confirmed
saved bus configuration. It never assigns addresses or resumes commissioning.
Setup does not program fixtures, remove buses, or prove that a configured HAT port
exists. Physical I4 validation remains pending; local evidence uses private PTYs
and an independent emulator.

Developer verification is in `src/service/tests/buses.rs`, the HAT helper test and
`tests/inventory_contract.rs`. Behavioral controls are in
`v2-bus-setup-controls.toml`; the design is
`superpowers/specs/2026-10-22-dali-v2-p5-bus-setup.md`.
