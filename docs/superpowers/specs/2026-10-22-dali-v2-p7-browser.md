# P7: served responsive browser application

## Problem and selected shape

The approved phone/desktop mockups are still static sample pages. P6a–c and P5g
now supply authentication, owned jobs, actual values, native activity and Blink/
Stop. Implement the real optional UI inside the bridge, retaining its MQTT
independence and existing durable service authority. Standing architecture/UI
approval applies. This remains T3 at configuration/identity boundaries. Budget:
roughly ten minutes design/review and 40–60 implementation/browser verification;
one restored Rust suite and scoped frontend checks, new controls only.

Use the approved Preact/TypeScript/Vite stack. Local Node 24.11 supports the
official Vite requirement. Registry versions checked: Preact 11.0.1, Vite 8.3.5,
TypeScript 7.0.2 and @preact/preset-vite 2.10.6 (Vite 8 supported). Pin direct
versions and commit npm lock. Follow official Preact/Vite documentation, not a
second UI framework. No CDN, browser credential storage, runtime Node server or
Store dependency. Alternatives: a separate hosted UI violates served-by-bridge;
raw copied mockup JS duplicates state/identity logic; server templates alone
would make live stable editing and native actions harder to maintain.

Scope this increment to working screens for current backend capabilities:
sign-in, overview/bus setup, Devices, Groups, Settings and Diagnostics. Native
scenes/full reassignment/lost-address recovery remain required later slices;
show their unavailable build capability honestly, with no fake-success controls.
This is a development increment, not completion/release of the overall mission.

## Packaging, routes and authentication

Add `web/` with source modules, CSS, exact npm manifest/lock, two entry points and
a reproducible build script. Build sign-in and authenticated app independently
with Vite's programmatic single-entry/library build API, bundling their small
Preact runtime separately. Fixed login/app JS and CSS filenames avoid a runtime
manifest, shared public chunks or arbitrary filesystem serving. Refuse unexpected
output files at build time. Commit production dist assets and verify rebuilding
does not change them; Cargo/Pi compilation needs no Node/network. Rust embeds
only those explicit assets and static HTML shells with include_bytes/include_str.
No runtime external asset downloads or inline script/style/CSP weakening.

`/login` becomes the sign-in HTML; `/api/identity` retains its public JSON contract.
Only sign-in HTML/JS/CSS plus controller/version identity are public. The app
shell and app JS/CSS require the existing session middleware. Authenticated page
paths are `/`, `/buses/{0..3}`, `/buses/{0..3}/groups`, `/buses/{0..3}/scenes`,
`/settings` and `/diagnostics`; unknown paths remain 404 after authentication.
Unauthenticated page navigation redirects to fixed `/login`; APIs return 401.
No arbitrary return URL, token query, localStorage/sessionStorage or service worker.
Existing Origin/CSRF checks and handler/stream/IO budgets stay authoritative.

Login fetches only public identity, posts password once, clears its field after
success and navigates to `/`. The app obtains CSRF through GET /api/session;
keep it in memory only. Fetch uses same-origin credentials. Explicit user input
may send a throttled activity request; SSE, polling and timers never do. Logout,
401 or confirmed expired session clears all inventory/drafts/events and closes
EventSource before fixed sign-in navigation. A failed logout cannot claim the
server revoked a session. Never replay pending writes after sign-in/reconnect.

## State, identity and commands

One application controller owns accepted snapshot, job book, current session,
bounded recent native actions and request AbortControllers. Snapshot and jobs
events replace their prior complete views. Native action loss is displayed and
the next snapshot replaces values; no fabricated action replay. Reconnect obtains
a new complete snapshot before enabling actions. Network errors mark data stale
and disable mutations while retaining visible drafts/errors; never show assumed
power/input state as actual. All event handlers/timers/requests clean up on exit.

SSE's initial snapshot/jobs events are the sole authority for application views.
Do not fetch/merge an HTTP bootstrap or refresh snapshot: inventory revision
does not order physical observations. Explicit refresh submits read jobs and
waits for the stream. Each session and EventSource connection has an epoch;
late callbacks from retired epochs cannot mutate views, CSRF or drafts. On
stream error close that source, mark stale and check the existing session without
renewal before bounded-backoff reconnect. Only the new source's first complete
snapshot enables actions again. Stop/replacement clears its pending timer.

POST 202 supplies only admission/id to the initiating waiter, never replaces
the streamed job book. Completion may already be present before 202 arrives.
GET job fallback, if needed to resolve a waiter, supplies only that waiter's
terminal result and cannot rewrite stream views or downgrade a terminal job to
running. No request result is merged into actual physical State. Tests delay
202/GET replies across newer SSE views and old-session callbacks, and assert
that neither bootstrap nor refresh fetches an HTTP snapshot.

