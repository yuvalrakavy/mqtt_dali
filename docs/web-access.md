# Optional web access: API foundation

Web access is disabled unless explicitly enabled. There is no default password.
The bridge serves its responsive sign-in and configuration screens directly.
Configuration/commissioning jobs reuse the existing service operations. Open the
configured origin in a browser; sign-in shows the controller name, version and
build. Phone and desktop use the same screens and saved configuration.

Use one canonical browser origin and an explicit listener. For example, in Bash:

```sh
read -r -s -p 'Web password: ' MQTT_DALI_WEB_PASSWORD
export MQTT_DALI_WEB_PASSWORD
mqtt_dali --instance Controller --broker 127.0.0.1:1883 \
  --config /var/lib/mqtt_dali/dali.v2.json --uart /dev/serial0 \
  --web-bind 127.0.0.1:8080 --web-origin http://localhost:8080
unset MQTT_DALI_WEB_PASSWORD
```

The password is exact, nonempty and at most 1024 UTF-8 bytes. Leading/trailing
whitespace is significant. `--web-password` also works and overrides the environment,
but a CLI secret is visible in the operating system's process arguments; prefer
operator-owned service environment configuration. The bridge consumes the CLI
secret before constructing shared runtime settings and never logs it. Passwords
and web settings are not written into the DALI inventory or MQTT retained data.
An environment password alone does not enable a listener.

Bind takes a numeric IP and nonzero port. Origin takes `http://host[:port]` or
`https://host[:port]`, with no userinfo, path, query or fragment. A trailing slash
and default port normalize away. The origin is the URL the browser uses; it can
differ from the backend address when a reverse proxy terminates HTTPS. Invalid
or incomplete settings fail before config import, logs, UART or listeners start.
An occupied listener fails managed startup; it does not silently choose another port.

For HTTPS, terminate TLS at your configured reverse proxy, bind this bridge to
loopback and set `--web-origin https://<your-browser-hostname>`. Proxy the same path
and allow streaming responses for `/api/events` without response buffering. The
proxy supplies its certificate and access policy; this bridge does not trust
Forwarded/X-Forwarded-* headers or derive authority from them. HTTPS origin enables
Secure cookies. Direct plain HTTP is intended for a trusted local deployment;
it does not encrypt credentials. No direct-TLS certificate implementation is claimed.

## Session and request contract

Successful login sets a random `DaliSession` cookie with HttpOnly, SameSite=Strict,
Path=/ and Secure for HTTPS. It returns a separate CSRF value in JSON. Session IDs
are never accepted in query strings, localStorage or Bearer headers. Browser code
must retain CSRF only in memory and attach `X-CSRF-Token` to logout/activity and
job submissions. Send the exact configured `Origin`; missing, null,
duplicate or other origins are refused. No cross-origin API access is enabled.

Sessions expire after 30 minutes of user inactivity or 12 hours absolute. Passive
reads, live events and keepalive traffic do not extend inactivity. An explicit
`POST /api/session/activity` records real user activity; the UI sends it at most
once a minute on explicit interactions. Logout/restart revoke sessions, including existing
streams. Re-login never resubmits a prior mutation. A running operation may have
finished after its client lost access; inspect current inventory before retrying.

At most 16 sessions exist. Login credential checks, correct or incorrect, are
limited to ten per rolling minute across this controller; 429 has Retry-After.
The login JSON limit is 8 KiB so a maximally escaped 1024-byte password still fits.
Malformed bodies/unknown fields and ambiguous cookies/CSRF headers are refused.
All responses are no-store and carry content/frame restrictions.

