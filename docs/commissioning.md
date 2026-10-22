# Assigning an address to a new light

The development bridge can assign one unaddressed output fixture per request.
Existing short addresses, saved names, groups and missing-fixture reservations are
preserved. Input-module assignment, full bus readdressing, lost-address restoration
and the approved commissioning web page remain separate unfinished work.

First [discover already addressed devices](discovery.md). Then read the current
inventory revision and controller `target-generation` from
`DALI/State/Controller/__inventory`. Send this JSON to
`DALI/Command/Controller`, with that MQTT v5 user property and a response topic:

```json
{"command":"assign_next_output","bus":0,"expected_revision":1}
```

The command has no `target`, address or mode field. Unknown/missing fields and
stale revisions/tokens are refused. An optional `traceparent` follows the request.
For another new fixture, repeat using the latest revision/generation. A browser
batch will iterate these bounded steps; no background bus job is exposed yet.

The result is in the BridgeKit reply's `value`:

```json
{"outcome":"assigned","bus":0,"address":2,"target":"Bus1.Outputs.Light2","revision":2,"reason":null}
```

Outcomes are:

| Outcome | Meaning |
|---|---|
| `assigned` | Programming, short-address verification and a fresh status query completed |
| `reserved_but_unconfirmed` | The named address was durably reserved, but programming could not be confirmed; `reason` explains why |
| `no_unaddressed_output` | No participating unaddressed output answered; revision is unchanged |
| `no_free_address` | All output slots are saved/reserved or physically occupied; revision is unchanged |

A responding unsaved fixture occupies its address. Missing saved fixtures also
keep their reservations. Input address 0 does not occupy output address 0. New
outputs use the first unused `LightN` name. No colour selection or light state
is inferred; use ordinary fixture refresh for current values after assignment.

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

INITIALISE and RANDOMISE require a repeated frame pair. The existing transaction
layer needs a known profile for whether the HAT reports one or two terminals for
that pair. Set `--uart-repeat-terminals 1` or `2` for a measured deployment profile.
The default `unverified` permits ordinary reads/control but refuses this addressing
sequence before it starts. Invalid values fail startup before resources open.
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

Typed clients use `ServiceHandle::assign_next_output(bus, expected_revision,
context, deadline)` and inspect `commissioning::Report::outcome`. Local literal
UART/PTY, private MQTT, process and independent-consumer tests are complemented
by [fault controls](v2-commissioning-controls.toml).
