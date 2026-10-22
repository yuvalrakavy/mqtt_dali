# P6a: optional authenticated web foundation

## Purpose, grounding and selected shape

Continue the approved architecture §§12–17. T3: a new authentication boundary
protects saved installation data and future physical/configuration operations.
The scope is server lifecycle, login/session/logout, protected snapshot and SSE;
HTML/Preact assets and mutating service routes follow. Remaining P4/P5 capability
work is still open. This independent foundation does not need unfinished scene,
readdress or identification APIs; full browser acceptance still needs them.

Use Axum inside DaliService's existing managed lifetime. A second daemon would
duplicate startup/shutdown and configuration authority; browser-only credentials
would not protect internal URLs. The approved stack and standing implementation
authorization choose the in-process server; no new product decision is needed.
Design budget: two grounded self-review passes and two sequential read-only
gates; same-family review limitation is explicit. Expected design/review 10–15
minutes, implementation/checks 20–30 minutes; reuse the warm build cache and
focused controls, then one restored suite. No SDK or UART recovery changes.

Verified current seams:

* `daemon::run` declares bridge-specific string options through BridgeKit and
  validates before `run_managed`; service builders open no resources.
* `DaliService::run` opens ConfigStore under its existing five-second bound,
  hands DaliProxy to `context.ready`, then owns HAT/config until bounded teardown.
* SDK `worker::run/link` calls proxy connect independently of broker connection.
  `DaliProxy::connect` attaches its Reporter to the owner; no additional broker
  bypass or lifecycle framework is needed for HTTP.
* `ServiceHandle::{snapshot,subscribe}` expose owned data and bounded updates.
  `Model::state_bytes()` is a prevalidated complete public envelope, <=512 KiB.
  Future mutations use existing typed handles and original generation/revision.