| Endpoint | Access and response |
|---|---|
| GET `/api/identity` | Public controller, version and build UTC JSON only |
| GET `/login`, `/assets/login.js`, `/assets/login.css` | Public sign-in page and its embedded assets |
| GET `/`, `/buses/{0..3}`, `/buses/{0..3}/groups`, `/buses/{0..3}/scenes`, `/settings`, `/diagnostics`, `/assets/app.js`, `/assets/app.css` | Cookie; protected browser application and assets |
| POST `/api/login` | Origin + strict JSON `{"password":"..."}`; sets cookie, returns `{"csrf":"..."}` |
| GET `/api/session` | Cookie; CSRF and remaining idle/absolute milliseconds; no activity renewal |
| POST `/api/session/activity` | Cookie + Origin + CSRF; 204 on accepted user activity |
| POST `/api/logout` | Cookie + Origin + CSRF; revokes session/stream and clears cookie; 204 |
| GET `/api/snapshot` | Cookie; routing/hardware status, complete public inventory and actual per-target scalar `states` |
| GET `/api/events` | Cookie; initial/latest `snapshot`, complete `jobs`, native `input` and `input_events_lost`; closes on revocation/expiry/stop |
| POST `/api/jobs` | Cookie + Origin + CSRF; strict command JSON below, 202 with job record and Location |
| GET `/api/jobs` | Cookie; `{"jobs":[...]}` in admission order; no activity renewal |
| GET `/api/jobs/{id}` | Cookie; job record or 404; no activity renewal |

Unauthenticated internal APIs, including unknown API paths, return 401 without
inventory. Protected page navigation redirects only to `/login`, with no return
URL. Unknown authenticated routes return 404. The API does not fabricate fixture
State from inventory.

## Browser workflow

Overview lists the four HAT bus slots. Choose **Set up bus**, enter its name and
enable it. Open that bus to see `controller/bus` with Devices, Groups and Scenes
tabs. Bus settings can rename or disable it while retaining saved child identity.

On Devices, **Add unaddressed** first discovers both addressed output and input
spaces, preserving saved names and reserving existing addresses. It then assigns
unaddressed gear one device at a time and reads current values. **Refresh
addressed** scans both spaces without assigning addresses or removing missing
items. Progress can stop after the currently admitted operation; completed
changes remain saved. Full destructive reassignment and lost-address restoration
are not available in this build.

Blink a fixture, type its meaningful name and press the checkmark, Enter, or move
focus to save. Those three actions share one save. Successful rename ends Blink
and restores its captured level; failed save keeps the draft and Blink so you can
correct or retry explicitly. There is no separate page-wide Save. A successful
rename reads current values again. Use `Light*` to find default names; `*` matches
any sequence and `?` one character, case-insensitively. Dirty/error rows remain
visible and filtering never limits discovery.

Rows offer actual power, committed brightness changes, Blink/Stop and an actions
menu. Colour uses the fixture's saved representation: Kelvin, normalized CIE xy,
or RGBWAF channel percentages. The form shows observed support and limits; it
does not infer a physical colour mode or convert between representations.
Unknown/stale values remain unknown. Operate an input to see its current state
and native action; a live update does not replace an unsaved name.
Queried sensor values retain exact hexadecimal data and bit resolution; dimmer
inputs also show their normalized position. Raw light-sensor data is not labelled
as calibrated lux. Sign out remains available while a command or batch is active;
it stops later browser submissions and revokes the session when successful.

Groups contains all 16 slots. Create/name a slot, assign fixtures individually,
or preview Add/Replace using a name pattern. Replace changes that group's members
only. **Verify all groups** reads every fixture's complete hardware mask and
shows matching, different and unresolved results, including unnamed slots.
Adopt or repair explicitly after reviewing; repairs are read back. A changed
configuration requires fresh verification before applying another old result.

Settings shows version/build and downloads an inventory report, which is not a
restorable configuration backup. Diagnostics shows actual HAT/browser state,
bounded jobs and recent native actions/loss; MQTT connection status is currently
unknown. Scenes are marked unavailable until their backend is implemented.
Connection loss disables operations until a new complete stream snapshot arrives.
Writes are never replayed after reconnect or sign-in. Runtime UI assets are
embedded in the executable; no Node server or CDN is needed.

## Configuration and commissioning commands

