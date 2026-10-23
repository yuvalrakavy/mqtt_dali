# Read, assign, adopt and repair output groups

Each output can belong to any of the bus's sixteen group slots (0..15).
The saved configuration names groups and records desired membership. The hardware
can differ, including memberships in slots the bridge has not named. These APIs
make that difference visible. The protected Groups screen provides fixture
checkboxes, reviewed Add/Replace pattern assignment and bus-wide verification.
Native [group power and dimming](group-output.md) use the selected saved group's
hardware destination and leave assignments unchanged. The membership APIs below
operate on one saved fixture at a time.

## Create a saved group

Read the complete inventory from `DALI/State/Controller/__inventory`. Publish to
`DALI/Command/Controller`, substituting your controller name:

```json
{"command":"create_group","bus":0,"group":3,"name":"Kitchen","expected_revision":12}
```

Use the current **controller** `target-generation` MQTT v5 user property, a
response topic, and optional correlation data/traceparent. Bus 0 must be saved;
slot 3 must be unused. Creation saves an empty group and returns its revision.
It needs no UART and does not program hardware. Rename a saved group with the
existing `rename` command, its exact target (for example `Bus1.Groups.Kitchen`),
its current item token and the current expected inventory revision.

## Inspect before changing existing hardware

```json
{"command":"read_groups","target":"Bus1.Outputs.Ceiling"}
```

This is a read-only command. Generation and trace headers can be omitted; if a
generation is supplied, it must still match this output. The reply preserves the
original MQTT metadata, including absence. The report separately identifies the
actually resolved `target_generation`. Reads use the original two-second budget.
All physical operations require a saved output on an enabled, ready bus.

`expected_mask` is the saved desired membership. `probe.mask` is the captured
hardware membership; `drift_mask` is their XOR. Bit 0 means group 0 and bit 15
means group 15. For example mask 32770 (`0x8002`) means slots 1 and 15. Two FF
reply bytes mean all groups (`65535`), rather than an unknown value.

A probe retains nullable low/high bytes and quality `complete`, `partial`,
`missing` or `collision`. It has a full mask only when both bytes are valid.
`stale` marks evidence whose bus/session lifetime changed. The two queries are
sequential captured evidence, not an atomic snapshot against another DALI master.
They do not infer light power, presence or current availability.

For an existing installation, choose one of the following after inspecting the
mask. Both require an exact fresh match to `reviewed_mask`:

```json
{"command":"adopt_groups","target":"Bus1.Outputs.Ceiling","reviewed_mask":32770,"expected_revision":13}
```

Adoption copies this fixture's hardware memberships into saved configuration.
Previously unnamed observed slots receive unused default names (`Group1`, etc.).
Existing group names/descriptions and every other fixture's memberships remain
intact. Adoption sends no hardware settings. Repeating unchanged adoption keeps
the revision. A changed bus lifetime during saving can leave the capture marked
stale even though the metadata was successfully adopted.

```json
{"command":"repair_groups","target":"Bus1.Outputs.Ceiling","reviewed_mask":32770,"expected_revision":13}
```

Repair makes hardware match all sixteen saved bits. It can remove unnamed
hardware memberships. It rereads the reviewed mask before programming and refuses
settings if it has changed. Repair does not edit desired configuration. Review
again after a refusal; do not silently substitute a newer mask/revision.

## Change one membership

```json
{"command":"assign_group","target":"Bus1.Outputs.Ceiling","group":3,"member":true,"expected_revision":13}
```

The selected group must already be saved. Assignment first saves that one desired
membership, then programs only that bit and reads both bytes for verification.
Other saved memberships, physical group bits and fixture values remain intact.
A verified assignment can still have nonzero drift in other slots. An unchanged
desired membership keeps the revision but still checks/repairs its physical bit.
Use `member:false` to remove the selected membership.

All three mutation commands require the current **output item** generation and
current inventory revision; the controller token can differ. They preserve the
original MQTT response topic, correlation, generation and trace. Unknown fields,
invalid types/ranges and non-output targets are refused. Assign/repair require an
explicit known HAT repeat profile (`--uart-repeat-terminals 1` or `2`) before any
probe/save; read/adopt work with `unverified`. Mutations share one original
eighteen-second MQTT budget, with three-second probe and four-second program caps.
No query renews the original deadline.

## Interpret results and failures

Successful commands return a report under `value`, with `operation` (`read`,
`assign`, `adopt`, `repair`), `outcome` (`observed`, `verified`, `adopted`,
`unconfirmed`), target/generation, bus/address, confirmed revision,
`configuration_changed`, expected mask, nullable probe/drift/reason and nullable
assignment group/member. Null fields remain explicit. A read's `observed` outcome
can contain incomplete evidence. `adopted` means saved metadata; `verified` means
the selected bit or whole repair matched a fresh complete final read.

Failed admission or an unconfirmed save returns the usual error. Disk/runtime
activation uncertainty freezes changes until explicit recovery. After a confirmed
desired save, physical failure reports `unconfirmed` and retains that intent.
Repair also reports `unconfirmed` if its program phase fails. A preparation probe
is never substituted for final verification. Partial hardware writes are not
rolled back. Inspect again, then explicitly repair or adopt as appropriate.
Recovery/restart never resumes programming automatically.
An ordinary correlated DALI setting collision fails that operation and leaves
the UART usable; it does not imply lost serial ordering.

Rust callers use `ServiceHandle::read_groups` or `change_groups` with typed
`groups::Action`, original `RequestContext`, expected revision and absolute
deadline. `MetadataEdit::CreateGroup` uses the ordinary durable metadata API.
Consumers follow the [inventory contract](inventory-contract.md): group membership
changes preserve identity but rotate that group's binding token/contract intent;
unchanged output bindings and intent continue.

Local coverage uses independent literal UART peers, private brokers and the
independent emulator. Physical I4/fixture validation is pending. See
`src/service/tests/groups.rs`, `src/service/tests/groups_cases.rs`,
`tests/inventory_contract.rs` and `docs/v2-group-controls.toml`.
