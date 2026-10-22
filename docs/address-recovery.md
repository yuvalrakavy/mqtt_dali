# Recover a fixture that lost its DALI address

For installers and bridge consumers. Recovery preserves the original saved
fixture, its name and identity. Its short address may change. The old address
reservation becomes available when the new address is adopted.

## Identify several recovered lights

Open the controller/bus **Devices** page and use **Add unaddressed**. Missing
saved fixtures remain in the list; newly addressed fixtures appear with default
names, such as Light3 at address 12 and Light4 at address 19.

1. Blink Light3 and look for the physical light. Stop Blink when finished, or let
   the adoption confirmation stop it and restore its previous brightness.
2. Open **Actions -> Restore lost address** on the original saved fixture, such
   as Kitchen. Choose **Use this fixture** on the identified Light3 row.
3. Review Kitchen's previous address, the identified row and its new address.
   Confirm **Adopt identified address**, or **Stop blink and adopt address**.
4. Kitchen now uses address 12 and retains its saved name and bindings. Light3's
   temporary saved identity is removed, and Kitchen's previous reservation is free.
5. Identify Light4 and repeat for the other missing saved fixture.

You can also start recovery first and use **Find another candidate**. Each press
assigns and discovers one unaddressed fixture in that saved fixture's address
space. The device list remains the working screen for Blink, input state and
selection. Closing recovery leaves already assigned candidates in the list.

For an input module, operate its switches and watch current state/actions under
the candidate module. Select the module, rather than a single button. Every
original saved instance number/type must exist on the identified module. Existing
module/button identities and names continue at the new address; extra physical
instances can be discovered afterward.

The selected discovered row is retired by adoption, including its input children
or its own desired output group/scene references. The original saved fixture's
representation, descriptions, desired group membership and private scene rows
continue. Hardware levels, colour values, group membership and scene memory are
not rewritten by adopting an address. Use the ordinary group/scene verification
screens to review their native settings where needed.

## When only one fixture is unaddressed

**Restore sole unaddressed fixture** is a shortcut that explicitly associates
the sole unaddressed fixture with the original saved name and reuses its previous
address. It refuses none or multiple participants. This also works when all 64
saved addresses are reserved and no temporary test slot remains. If multiple
fixtures are unaddressed and no temporary slot is available, deliberately isolate
the intended fixture or free an unused saved reservation before identifying it.

Both paths freshly check the original address is absent. Cached missing status
does not authorize a repair. Recovery stops for changed configuration, stale
identities, connection loss, other Blink claims or uncertain evidence. Completed
saves/assignments remain saved if you close the page or stop later steps.

## API

Use the saved fixture's original `target-generation` MQTT v5 property, a response
topic, and the exact current inventory revision. The temporary candidate also
requires its own original mapping generation:

```json
{
  "command": "restore_identified_address",
  "target": "Bus1.Outputs.Kitchen",
  "candidate": "Bus1.Outputs.Light3",
  "candidate_generation": "0123456789abcdef0123456789abcdef.8",
  "expected_revision": 8
}
```

The resulting report includes the original target generation and the resulting
`current_generation`, `previous_address`, new `address`, own committed `revision`,
candidate/source address and `configuration_changed`. `restored` means the
identity mapping was recovered, not that current light/button values are known.
The browser follows ordinary discovery/reads at the new address. A failed refresh
does not undo the mapping save.

The shortcut uses:

```json
{
  "command": "restore_unaddressed_address",
  "target": "Bus1.Outputs.Kitchen",
  "expected_revision": 8
}
```

Its successful report may keep the same revision, token and address. Refusal
outcomes `no_unaddressed_fixture` and `multiple_unaddressed_fixtures` mean no
address was programmed. `unconfirmed` requires inspecting hardware before retry.
`configuration_updated_but_unconfirmed` retains the confirmed new mapping/root
but lacks current final evidence. Disk or runtime activation uncertainty uses the
normal frozen routing and explicit `recover` contract; it never schedules a
second hardware address change.

The identified operation probes only, then atomically removes the candidate and
readdresses the original saved identity. The original command still completes
under its original request generation even though the fixture mapping rotates.
Consumers replay the ledger to retain their bound device object and discard
obsolete observations. The old short address can subsequently receive another
fixture with a fresh identity.

Both commands require output/input-module targets on the same enabled bus/space,
distinct candidate addresses and exact tokens/root. No raw-address or instance
target is accepted. Authenticated HTTP jobs use the same parser with required
generation, session and CSRF. One permit and original 18-second budget cover
the whole operation; no implicit retry or rollback. Identified recovery needs
no repeated hardware settings command. The unique shortcut needs the measured
output HAT repeat profile, or ordinary 24-bit input pairs under the existing
100 ms host bound.

The PTY/emulator and browser checks are local evidence. Physical HAT/fixture
validation and actual Store object integration remain separate phases.
