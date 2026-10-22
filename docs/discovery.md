# Discovering addressed DALI devices

The development bridge can probe one existing short address and save newly
detected devices with default names. It queries outputs and input modules in
their separate address spaces. Use the separate [assignment operation](commissioning.md)
for new unaddressed outputs or input modules. Input assignment creates an empty
module; follow it with this discovery to add real instances. Discovery does not
reset addresses or remove missing saved items. The served Devices page composes
these addressed probes with assignment and current-state refresh.

Read `DALI/State/Controller/__inventory` for the current revision and controller
`target-generation`. Publish this JSON to `DALI/Command/Controller` with that
MQTT v5 user property and a response topic:

```json
{"command":"discover_address","bus":0,"space":"outputs","address":1,"expected_revision":1}
```

`space` is exactly `outputs` or `inputs`; bus numbers are 0..3 and short addresses
0..63. The bus must be saved and enabled. Omit `target`. Unknown/missing fields,
wrong JSON types, retired generations and changed revisions are refused. An
optional `traceparent` follows the operation and its reply.

The successful BridgeKit reply's `value` contains:

```json
{
  "probe": {
    "bus":0,"space":"outputs","address":1,"quality":"complete",
    "status":0,"device_types":[6,8],"types_complete":true,
    "instance_count":null,"instance_types":[],"instances_complete":false
  },
  "revision":2,"target":"Bus1.Outputs.Light2","added_items":1
}
```

The probe is captured evidence. It does not establish current fixture
availability after the save; use ordinary fixture refresh for current values.
`quality` is `complete`, `partial`, `missing`, `collision` or `type_mismatch`.
Incomplete type lists preserve only their verified prefix. Input discovery
reports instance count/types instead of output device types. Unknown valid input
types remain raw metadata. A module with zero instances can be saved empty.

A present new output receives the first unused `Light1`, `Light2`, ... name on
its bus. Partial type information can still establish a basic light. A new input
module needs complete instance metadata and receives the first unused `InputN`
name, with `InputN` children. `added_items` counts the module and its children,
or individually added children when expanding an existing module. No colour
representation is selected automatically.

Existing names, addresses and identities are preserved. Repeat discovery is a
no-op when there are no additions. A complete larger input list adds missing
instances only when overlapping saved types agree. A mismatch changes nothing;
a smaller list never deletes saved children. Missing hardware stays reserved.
Output and input address 0 can coexist.

Read the updated inventory after each addition before probing another address:
the controller revision/generation changes. The command that caused that change
still receives exactly one reply under its original token. To scan a bus, iterate
the wanted addresses/spaces and stop sending requests when cancelled. The served
Devices page performs this iteration for Add unaddressed and Refresh addressed.

MQTT discovery has one 10 s budget, including admission, a maximum 8 s hardware probe
and durable save. A busy state read delays admission within that same deadline;
discovery then rechecks its original revision/generation before any UART query.
Cancelled or expired waiters send no work. All query steps share that deadline
and the physical owner. Button events continue arriving. Cancelling before save skips adoption; a cancellation
or uncertain save response may require inspecting inventory and explicit
`recover`. An already committed addition is not undone if the device disappears
while saving. No automatic replacement, retry or address reassignment is implied.

Typed clients use `ServiceHandle::discover_address` with `discovery::Space`, the
expected revision, original controller `RequestContext` and absolute deadline.
Local PTY/MQTT tests and [fault controls](v2-discovery-controls.toml) cover these
contracts. Physical I4 firmware and fixtures remain unverified.