Use published item target and binding_revision, never split/construct target
syntax from display names. Form original generation from validated epoch and
safe-integer binding revision; controller operations use inventory revision.
Fail closed for unsafe numeric metadata rather than rounding authority. Expected
revision remains a JSON integer. No new backend identity/schema/dependency is
needed to accommodate an impractical >2^53 edit count.

Each command captures current target/token/revision at explicit action and uses
the existing strict job envelope. 202 means running; wait for the owned job's
actual result and then a current snapshot. Treat reserved_but_unconfirmed,
unconfirmed, missing and partial outcomes explicitly. No optimistic physical
confirmation, invisible retry, rebase after conflict, or browser-side UART owner.
For a reviewed finite multi-operation action, retain each original selected
item/token and update controller revision only through that batch's own confirmed
result. Wait for a streamed snapshot at that result revision; stop if it differs
from the expected own revision/epoch/
bus identity; external changes require a fresh user action. Closing/canceling a
batch stops later submissions but does not undo an already-admitted backend job.

Inline names use row identity from accepted inventory location/continuing ledger
Origin. Stable keys prevent live events/snapshots reordering or remounting an
edited row. Keep captured baseline item target/token/binding revision while dirty.
For individual edits capture the current controller revision once when the
explicit commit is prepared, after checking the selected item still matches its
baseline. A small serial metadata queue may prepare the next explicit commit
after the preceding save/stream update; it must not refresh a prepared command
or retry a conflict. Thus moving focus quickly between two names does not reuse
the first save's controller revision or lose the second field's focus. Checkmark,
Enter and blur share one deduplicated commit promise; pointerdown on checkmark
prevents its blur double-submit. A snapshot arriving during editing does not
overwrite the draft or refresh its authority. Failed save retains draft, inline
error, focus/edit context and Blink status. A successful save adopts the proven
continuing name and token; changed rename restoration remains visibly pending
until identification reports restored/interrupted. Filter does not hide a dirty,
invalid or in-flight row or limit discovery.
Removal/re-add at the same address and name creates a different Origin and must
not inherit the retired row's draft, error, pending write or native activity.

## Working screens

Preserve approved green/white system-font presentation. Desktop has persistent
bus navigation; narrow screens use compact rows, sibling tabs and a menu with
touchable actions. Controller name and version/build appear on sign-in/overview;
bus title is controller/bus. Accessible labels, keyboard focus, scrollable dialogs
and visible errors are required. Status indicators use actual supplied evidence:
HAT/bus state and SSE connection; MQTT status stays unknown unless exposed by the
runtime. Never copy the mockup's fabricated green MQTT status.

Overview shows configured buses and counts, with initial setup for absent bus
slots through configure_bus. Devices lists outputs and modules/instances, compact
power toggle, brightness slider (submit on release), Blink/Stop, current input
state and recent native action. Read/refresh is explicit; unknown stays unknown.
Colour controls edit the saved representation and its actual scalar values:
Kelvin, normalized xy, or individual RGBWAF channels as percentages converted to
0..1. No invented HSV picker, RGB composite physical fixture or automatic mode
conversion. Base power and colour are independent; unavailable/unsupported values
and capability limits remain explicit. Discovery reports provide observed type
hints for this session; input native types persist in inventory. Do not infer DT8
from configured representation or fabricate missing output types after reload.

Refresh addressed scans both independent 0..63 address spaces serially, preserving
existing names/addresses and missing items. Add unaddressed first performs that
addressed scan, so a first install with used/addressed gear cannot allocate an
occupied address merely because it is absent from saved inventory. It then uses
assign_next_output/assign_next_input serially until none remain, discovering each assigned address
to obtain types/real instances. Preserve reserved-but-unconfirmed items and stop
on ambiguous outcomes. Each operation is bounded by existing service deadlines;
the browser displays progress and Cancel for later steps. Filters never change
the scan scope. Inline menus offer actual Read, Remove from list (explicit scope
confirmation), colour representation and group membership. Bus settings edits
name/enabled state; advanced unavailable actions never pretend to run.

Groups uses slots 0..15 and existing create/rename/read/assign/adopt/repair jobs.
Checkbox assignment changes one bit and preserves other groups. Pattern Add/
Replace first previews the exact selected fixtures; Replace affects only that
group. Verification reads full masks including unnamed slots, shows differences
and unresolved devices, and requires explicit adoption/repair review. Re-read
after programming and retain unresolved outcomes. Settings shows startup/web
information and safe current inventory export; unimplemented import is not a
working control. Diagnostics shows supplied connection/phase, bounded job history
and native action loss, without claiming raw UART traffic not exposed by the API.

## Verification and docs

Fail first at embedded-route authentication/assets and deterministic inline-edit/
batch guard behavior. Use frontend pure tests only for real transformations and
race contracts; browser tests prove actual interaction. Drive installed Playwright
and isolated Chrome against the real bridge plus PTY emulator/private broker (no
live Store, LAN broker or /dev/serial0). Exercise 390x844 phone and 1440x1024 desktop:
sign-in/name/build, bypass/session logout, initial bus setup, three-fixture naming,
Blink/Stop restoration, checkmark+blur one revision, filters, events while editing,
actual power/colour, failed saves/reload persistence, added fixtures, removal,
groups/pattern verification and all menus/dialog scroll/keyboard behavior.
Include delayed HTTP versus newer stream events, completion before 202, old
connection/session callbacks and remove/re-add at the same address/name.
Inspect screenshots and console errors; no compile-only UI acceptance.