* Axum 0.8.9, sha2 0.10.9, subtle 2.6.1 and futures-util are cached locally.
  Axum's Listener abstraction permits a bounded accepted-connection wrapper.
  [Axum serve](https://docs.rs/axum/0.8.9/axum/fn.serve.html)
  owns connection handling; use its graceful shutdown, plus actual joined teardown.

The security model follows the primary
[OWASP session guidance](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html)
and [CSRF guidance](https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html):
unpredictable server-side sessions, cookie transport, server-enforced expiry,
explicit revocation, same-origin requests and a synchronizer CSRF secret.
This is local configuration access, not an identity provider or Internet exposure
hardening project. No user accounts, roles, JWTs, persistent sessions or password
change endpoint are introduced.

## Configuration and ownership

Web is disabled by default, with no listener/task/session store. Enable with
`--web-bind <IP:port>`, `--web-origin <http(s)://host[:port]>` and a nonempty
`--web-password <value>` or `MQTT_DALI_WEB_PASSWORD`. CLI password wins if both
are supplied; an environment password alone never enables web. Explicit web
options without a bind, empty credentials, invalid socket/origin, URI userinfo,
query/fragment/non-root path or password >1024 bytes are usage errors before
resources start. Reject CLI port zero; private programmatic socket tests use
ephemeral ports. Preserve actual password whitespace;
no canned/default value. Configured programmatic Settings is the embedding seam.
Inventory/retained MQTT never contains web credentials/settings.

Explicit canonical origin avoids deriving authority from untrusted Host/Forwarded
headers. HTTP is supported for trusted local deployments; HTTPS is supported via
a reverse proxy with a fixed configured HTTPS origin and loopback backend. Secure
cookie follows the configured scheme. Never infer client/origin authority from
X-Forwarded-*; document proxy example and process-list visibility of CLI passwords,
with environment/configured service credentials preferred. Direct TLS termination
is not needed when the documented proxy deployment supplies HTTPS.

Web Settings owns a fixed SHA-256 verifier of the configured process password,
compared in constant time; no password database/hash file is created. This verifier
is only an in-memory comparison mechanism, not a password-storage format. Never
derive Debug for secret-bearing Settings/Auth/Session. Do not log raw command
arguments, bodies, cookies, CSRF, tokens or password/verifier values.

The shared SDK Settings derives Debug and stores declared option values. After
the shared preflight parser validates arguments, consume/remove `web-password`
from its temporary options and hash it locally. Strip every separate/inline
web-password option/value from the argument vector passed to Bridge; do not
register that option in the runtime-only declaration loop. Preserve the shared
parser's other arguments verbatim (including literal option-looking values).
The public help still declares the option in preflight. Unit regression parses
the sanitized runtime arguments and asserts neither options nor Debug contains
the credential. No SDK Settings redaction/refactor is needed.

Add `DaliService::with_web(settings)` and optional field. After config opens and
controller identity validates, bind/spawn the server under the existing startup
ceiling and before publishing proxy readiness. Startup/bind failure closes the
config owner under the existing bound, returning managed startup failure. The
server reads the same handle even before runtime attachment; snapshot phase
truthfully shows AwaitingRuntime. No independent startup config writer.

Server is an owned task with abort-on-drop; no detached HTTP resource owner.
Unexpected server completion/panic is collected in the service loop and fails the
managed service. Begin HTTP graceful stop when service stopping starts. Existing
HAT, disk and HTTP cleanup share the same absolute 1.5 s local deadline. On expiry,
abort/actually collect the task within the parent lifecycle ceiling; never claim
clean stop with accepted connections alive. Axum graceful shutdown plus the SSE
stop signal end streams; a retained frontend handle owns no listener. Test early
startup failure, managed stop, SSE/slow clients and retained-handle teardown.

Actual cached Axum source (`serve/mod.rs:275–303,389`) spawns its signal helper
and connection tasks. Aborting only the outer server is insufficient. The bounded
accepted IO wrapper also owns a pollable `tokio_util::CancellationToken` wait;
forced cancellation wakes pending reads/writes and returns connection-aborted.
Server Drop cancels both graceful signal and forced IO before aborting the outer
task. Graceful join gets the shared deadline minus a short fixed 100 ms force/
collection reserve. On grace expiry force IO, abort/collect the outer task, then
acquire all 64 connection permits before claiming successful destruction. This
barrier measures actual accepted-IO drop, including the library's spawned tasks.
If it misses the original shared ceiling, report incomplete teardown. No deadline
renewal or claim that task abort alone closed every accepted descriptor.

## Session contract

Generate independent 256-bit cryptographic random session and CSRF values, hex
encoded. A process-local bounded map holds at most 16 sessions, each with created,
last user activity and CSRF. Do not persist it; restart/password-startup change
invalidates all sessions. Prune expired entries before admission; refuse a full
map instead of evicting an active user. New successful login rotates any valid
presented session and never adopts a client-selected token.

Cookie: `DaliSession=<opaque>; Path=/; HttpOnly; SameSite=Strict`, plus Secure for
HTTPS origin. No Domain, no URL/localStorage/Bearer token acceptance. Strictly
require exactly one valid cookie value; malformed/duplicate session cookies fail.
Logout deletes the session and clears the cookie with matching attributes.

Default idle expiry is 30 minutes and absolute expiry is 12 hours. Server monotonic
time is authoritative; at the boundary expire. Only an explicit authenticated
activity request (`POST /api/session/activity`) or an authenticated mutation updates
last activity. Snapshot reads, SSE creation/updates/heartbeats and passive session
inspection do not extend it. P7 sends activity on real user interaction, throttled;
no passive timer keeps a browser logged in. Tests inject shorter timing or paused
time without production CLI timeout/test seams.

Rate limit submitted credential checks, correct and incorrect: process-wide 10 per rolling
60-second window, bounded fixed queue. Excess returns 429/Retry-After without
checking the password. No unbounded source-IP map or forwarded-IP trust. Wrong
password returns generic 401; compare fixed-size digests. JSON body <=8 KiB,
password <=1024 bytes, strict fields. Logout/activity require both exact configured
Origin and independent `X-CSRF-Token`; login requires exact Origin and JSON.
Missing/null/duplicate/untrusted Origin fails. No CORS opt-in. All other future
state-changing routes reuse this gate; GET/HEAD never mutate physical/config data.

## HTTP contract and protected stream

All responses use no-store, nosniff and a restrictive same-origin CSP/frame policy.
No redirect return URL is accepted. Token query strings never authorize anything.
P6a has API foundation only; P7 supplies the approved login/application/assets.

| Route | Access | Result |
|---|---|---|
| GET /api/identity | Public | Controller name, version, build UTC only |
| POST /api/login | Exact Origin, JSON, rate limit | Cookie + JSON CSRF; no secret token in body |
| GET /api/session | Session, no idle touch | CSRF + idle/absolute remaining times |
| POST /api/session/activity | Session + Origin + CSRF | Refresh user activity, 204 |
| POST /api/logout | Session + Origin + CSRF | Revoke, clear cookie, 204 |
| GET /api/snapshot | Session, no idle touch | Identity, routing phase, hardware facts and complete public inventory envelope |
| GET /api/events | Session, no idle touch | Initial/latest complete snapshot as SSE `snapshot` events |
| Other /api/* | Session gate first | 401 unauthenticated; 404 after authentication |
| Protected page paths | Session gate first | Fixed /login redirect unauthenticated; 404 until P7 assets |
| GET /login | Public P6a bootstrap | Same minimal identity JSON until P7 HTML; no inventory |

Snapshot exposes no private filesystem paths, revision-high-water, Settings or
internal physical stamps. Reuse the public model envelope, with phase/hardware
mapped explicitly. It never invents fixture observations. P6b adds common
observation projections and typed mutations; P6a does not claim those are ready.
Bound serialized snapshot to 1 MiB; a refused projection is 503, not partial data.

Stream state owns a subscription, session key, stop/revocation watch and fixed
session expiry. Use a pull-based stream, no detached per-subscriber producer or
unbounded per-client event queue. Before every emitted snapshot, revalidate the
session; select also wakes for logout/expiry/stop even with no bus updates. Recompute
the idle deadline after activity; never reset absolute expiry. A timer wake after
activity may continue, while actual expiry/revocation ends the response. Any bytes
already accepted by the network before logout are not recalled; no later snapshot
is authorized. Lag resnapshots complete latest data, not event replay. Native input
actions remain a separate future stream concern, not fabricated from State.

Bound concurrent streams to 8 with nonwaiting admission. Limit active accepted
TCP connections to 64 with an Axum Listener/IO permit wrapper; release at actual IO
drop, reject excess by closing. Limit non-stream handler concurrency to 32 and
request handling/body time to 5 s. No global lock spans await: session/rate critical
sections operate on bounded maps/queues only. Slow/unread sockets cannot hold the
service owner or shutdown; exact source checks/real socket tests validate this.

## Implementation and evidence

Files: Cargo.toml/lock; new `src/web.rs`, `web/{auth,server,handlers,transport}.rs`
and tests as size warrants; `src/{lib,daemon,service}.rs`, `service/owner.rs`;
new operator guide and behavioral controls, wait registry, support/progress and
CLAUDE navigation. Do not add frontend/build placeholders or mutate Store.

Fail-first tests at real private HTTP sockets cover disabled/missing/empty/config
validation, minimal identity, arbitrary direct page/API/SSE denial, cookie-only
authentication, CSRF/origin (login/logout/activity), strict JSON, duplicate cookie/
Origin, guessed tokens, no URL/Bearer bypass, rotation/cap/rate recovery, idle and
absolute expiry on both edges, passive traffic not extending idle, live SSE logout/
idle/absolute/restart revocation, latest full snapshot, broker/HAT absence, occupied
port and bound teardown with unread clients. Pure Auth tests use controlled time;
HTTP tests use bounded requests and deterministic notifications where possible.

Controls remove the auth fallback, Origin/CSRF checks, expiry/absolute enforcement,
SSE revalidation/wakeup, idle non-touch, rate/cap and managed ownership guards.
Require named behavioral RED, restore, then focused + full native acceptance,
scoped strict Clippy, Pi all four binaries, doc/fmt/wait/diff. Dependency changes
must preserve the single SDK pin and not launch another live bridge/broker.
Review changed caller arms and errors, then freeze and run two sequential source
gates. Browser interaction acceptance belongs to P7; socket tests are not a claimed
browser pass. Log only server lifecycle and original traced operations, with the
instrumentation skill; no per-update INFO or secret-bearing records.

Self-review 1 opened daemon/service/SDK worker/managed seams and cached Axum
Listener implementation, checked fixed lifecycle bounds and local dependency
availability. Self-review 2 checked default-disabled/error paths, login CSRF,
duplicate/alternative token transport, passive-traffic idle extension, stream
expiry without updates, restart/stop ownership, map/connection/body copies and
failed startup. Fixed configured origin and explicit user activity avoid trusting
proxy headers or treating background reads as evidence of user presence.
