# Control a hardware group's power and brightness

A DALI output group is one of a bus's sixteen hardware destinations. Save a group
name and [verify its assignments](groups.md) first. Native
group commands follow the fixtures' actual hardware membership, which can differ
from the saved checkboxes. Untracked or unaddressed gear with that hardware
membership also receives the command.

## Protected Groups screen

Sign in, open **controller/Bus1 → Groups** and select a saved group. Its compact
power/brightness card keeps assignment controls on the same screen.

- **Read group values** samples membership and actual levels. Entering the page
  sends no automatic hardware read, so assignment batches retain admission.
- A uniform power value has one On/Off toggle. Mixed/unknown power offers explicit
  **On** and **Off** commands. Explicit commands do not need an earlier reading.
- **New level** and **Set group level** send a selected brightness percentage.
  The separate **Brightness** label reports actual uniform/mixed/unknown state;
  the slider's requested percentage is not an observed value.
- Readings with missing fixtures remain unknown. Membership differences direct
  you to **Verify all groups**; a level command never repairs saved assignments.

The bridge sends one native group command, then shows sampled actual results.
Off is immediate OFF; On recalls each member's configured maximum. Brightness
is DALI control-code position from0 to254, normalized to0..1, not luminous flux.
Fixtures can clamp or fade, so a sent command does not promise the requested
level has been reached. Closing/navigating cannot recall an already sent command.

## MQTT and typed API

Publish the ordinary command envelope at `DALI/Command/Controller`, with an MQTT
v5 response topic. Read may omit generation; a supplied token must match. Set
requires the saved **group's** original `target-generation` and current inventory
revision, which can differ from the controller or any fixture's token.

```json
{"command":"read_group_output","target":"Bus1.Groups.Kitchen"}
```

```json
{"command":"set_group_output","target":"Bus1.Groups.Kitchen",
 "expected_revision":23,"power":false}
```

```json
{"command":"set_group_output","target":"Bus1.Groups.Kitchen",
 "expected_revision":23,"power":true,"brightness":0.5}
```

Read accepts no parameters. Set needs power, brightness or a consistent pair.
Power must be boolean and brightness a finite number from0 to1. Nulls, unknown
fields, missing revision, contradictory values, retired/individual targets and
disabled buses reject before mutation. Active identification on this bus blocks
group writes; stop Blink first. Read admission remains available.

Read is Read-class and Set WithReply. Successful BridgeKit MQTT replies carry
the report under `value`; protected HTTP jobs return that report as their result.
Both use the same strict parser and original authority. Typed callers use
`ServiceHandle::read_group_output` / `set_group_output` with a checked gear Intent,
original RequestContext and absolute deadline. Scalar Desired group properties
and DT8 group colour are subsequent slices; this operation provides commands.
Protected HTTP retains complete group results up to 1 MiB, including long saved
fixture names. Job lists and live summaries omit the result; fetch it by job id.

## Understand the report

The report carries operation/outcome, original target/generation/revision,
bus/group, `coverage:"saved_inventory"`, nullable membership comparison, aggregate
values and a row for every output saved on that bus. Names, tokens and physical
addresses are captured once; an unvisited suffix never disappears from coverage.
Each row separates expected membership from queried actual membership and keeps
raw low/high membership, actual level and status evidence. Full FF membership
bytes and statusFF remain data; actual levelFF is MASK and cannot become maximum.
Missing replies, collisions and unvisited fields remain distinguishable.

The bridge queries individual short addresses for membership and current values.
It never expects a collision-free group level reply. Coverage is the saved
inventory, not proof that no other physical gear exists. Captures are sequential,
not an atomic snapshot against another DALI master.

| Aggregate quality | PowerState / brightness |
|---|---|
| `uniform` | All actual members in the saved coverage have complete matching levels |
| `mixed` | Levels differ; brightness is null. Power remains boolean if all are on, otherwise null |
| `empty` | No saved fixture reports actual membership; values stay null, never off |
| `partial` | Some evidence exists, but membership or a member's level/status is unresolved; aggregate values stay null |
| `unavailable` | No usable sample exists; aggregate values stay null |

JSON value fields are `power_state`, `brightness`, `power_mixed`,
`brightness_mixed`, `known_members` and `unresolved`. Known members is a lower
bound when membership is incomplete. `membership_matches:null` means unresolved;
false means at least one complete membership differs from saved intent.

Read outcome `observed` can contain partial evidence. Set outcome `sent` means
one ordinary native OFF/MAX/DAPC send completed and subsequent samples are shown;
it is not a claim that all fixtures attained the intent. Even an ignored command
reports actual earlier levels. Failed/canceled/expired writes can be
`unconfirmed`, with no current values; Refresh before relying on earlier state.
There is no automatic retry or rollback.

Work keeps one original permit/context/deadline/stamp through physical settlement,
bounded to15 seconds within the caller's deadline. Before possible mutation and
again at settlement, cached actual output/colour samples on that bus become stale.
This prevents old individual values staying current after a native group command,
including one whose caller disappeared. Ordinary individual readbacks repopulate
that cache; these reports are local evidence, not a replacement inventory/State
stream. Other buses, input measurements, queued button activity and configuration
remain intact. Base commands can activate already staged DT8 colour; no explicit
colour/scene/settings programming is part of this slice.

Local checks use independent literal peers, PTY/model, MQTT, authenticated jobs,
client guards and actual phone/desktop browsers. Physical HAT/fixture validation
and certification are not claimed. See the [support matrix](support-matrix.md)
and [progress ledger](v2-progress.md).
