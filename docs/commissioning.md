# Assigning an address to a new light or input module

The development bridge can assign one unaddressed output fixture or input module per request.
Existing short addresses, saved names, groups and missing-fixture reservations are
preserved. The served bus page composes discovery/assignment and current reads.
For a complete restart of selected address spaces, use
[explicit full reassignment](reassignment.md). Lost-address restoration remains
a separate unfinished workflow.

First [discover already addressed devices](discovery.md). Then read the current
inventory revision and controller `target-generation` from
`DALI/State/Controller/__inventory`. Send this JSON to
`DALI/Command/Controller`, with that MQTT v5 user property and a response topic:

```json
{"command":"assign_next_output","bus":0,"expected_revision":1}
```

The command has no `target`, address or mode field. Unknown/missing fields and
stale revisions/tokens are refused. An optional `traceparent` follows the request.
For another new fixture, repeat using the accepted revision/generation. The
served browser iterates these bounded steps; completed changes remain visible.

The result is in the BridgeKit reply's `value`:

```json
{"space":"outputs","outcome":"assigned","bus":0,"address":2,"target":"Bus1.Outputs.Light2","revision":2,"reason":null}
```

Outcomes are:

| Outcome | Meaning |
|---|---|
| `assigned` | Programming, short-address verification and a fresh status query completed |
| `reserved_but_unconfirmed` | The named address was durably reserved, but programming could not be confirmed; `reason` explains why |
| `no_unaddressed_output` | No participating unaddressed output answered; revision is unchanged |
| `no_unaddressed_input` | No participating unaddressed input module answered; revision is unchanged |
| `no_free_address` | All slots in the selected address space are saved/reserved or physically occupied; revision is unchanged |

A responding unsaved fixture occupies its address. Missing saved fixtures also
keep their reservations. Input address 0 does not occupy output address 0. New
outputs use the first unused `LightN` name. No colour selection or light state
is inferred; use ordinary fixture refresh for current values after assignment.

For an input module, use the same controller authority and fields with:

```json
{"command":"assign_next_input","bus":0,"expected_revision":1}
```

The result has `space:"inputs"`, a target such as `Bus1.Inputs.Input1` and the
raw short address. Missing saved input modules keep their address reservations;
output reservations do not consume input slots. Assignment creates an empty
default-named module with a fresh identity. It does **not** invent instances or
current values. After `assigned`, call `discover_address` with `space:"inputs"`,
the returned address and the latest confirmed revision/controller generation.
That separate operation discovers real instances under the continuing module
identity. The served browser performs both steps on the bus page.

The bridge saves a reservation **before** programming. On interruption, the new
row remains in inventory with unknown state. Inspect it before retrying: do not
assume an error or lost reply undid an address change. Disk/runtime activation
uncertainty instead returns the existing error and freezes routing until explicit
`recover`; recovery adopts confirmed inventory and never resumes programming.
The command's successful save still receives its final reply under the original
request token, even though the inventory generation changes.

MQTT has one 18 s budget inside the shared runtime's 20 s limit. The hardware
search is capped at 10 s, then programming at 4 s within the remainder. One owner
and permit cover search, save, programming and actual settlement. A bus cycle
while saving invalidates the selected candidate; a confirmed reservation remains
visible as unconfirmed. Native button events continue through the same reader.
Caller cancellation before reservation skips it; after confirmation it cannot
delete the reservation. There is no automatic address retry or persistent UART
journal. Temporary initialization mode can expire after the standard 15 minutes
or end with the next operation's initial Terminate if an operation was abandoned
between helpers. Ordinary lighting commands still work in that mode.

## HAT repeated-command profile

Output INITIALISE and RANDOMISE require a repeated frame pair. The existing transaction
layer needs a known profile for whether the HAT reports one or two terminals for
that pair. Set `--uart-repeat-terminals 1` or `2` for a measured deployment profile.
The default `unverified` permits ordinary reads/control but refuses this addressing
output sequence before it starts. Invalid values fail startup before resources open.
Do not infer the count from a firmware number or an emulator pass.

For the bundled **emulator** scenario `basic-i4.json` (`one_terminal`), use:

```sh
mqtt_dali --instance Controller --broker 127.0.0.1:1883 \
  --config /tmp/dali-demo/dali.v2.json --legacy-config /tmp/dali-demo/dali.json \
  --uart /path/printed/by/emulator --uart-repeat-terminals 1
```

The broker/address and paths are examples; use a private test broker and owned
directory when developing. Physical I4 terminal counts and fixture timing remain
unverified in this session. The [ATX manual](https://atxled.com/pdf/AL-DALI-HAT-I.pdf)
documents the repeated transmit mode; it is not a substitute for firmware evidence.

Input addressing uses the HAT's ordinary 24-bit `l` mode: two single exchanges
for INITIALISE and RANDOMISE, each pair under one fixed 100 ms host deadline.
Both exchanges must finish within it; failure stops without retry. RANDOMISE then
has a 200 ms settling wait under the original search deadline (outputs use 100 ms).
This works with the `unverified` **16-bit** repeat profile because no 24-bit `t`
mode is assumed. Host bounds and emulator results do not certify physical timing;
slow firmware may refuse. Native events continue; noisy/ambiguous exact candidate
evidence is refused rather than putting inputs into an additional silent mode.

Typed clients use `ServiceHandle::assign_next_output(bus, expected_revision,
context, deadline)` or `assign_next_input` with the same arguments and inspect
`commissioning::Report::{space,outcome}`. Local literal
UART/PTY, private MQTT, process and independent-consumer tests are complemented
by [output/shared fault controls](v2-commissioning-controls.toml) and
[input fault controls](v2-input-assignment-controls.toml).