Submit the same command names/parameters documented for MQTT in the commissioning,
discovery, group and inventory guides, inside this HTTP envelope:

```json
{
  "generation": "0123456789abcdef0123456789abcdef.1",
  "target": null,
  "command": "configure_bus",
  "params": {"expected_revision": 1, "bus": 0, "name": "Bus1", "enabled": true}
}
```

Use the current inventory epoch plus revision for controller commands; use epoch
plus the selected item's binding_revision for a target command. Preserve these
original values while submitting; refreshing the generation of an old action
could select a replacement fixture. For example, a rename uses the complete
`PrivateBus.Outputs.PrivateLight` target and params `expected_revision` and `name`.
Names follow the [existing target codec](inventory-contract.md); HTTP does not
change MQTT naming or add a second encoding. Params is always a JSON object.

Supported commands: `configure_bus`, `rename_bus`, `rename`, `describe`,
`set_color_representation`, `remove_from_list`, `recover`, `discover_address`,
`assign_next_output`, `assign_next_input`, `create_group`, `read_groups`,
`assign_group`, `adopt_groups`, `repair_groups`. HTTP also supports the observation
operations below. These extra names are HTTP-only; MQTT keeps its existing
Desired and refresh forms.
Unknown envelope/parameter fields are refused. Malformed commands return 400;
retired authority or a changed revision/routing state returns 409. Refresh and
review the selection before another attempt. Authorization is checked again
after the body arrives, before any preparation or admission.

A 202 response confirms admission for an attempt, not successful programming.
Follow Location or inspect the returned id in `/api/jobs`. Records contain id,
command, target, status (`running`, `succeeded`, `failed`), result and error.
Metadata success returns a revision; discovery/assignment/group success returns
the existing structured report. Inspect its outcome and quality: a succeeded
job can report `reserved_but_unconfirmed` or an unconfirmed group operation.
Saved reservations survive incomplete physical programming. Private error text
is replaced with safe inspection guidance; errors have a stable code: busy,
closed, expired, refused, uncertain or result_unavailable.

Work keeps its original deadline (2 s metadata/recovery/group read, 10 s addressed
discovery, 18 s assignment/group mutation). Jobs own their result after an HTTP
connection closes or the initiating session logs out. Log in again to inspect.
There is no automatic retry, idempotency guarantee or persistent job journal.
If a response is lost, inspect jobs and current configuration before repeating
the command. Restart clears job history, while saved configuration survives.

Four jobs can be active, including the small mailbox; excess returns 503. The
existing service still owns one physical/configuration operation permit and may
report Busy. History retains up to 32 jobs, evicting oldest completed records;
running records are retained. Encoded sanitized results cap at 32 KiB as a whole.
All authenticated sessions share this controller's history. Passive polling does
not keep sessions alive; authenticated submissions record explicit user activity.
Stopping the server drops its owned command futures through existing service
cancellation semantics, including uncertainty after an interrupted save.

## Light controls and current input values

`blink` and `stop_blink` also use individual output targets and empty params.
They start/stop controller-owned identification with four-second request budgets.
Snapshots include nullable `identification` status; successful rename starts
restoration, whose completion is separate from durable rename success. See
[Blink and Stop](identification.md) for the two-minute limit and DT8 behavior.

Use the selected item's original generation and complete target for these jobs.
They do not change inventory and take no expected_revision:

| HTTP command | params | Result |
|---|---|---|
| `apply_output` | `{"values":{"PowerState":true,"Brightness":0.4}}` | Accepted actual scalar state after physical readback |
| `read_output` | `{}` | Actual base/selected-family colour values and quality |
| `read_input` | `{}` | Current individual input-instance values, source and resolution |

Only individual outputs/instances are accepted. Apply values reuse the complete
MQTT scalar contract: booleans for PowerState, 0..1 brightness/RGBWAF, Kelvin
temperature and normalized xy. Colour fields must belong to the saved
representation. An invalid member rejects the entire map before any write.
Colour-only changes preserve master level; omitted channels/xy coordinates
preserve their physical values. Read the output after selecting a representation
to discover physical capabilities before applying colour. See
[output/colour contract](mqtt-outputs.md).

