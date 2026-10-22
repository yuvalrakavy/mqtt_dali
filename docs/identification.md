# Identify a light with Blink and Stop

Start Blink on an individual saved output to find its physical fixture. The bridge
reads its current level, then alternates immediate maximum and off with 750 ms
dwell after each completed operation. One fixture can blink per controller. Stop
before starting another fixture. Unknown or currently fading levels cannot start
identification because the prior level must be stable.

Stop restores the captured actual level. A successful changed fixture or bus
rename stops pulsing and starts restoration automatically, preserving identity.
Invalid, failed and unchanged renames keep blinking while routing remains ready.
The rename reply confirms the saved name; observe identification status to see
restoration complete. A positive restored level can fade according to existing
gear settings; readback does not promise the fade has finished.

MQTT commands are `{"command":"blink","target":"Bus1.Outputs.Light1"}` and
`{"command":"stop_blink","target":"Bus1.Outputs.Light1"}`. Supply the selected
output's current `target-generation` and normal response metadata. HTTP uses the
same names with empty `params` in the authenticated job envelope. Neither takes
expected_revision or changes saved configuration. Both retain a four-second
request deadline. Stop on an inactive fixture performs no hardware write.

The result and snapshot's nullable `identification` carry target, generation,
phase and nullable raw actual_level. Phases are starting, blinking, restoring,
restored and interrupted. Restored reports actual readback, including clamping;
interrupted does not claim successful restoration. Snapshot/SSE phase updates
require authentication. There is no new retained MQTT scalar for this transient
operator activity.

Stop identification before ordinary power or colour writes to that fixture. The
bridge rejects those writes so restoration cannot overwrite new intent. Reads,
other fixtures and metadata remain available through shared UART admission.

Identification automatically restores after at most two minutes, plus one bounded
restoration operation. Closing the browser leaves the bounded service owner in
charge. Removal, disabling the bus, uncertain configuration or changed UART/bus
evidence interrupts rather than writing to a potentially different fixture.
Process shutdown/crash does not guarantee restoration and no prior level is saved
across restart. No automatic retry or journal is introduced.

Only basic power/level commands are sent; fade, group, scene and colour settings
are not explicitly changed. On DT8 gear with auto-activation enabled, power
commands can activate already-staged colour. The bridge preserves this native
behavior and does not switch modes or roll it back.

Local tests use the independent ATX emulator and literal PTY peers. Physical
fixture and firmware timing remain unverified. See [capability status](support-matrix.md).

## Contributor ownership

`service::identification::Manager` owns one future and bounded watch/control state
inside the main service select loop. Each internal base operation uses existing
physical Work, cache acceptance and settlement; dwell holds no permit. Metadata
waits fairly within its original deadline. Foreground writes are checked at both
submission and owner admission.

A private claim retains Origin, exact bus/address and original physical stamp.
Only a continuing Origin can supply a renamed private restoration target. Every
later physical Work requires the original stamp; public request tokens are never
refreshed. Only public status enters HTTP snapshots. Parent cancellation drops
the future before HAT/config/web close. See [design](superpowers/specs/2026-10-22-dali-v2-p5g-identification.md)
and [behavioral controls](v2-identification-controls.toml).
