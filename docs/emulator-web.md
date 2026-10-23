# DALI Emulator browser and TCP HAT

Run an independent simulated installation:

```sh
dali_hat_emulator --scenario examples/emulator/colour-i4.json \
  --hat-listen 127.0.0.1:5580 --web-listen 127.0.0.1:5581
```

Open `http://127.0.0.1:5581` beside the bridge tab. Start the bridge with
`--hat-tcp 127.0.0.1:5580` and its own configuration/MQTT/web options. The
emulator never opens MQTT, real UART or bridge inventory. Fixture IDs remain
stable when the bridge renames or readdresses devices.

Both listeners require explicit flags. Port 0 selects a disposable endpoint;
the JSON ready record reports actual `hat_listen` and `web_listen`. Without
`--hat-listen`, the existing PTY mode remains the default. The private Unix
control socket works in either mode. Raw TCP carries unchanged ATX bytes with
one active HAT client; additional clients close immediately. Reconnect emits
fresh `B000004\n`, retains fixtures/settings, and drops prior parser/output,
faults, delayed terminals and disconnected input events. Select the same
one/two-terminal profile in the scenario and bridge.

TCP is an explicit trusted-network choice without authentication/encryption.
Web binds require loopback. Use the exact numeric address/port from the ready
record; remote web listeners and `localhost` aliases are unsupported. Every HTTP
route checks Host, including token/snapshot reads. POST additionally requires
one exact Origin and process-random `X-Emulator-CSRF` token. Responses are
uncached and provide no CORS permission. There is no default password; local
applications remain trusted. The UI reacquires its token after disconnection
without discarding its local draft or replaying a failed mutation.

## Installation and saved start

**Live installation** shows native level/DT8 colour, DT7 contact separately from
virtual level, input settings/current state and visible feedback. Lose address
retains the emulator ID. Live add/remove changes only the selected device;
untouched devices retain native state, settings and latches. Add bus is also live;
bus removal is available through a draft.

**Starting scenario** is a local draft. Add buses, output/input modules and
instances; select dimmer/DT7/DT8/buttons/absolute/occupancy/light; configure IDs,
initial addresses (blank means unaddressed), levels, group mask and scene levels.
Colour capabilities/values, queried value/resolution, feedback and optional native
settings are editable through strict JSON in the fixture editor. Unknown native
types remain available in the full record. Removing an instance explicitly
renumbers subsequent instance profiles.

Export draft downloads the local draft; Import JSON loads a local draft. Apply
draft & reset validates the complete candidate before saving it as the in-memory
start and replacing the live installation. Invalid candidates preserve live
state and the TCP connection. Applying closes the old TCP client so old evidence
cannot cross the reset. The bridge reconnects through its existing initialization.
PTY reset drains at most 64 KiB/100 ms before a delimited Boot; failure to
establish the receive fence fails that PTY session.

**Save current state as scenario** captures live addresses, groups/scenes,
levels/colours and supported native settings into the saved start. It preserves
the local draft. Download saved start exports the captured start; Reload saved
start explicitly replaces the draft. Browser actions never rewrite the CLI
scenario file or accept server filesystem paths. Save downloaded JSON yourself
and supply it with `--scenario` next time.

**Reset scenario** restores the saved start. **Reboot HAT** preserves physical
devices/settings while clearing HAT parser/output/fault schedules. Advanced
also exposes bus power, model time and bounded next-terminal faults. Timeout
does not undo an applied mutation.

## Scenario and API additions

Existing strict version-1 files remain valid. Optional capture fields:

- Output `settings`: min/max, power-on/failure levels, packed fade and virtual
  level. Ordinary `level` remains DT7 actual contact level. `colour` contains
  actual colour/capabilities; `colour_memory` stores actual control and sixteen
  native colour scene slots.
- Input `initial_settings`: complete per-instance native enabled/priority/scheme/
  filter/groups, button timers and sensor settings. `initial_physical` captures
  pressed/raw10/occupied/movement even without a queried resolution profile.

Capture excludes transient DTR/search/commissioning/latch state, elapsed clock,
transport queues, active feedback patterns and DT7 directional history. Settings
and physical states are validated before construction. Structural input edits
reset captured per-instance settings/physical defaults explicitly rather than
retaining a mismatched instance family.

New tagged controls share the actor used by the private socket:

```json
{"op":"scenario"}
{"op":"capture_scenario"}
{"op":"reset_scenario"}
{"op":"apply_scenario","scenario":{"version":1,"buses":[{"number":0}]}}
{"op":"add_bus","bus":{"number":1}}
{"op":"remove_bus","bus":1}
{"op":"add_output","bus":0,"output":{"id":"lamp","short_address":null}}
{"op":"add_input","bus":0,"input":{"id":"button","short_address":0,"instances":[1]}}
{"op":"remove_fixture","id":"lamp"}
{"op":"stimulate","id":"button","instance":0,"stimulus":{"kind":"button","pressed":true}}
```

Other stimuli: `{"kind":"value","raw_10":513}` for absolute/light, and
`{"kind":"occupancy","occupied":true,"movement":true}`. GET `/api/session`
returns the CSRF token; GET `/api/snapshot`, `/api/scenario`, `/api/examples`
return native/start/example data. POST `/api/control` accepts a tagged control.
HTTP scenario bodies are limited to 4 MiB, connections to 64, handlers to 16;
requests expire after 2 s and actor controls after 1 s. Owned shutdown cancels
accepted IO and checks its actual destruction. The socket retains its existing
16 KiB request/1 MiB response budgets.

## Semantic scope and verification

The independent physical model owns enable/filter/source scheme, button
press/release/short/double/long/repeat/stuck/free timing, sensor report/deadtime,
occupancy hold and light hysteresis. Group schemes require a primary group;
device schemes require a short address. Disabled instances/unpowered buses
emit no traffic. Physical controls still change physical state. Native priority
is stored/read back; bus arbitration physics are outside this model.

Frozen time advances explicitly. Realtime wakes within 20 ms plus scheduler
delay, independently of TCP traffic. Large jumps coalesce repeats/reports.
Values are native raw10, not calibrated lux. Queried resolutions through 32 bits
are normalized with repeated low padding; wider profiles refuse semantic stimuli
and retain explicit `set_input_value` byte controls. Type-only profiles do not
invent query support. Raw `input_event` bypasses settings and changes no queried
value; `set_input_value` changes queried bytes without an event. Fades remain
immediate. These software tests do not establish physical HAT certification.

The separate Preact source/lock/assets live in `emulator-web/`; runtime and Rust
builds need no Node:

```sh
npm --prefix emulator-web ci
npm --prefix emulator-web run build
npm --prefix emulator-web run check-dist
DALI_EMULATOR_BINARY=/path/to/dali_hat_emulator npm --prefix emulator-web run test:browser
```

The Playwright runner owns disposable loopback endpoints/process/scenario/socket
and writes desktop/phone screenshots to its printed temporary directory. It uses
Chrome (`DALI_BROWSER_CHANNEL` overrides the channel). No physical HAT or
production broker is contacted.