The result is `{"target":"...","generation":"...","values":{...}}`.
Brightness and colour are actual readback, including quantization/clamping;
they can differ from the request. Base and colour availability/quality remain
independent. The existing 2/6/8-second output budgets and four-second input read
budget begin at preparation and are never extended by queueing. A rename or
receive change before completion cannot turn old evidence into a new item result.

`states` in the complete snapshot is an array of the same record shape. Replace
previous state when a new snapshot arrives. Missing base values and explicit
nulls are unknown/unavailable; retaining an older value would misrepresent a
disabled or changed fixture. Querying an input never fabricates a button action.
Input values expose ValueSource, exact InputValueRaw hex and ValueResolution up
to 255 bits; do not convert wide hex into a JavaScript Number. Contact InputState,
button Status, occupancy and raw light values follow [the input contract](mqtt-inputs.md).

## Live activity and job changes

An SSE connection begins with a complete `snapshot` and `jobs` event. Subsequent
updates coalesce complete snapshots/job books; they are not incremental patches.
For `jobs`, replace the previous list, including completed results. A completed
job may already be present by the time an admission notification is read.

`input` contains the native action payload plus its original generation. Only
the exact current endpoint and receive lifetime can emit it. Button actions
include pressed, released, short_press, double_press and the native long-press
variants. These identify actual activity independently of the latest retained
state; a short press need not imply that the button is still held.

`input_events_lost` carries a dropped count when the bounded native receiver
falls behind, followed by a current snapshot. Show missed activity explicitly;
do not reconstruct missing presses from state. Streams rotate ready snapshot,
job and native sources so a busy status feed cannot hide switch activity.
There is no historical input replay or Last-Event-ID resume. Reconnect supplies
current values/jobs and newly observed actions, with the same authentication and
expiry checks before every event kind. Background activity does not renew login.

Streams are limited to eight and accepted TCP connections to 64; excess requests
are refused/closed. Handlers have a five-second bound and capacity 32. Snapshot
responses cap at 4 MiB and fail as a whole if unavailable. The MQTT inventory
ceiling stays 512 KiB. A local dense-inventory probe (522,331 inventory bytes,
1,824 initially unobserved states) produces a 1,054,621-byte HTTP snapshot; this
is a measured example, not a maximum-size proof for every valid configuration.
Slow clients do not hold
the UART/config owner. Broker/HAT absence still permits protected saved inventory
and truthful degraded status. The managed service owns startup and shutdown;
accepted IO must actually drop before it reports successful cleanup.

## Contributor verification

Tests use private loopback sockets, FakeBroker or a private sink that never sends
CONNACK, and no physical UART. They cover direct bypasses, CLI/environment
credentials and Debug/log privacy, fixed origin, cookies, CSRF, expiry/activity,
live revocation, capacity, inventory updates, occupied bind and cleanup with
incomplete uploads/retained handles. [Behavioral controls](v2-web-controls.toml)
temporarily remove guards and require named failures before restoring source.
Browser/mobile/desktop workflow acceptance belongs to P7; socket tests do not
claim it. See [design](superpowers/specs/2026-10-22-dali-v2-p6a-web-foundation.md)
and [command-job design](superpowers/specs/2026-10-22-dali-v2-p6b-web-commands.md).
Command tests also exercise the independent PTY emulator and held/failed saves.
[Job controls](v2-web-job-controls.toml) cover authority, reauthorization, bounded
ownership/results and privacy. [Observation controls](v2-web-observation-controls.toml)
exercise real scalar application, final mapping/readback acceptance, native
receive cutover/loss, fair stream progress and live job updates. See
[the observation design](superpowers/specs/2026-10-22-dali-v2-p6c-web-observations.md)
and [progress](v2-progress.md).
