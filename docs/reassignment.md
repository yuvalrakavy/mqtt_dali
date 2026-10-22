# Reassigning a complete DALI address space

For bridge operators replacing an installation or reusing previously addressed
fixtures. Ordinary **Add unaddressed** preserves existing names and addresses;
use it when adding new fixtures. **Refresh addressed** finds already addressed
gear without assigning new addresses. Full reassignment deliberately starts the
selected saved device list again. Lost-address restoration to an existing saved
fixture remains a separate unfinished workflow.

## Served browser

Open `controller/bus`, then **Bus actions → Reassign addresses**. Select outputs,
inputs or both. Review the frozen controller/bus, selected spaces and saved names.
Opening or cancelling this review changes nothing. Stop any active Blink session
before proceeding. Choose **Clear and reassign** to start.

The bridge removes saved identities in the selected space before clearing native
short addresses. This includes saved missing devices. Output removal also prunes
saved group members and private scene expectations; group and scene slot names
remain. Input removal includes its saved instances. Fresh discovery uses default
names and fresh identities, even when the same address/name is reused. Identify
outputs with Blink/Stop and rename inline; operate inputs to identify their rows.
Existing logical application bindings must be reviewed by their consuming system.

Live output levels/colour, native group bits and native scene memory are
preserved. Use the existing group/scene verification and explicit adoption,
repair or clearing workflows to reconcile those settings afterward. Other buses
and unselected address spaces keep their saved identities and settings.

Each selected space is processed as clear, assign/discover one device at a time,
then addressed/current-value refresh. Both finishes the output steps before
starting input steps. Progress stays on the device list. **Stop after current
step**, leaving the tab, disconnect or a conflicting configuration edit prevents
later submissions. Completed changes remain saved. No automatic rollback or
retry occurs. At 64 confirmed assignments, the UI reports full capacity;
additional unaddressed devices may still exist.

If clearing or assignment is unconfirmed, the batch stops. Inspect the saved
list and completed jobs before another deliberate operation. A lost browser
reply does not undo an admitted job. Refreshing after an interruption can find
addressed fixtures, but cannot restore their retired names/identities implicitly.

## Shared MQTT and typed API

The low-level command clears one space; it does **not** assign new addresses.
Any MQTT v5 consumer can compose it with the existing commissioning/discovery
commands. No Store server is required. On `DALI/Command/Controller`, provide the
current controller `target-generation`, a response topic and optional traceparent:

```json
{"command":"reset_address_space","bus":0,"space":"outputs","expected_revision":12}
```

`space` is exactly `outputs` or `inputs`; there is no `both` value or `target`.
All three parameters are required and unknown parameters are refused. HTTP uses
the same command/params through protected jobs, with the usual session, CSRF and
controller-generation requirements. The typed equivalent is
`ServiceHandle::reset_address_space(bus, space, expected_revision, context, deadline)`.

The response `value` is a count-only report, for example:

```json
{"bus":0,"space":"outputs","outcome":"cleared","revision":13,"retired_devices":3,"retired_instances":0,"checked_addresses":64,"reason":null}
```

`cleared` requires all 64 short-address status queries to return NoResponse and
the original physical evidence/current committed mapping to remain valid.
`retired_devices`/`retired_instances` count saved rows; they do not count physical
gear. `checked_addresses` counts completed status checks, including a failing
check. A HAT N alone is transport completion, not proof of device support or a
physical fixture count.

`retired_but_unconfirmed` retains the committed revision and retirement counts
with a reason. Do not continue assigning automatically from that outcome.
Persistence failure/uncertainty before activation sends no native clear and uses
the existing rejected/frozen-routing contract. Recovery never resumes clearing.
An empty saved space can still clear unsaved physical gear and can return the
same revision. Successful edits may skip reserved revision numbers; use the
exact returned revision and corresponding Inventory, without assuming +1.

After `cleared`, use `assign_next_output` or `assign_next_input` until the
appropriate no-unaddressed outcome or 64 verified assignments. Discover each
returned address, then refresh only the selected space. Keep each operation's
own accepted root; do not silently substitute a newer unrelated Inventory.

## Deployment and implementation limits

Outputs require a measured `--uart-repeat-terminals 1` or `2` profile before any
saved identity is retired. Inputs use two ordinary 24-bit exchanges under one
100 ms host deadline and can use the unverified 16-bit profile. The shared job
budget is 18 seconds; the browser composes bounded jobs for a complete bus.

Local literal UART peers, the independent physical emulator and private-broker
browser tests cover this workflow. They do not certify a real HAT/fixture or
firmware timing. See [commissioning](commissioning.md),
[the support matrix](support-matrix.md) and
[contributor fault controls](v2-reassignment-controls.md).
