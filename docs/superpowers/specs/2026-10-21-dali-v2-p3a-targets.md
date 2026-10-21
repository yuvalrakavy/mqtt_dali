# P3a canonical qualified targets

Freeze the target codec required by approved design sections 5–6 before either
MQTT or Store code depends on it. This is the first P3 slice; it does not settle
the managed-process API or request-generation fencing.

A physical target has three named components: bus, address space, item. Address
spaces are exactly `Outputs`, `Inputs`, `Groups`, `Scenes`. An input instance adds
exactly two components: `Instances`, instance name. There is no controller or
`Buses` component in a target; the controller is the BridgeKit instance. The
reserved metadata target is exactly `__inventory`, structurally separate from
physical targets. Bus/input module targets are not created by guessing a short
address. Names remain persistent public identity; addresses are separate fields.

Join components with literal dots. Encode each name's UTF-8 bytes using uppercase
percent escapes except ASCII letters/digits, underscore and hyphen. Thus literal
dots, percent, whitespace, Unicode and MQTT wildcards are encoded. Namespace
components remain the exact literal spellings above. Reject empty/whitespace-only
names, control characters, slash, and standalone `.`/`..`, consistent with current
inventory validation. Never trim, normalize Unicode, lowercase or truncate names.

Examples:

```text
Bus1.Outputs.KitchenCeiling
Bus%2E1.Inputs.Wall%20Panel.Instances.Key%2E1
Bus1.Outputs.%CF%80%25%2B%23
```

Parsing is strict and canonical: reject lowercase escapes, escaped safe ASCII,
invalid percent sequences/UTF-8, raw unsafe bytes, unknown namespaces, wrong
depth and Instances outside Inputs. Re-encoding a parsed target must equal the
original bytes. Both encode/decode reject encoded targets longer than 4096 bytes,
bounding the target itself; complete topic/packet validation remains separate.
A too-long inventory name
will cause a clear error during P3 inventory preparation, not a truncated target.

Files: `src/targets.rs`, its tests, and public library export. Implement typed
construction/parsing, namespace and instance accessors. Literal/collision vectors
cover Unicode normalization distinctions, reserved characters, namespaces,
metadata separation and bounds. Run focused tests first; defer inventory and
Store callers to their owning slices. No MQTT lane grammar changes.
