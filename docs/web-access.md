# Optional web access: API foundation

Web access is disabled unless explicitly enabled. There is no default password.
The current development phase supplies authenticated HTTP APIs and live snapshots;
the approved Preact login/screens and configuration controls are still being built.
`/login` currently returns minimal controller/version/build JSON, not the finished UI.

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
future state-changing calls. Send the exact configured `Origin`; missing, null,
duplicate or other origins are refused. No cross-origin API access is enabled.

Sessions expire after 30 minutes of user inactivity or 12 hours absolute. Passive
reads, live events and keepalive traffic do not extend inactivity. An explicit
`POST /api/session/activity` records real user activity; future UI code will send
it on throttled interactions. Logout/restart revoke sessions, including existing
streams. Re-login never resubmits a prior mutation. A running operation may have
finished after its client lost access; inspect current inventory before retrying.

At most 16 sessions exist. Login credential checks, correct or incorrect, are
limited to ten per rolling minute across this controller; 429 has Retry-After.
The login JSON limit is 8 KiB so a maximally escaped 1024-byte password still fits.
Malformed bodies/unknown fields and ambiguous cookies/CSRF headers are refused.
All responses are no-store and carry content/frame restrictions.

| Endpoint | Access and response |
|---|---|
| GET `/api/identity` or `/login` | Public controller, version and build UTC only |
| POST `/api/login` | Origin + strict JSON `{"password":"..."}`; sets cookie, returns `{"csrf":"..."}` |
| GET `/api/session` | Cookie; CSRF and remaining idle/absolute milliseconds; no activity renewal |
| POST `/api/session/activity` | Cookie + Origin + CSRF; 204 on accepted user activity |
| POST `/api/logout` | Cookie + Origin + CSRF; revokes session/stream and clears cookie; 204 |
| GET `/api/snapshot` | Cookie; explicit routing/hardware status and complete public inventory envelope |
| GET `/api/events` | Cookie; SSE `snapshot` with initial/latest complete snapshot; closes on revocation/expiry/stop |

Unauthenticated internal APIs, including unknown API paths, return 401 without
inventory. Protected page navigation redirects only to `/login`, with no return
URL. Unknown authenticated routes return 404. Future mutation handlers will call
the existing service APIs with original revision/generation; none are exposed by
this foundation phase yet. The API does not fabricate fixture State from inventory.

Streams are limited to eight and accepted TCP connections to 64; excess requests
are refused/closed. Handlers have a five-second bound and capacity 32. Snapshot
responses cap at 1 MiB and fail as a whole if unavailable. Slow clients do not hold
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
and [progress](v2-progress.md).