Run pinned npm typecheck/build/tests, reproducible dist check, scoped Rust tests,
behavioral guard controls, restored full native suite, strict scoped Clippy, Pi
four-bin compile, changed-file formatting, wait validation and affected control
plans. Two sequential design/source reviews retain same-family/source-only limits;
browser/hardware evidence is separate. Update operator/developer build guides,
support matrix, embedded asset maintenance, wait/control registry and progress.

Self-review round 1 inspected actual API envelopes, inventory targets/tokens,
auth/CSRF/SSE, mockup rendering and Node/browser availability. Round 2 checked
adjacent lifecycle/error paths: protect app assets, no passive session renewal,
stale draft authority, own-result-only batch continuation, type/MQTT unknowns and
partial commissioning results. No additional user decision is required.

First design gate (75 seconds, same-family/source-only) requested the explicit
HTTP/SSE ordering rule and same-address/name replacement draft test. Both are
incorporated; no backend sequence/schema expansion is needed.
Second gate approves (55 seconds, same-family/source-only). Isolated pinned
toolchain probe produces exactly login.js/login.css and app.js/app.css with
separate bundled runtimes; no extra chunks. Read current commissioning allocation
before coding: the Add workflow scans addressed gear first, then allocates from
the resulting saved reservations. No destructive full reassign is implied.

## As built and mandate check

- Embedded sign-in/application shells and four fixed assets; protected internal
  pages/assets, authenticated unknown-route 404 and revoked-session checks pass.
- One SSE-authoritative client unwraps the existing `inventory.Inventory`
  envelope, validates identity history/safe authority and fences retired callbacks.
  Delayed POST/GET/session replies have deterministic tests. HTTP results resolve
  waiters without replacing streamed views. Pending requests/timers/streams close.
- Stable ledger keys, original dirty item authority, deduplicated check/Enter/blur
  and serialized explicit name preparation are tested. Successful rename refreshes
  current values; a failed save keeps the draft and identification.
- Overview/setup, compact device/filter/input rows, selected-family colour forms,
  group slots/pattern preview/assignment/verification/adoption/repair, Settings and
  Diagnostics operate through existing jobs. Discovery, pattern assignment and
  verification expose progress and stop future steps on explicit cancellation.
- Real Chrome/Playwright passes at 390×844 and 1440×1024 with **308 actual owned
  commands**, private MQTT/PTy/disk evidence and inspected screenshots. Includes
  initial addressed-before-unaddressed allocation, three fixture names, failed
  save, Blink restoration, dirty input/native events, actual power/brightness/DT8,
  group drift adoption/repair, removal, rediscovery preserving other names, export
  and logout. No runtime/CSP console errors. The emulator does not hot-add fixtures;
  rediscovery covers a newly reintroduced saved address, not a physical hotplug claim.
- Fourteen frontend tests pass. Thirteen current controls have intended RED evidence.
  The first broad decoder mutation stranded an unrelated race-test setup; interrupt
  restored the tree, then target each row at its named test. An old-source guard
  initially stayed GREEN because snapshot epochs independently protected snapshots;
  add native-action and retired-error cases, then the affected five controls are RED.
- The first browser failure revealed the real Inventory wrapper mismatch; fix the
  decoder and bind its contract to the Rust snapshot test. Other fixture corrections
  use the existing managed stop API, a short private macOS socket path, wait for
  native dialog opening/enabled slider and inspect actual DT8 colour state rather
  than its immutable setup profile. None expands UART recovery.

Logs: `/tmp/dali-browser-review-fix-functional-final.log`, artifacts
`/tmp/dali-browser-c5976b6a2974c740cfd0f272e0f118d7`,
`/tmp/dali-browser-controls-final.log`,
`/tmp/dali-browser-controls-client-final.log` and
`/tmp/dali-browser-acceptance`. Restored native/Pi/lint checks and the two sequential
frozen-source verdicts are recorded in the progress entry when complete.
No hardware timing, HTTPS proxy, live Store or production deployment acceptance.

First frozen-source review (about 90 s) requests two corrections: query-only
multi-bit input display and Sign out during foreground work. Both real-browser
regressions fail first. The fixes render exact query hex/resolution without
inventing units and route logout outside the busy gate, stopping later batch
steps while reporting revocation failure honestly. Browser coverage now includes
12-bit Part 302, 64-bit Part 304 and logout with an admitted discovery job held.
The full phone/desktop run passes 308 commands and fourteen frontend tests pass.
Two additional behavioral controls are RED. Updated frozen-source verdicts follow.
