# Building and testing the served browser UI

Output **Fixture settings** uses `OutputSettingsScope` and a compact modal under
the existing row actions. Freeze the original row/address/token/root and view
epoch; revoke synchronously on dismissal and in layout cleanup. Read on entry,
save changed fields only and require coherent raw readings plus actual requested
values. Missing families remain disabled; a readable packed fade with reserved
rate0 allows repair. Percentage conversion preserves all255 ordinary native
codes0..254. Retire actionable values before possible saves; failed/unconfirmed
replies require explicit Refresh. Matching identification blocks Save, not Read.
Use functional draft setters: blur and dropdown changes may be batched, and a
captured draft can silently erase another field's pending edit. The actual browser
flow checks both crossing limit orders, all six fields, Off retained, reopen,
lost replies, explicit refresh, held Cancel/Close/navigation and exact restoration
against native snapshots and unchanged inventory.

Input-instance **Event settings** uses `InputSettingsScope` and its own compact
modal. Freeze row, token, root and connection epoch; recheck after each awaited
job. Job reports are local hardware evidence, never inventory/State authority.
Read only on entry; patch only changed fields; require complete exact readback
before showing success. Unknown filters are read-only. Explicit dismissal revokes
synchronously and layout cleanup revokes on navigation. The actual browser flow
checks phone/desktop save/reopen, retained inventory, sensor labels and held entry
read cancellation. Dialog-bound assertions use the actual viewport dimensions.
Timer/sensor text updates on input and remains separate from its parsed native
draft, so another control's render cannot erase an uncommitted number or an
intermediate hold value. Merge nested timers, sensors, filters and groups from
current state, capturing DOM values before deferred setters. Actual browser cases
edit numbers before dropdowns, retain invalid intermediate hold text and prove
that an invalid mixed patch sends no job or physical change. Save/reopen/restore
checks compare complete native profiles. Controls: `v2-input-draft-controls.toml`.

The input-settings rejection test varies one failure at a time: keep actual
settings equal to the requested patch except in the explicit mismatch case.
Otherwise a mismatch guard can hide a broken outcome/authority check. Intentional
controls in `v2-managed-input-settings-controls.toml` exercise those guards.

Part301 timing uses `button-timing.ts` within that same dialog and frozen scope.
Keep raw ticks in the draft; show exact milliseconds (20ms steps) or stuck seconds.
Validate reported minima and declared complete values independently, preflight
before job submission and compare requested timers after readback. Partial timer
evidence leaves common-only settings usable. The real phone/desktop flow fills,
saves, reopens and restores all four timers, verifying native raw bytes and
unchanged inventory. For no-submit assertions, check the actual command count
before checking the error message, so another validation guard cannot hide I/O.
Controls: `v2-managed-button-timing-controls.toml`.

Part302–304 fields use `sensor-settings.ts` in that same frozen scope. Keep raw
bytes in the draft, show report seconds/deadtime50ms steps, occupancy hold1s or
10s increments, and light hysteresis percent/minimum sensor input units. Hold's
valid display set is1 or10..2540 in10s increments; do not rely on HTML step alone
or round a value. Preserve unsupportedFF versus missingnull; an unsupported hold
does not disable report/deadtime. Validate family, support, ranges, complete
evidence and actual requested values. The real phone/desktop flow edits/reopens/
restores all three families, checks raw native settings and unchanged inventory,
and scrolls the actions into view. Controls: `v2-sensor-settings-controls.toml`.

Input module and instance **Feedback** uses `FeedbackScope` and its compact
modal. Capture the selected row's own token and physical scope, root/revision,
connection epoch and lifetime. Validate returned scope/type, complete feature/
capability/active consistency and requested actual state. Unknown or unsupported
evidence cannot submit an action; do not optimistically flip the indicator.
Dismiss synchronously and revoke in layout cleanup on navigation. Real phone/
desktop cases read/toggle/reopen/restore module and multiple instance profiles,
verify unsupported inputs offer no action and hold entry reads across actual
dismissal/navigation. Compare native profile/settings/values and saved inventory.
The generation test varies the selected instance token for an instance, rather
than only changing its parent's token. Controls: `v2-managed-feedback-controls.toml`.

The Groups title's power/dimming card uses `GroupOutputScope`. Entering or
remounting a saved group performs no hardware read; only explicit operations use
admission, so membership assignment batches do not compete with automatic reads.
Each click freezes the selected group, original token/revision, full saved output
roster, connection epoch and component lifetime. Recheck after awaits; retire the
scope on slot/generation/navigation unmount. Hide values after retirement or before
I/O. Validate every captured member identity and raw reading, then independently
check the aggregate against the whole roster. An omitted suffix cannot become
uniform/empty. Power may be uniform while positive levels differ; never average
brightness or echo a sent intent. Unknown aggregate values leave explicit native
On/Off/level commands available. The slider is labeled New level, separate from
observed brightness. Actual phone/desktop flows exercise native selection, mixed
values, missing addresses, explicit writes without prior values, unchanged saved
configuration/other fixtures, retirement across slot/navigation and actual level
restoration. Controls: `v2-group-output-controls.toml`.

For bridge contributors: `web/` owns the responsive Preact/TypeScript application.
Node is needed to change assets, not to run the bridge or compile its embedded
assets with Cargo. Use Node 24.11 or newer with the exact npm lockfile:

```sh
npm --prefix web ci --ignore-scripts
npm --prefix web test
npm --prefix web run build
npm --prefix web run check-dist
```

Commit `web/src`, HTML shells, manifest/lock and all four `web/dist` files.
`check-dist` rebuilds into a temporary directory and compares bytes with committed
assets. The build refuses unexpected chunks. Rust serves a fixed whitelist;
sign-in assets are public, application assets use the same session middleware as
internal pages. Preserve the self-only CSP; do not add inline script/style,
credential storage, a filesystem asset server or public shared application chunks.

The browser client unwraps `snapshot.inventory.Inventory`, the existing published
State envelope. SSE is the sole authority for view snapshots and job books.
POST 202 admits a job; SSE job summaries omit results. GET job lookup resolves
its waiter, including a completed streamed summary. Neither may
overwrite a newer streamed view. Connection/session epochs fence late callbacks.
No passive traffic renews user activity; requests, timers and streams stop when
the client closes. Native action history is bounded and loss is explicit.

Rows replay identity-ledger transitions atomically to retain continuing Origin
keys across renames. A remove/re-add gets a fresh key. Dirty drafts keep the
original item target/token; explicit queued commits prepare their controller
revision once, and conflicts require another user action. Reviewed batches can
continue only through their own confirmed revisions, original bus and selections.

The opt-in functional browser test starts the real managed bridge with a private
MQTT broker, independent PTY emulator and private Unix control socket. It uses
pinned Playwright and local Chrome; set `DALI_BROWSER_CHANNEL` to another installed
Playwright channel if appropriate. It never connects to a live broker or UART:

```sh
cargo test --offline --target aarch64-apple-darwin --lib \
  browser_functional_phone_and_desktop -- --ignored --nocapture
```

The test prints its private `/tmp/dali-browser-…` artifact directory containing
screenshots and browser log. It exercises 390×844 and 1440×1024 viewports through
real DOM interactions and checks resulting disk/physical state, including failed
save injection, live inputs, DT8 and group drift review. Scenes covers Tc/xy/
RGBWAF values and MASKs, inline save/retry, frozen capture, scoped copy/clear,
native adoption, matching repair review, group recall, all sixteen slots,
metadata/privacy and cancellation when leaving the page. A held creation reply
cannot erase a later slot's name draft. Native hardware drift/repair execution
has separate service/literal tests; the browser repair case uses matching data. Inspect screenshots and
console errors; a successful compile alone is not UI acceptance. Routine Rust
suites skip this explicitly ignored browser dependency.
Full reassignment cases exercise cancelled frozen review, outputs/inputs/both,
fresh names, reference pruning, preservation of native live/group/scene values,
and leaving the tab during an admitted reset.
Recovery cases cover unique repair, multiple-candidate refusal, normal scan-first
candidate selection, confirmed Blink stop followed by adopting the new address,
input activity/child continuity, freed old reservations and tab cancellation.
Individual-clear cases exercise cancelled phone review, output/input confirmations,
confirmed Blink stop, retained saved identity/native settings and explicit recovery.
The ClearAddress wrapper freezes a Restoration scope; dialog closure and route
unmount permanently cancel it. It accepts only its original unchanged mapping/root
and a confirmed `cleared` result; it submits no automatic rediscovery.
Dismissal must revoke the scope in its event handler. ClearAddress lifetime
cleanup uses `useLayoutEffect`, because passive `useEffect` can run after an
awaited reply has resumed. Test the actual Cancel/Close/navigation handlers with
an admitted Stop reply held and browser timers/animation frames paused; directly
setting a test scope's cancellation flag does not test that lifecycle wiring.
Query-before-event cases include multi-bit Part 302 inputs and exact 64-bit
Part 304 sensor data. The logout case holds a real job admission while discovery
is busy, then verifies session revocation and absence of later batch commands.

Run `docs/v2-browser-controls.toml`, `docs/v2-scenes-ui-controls.toml` and
`docs/v2-reassignment-controls.toml` and `docs/v2-restoration-controls.toml` with
the existing Store negative-control runner
to prove the frontend authority/race guards and protected embedded routes detect
their intended mutations. Mutations, source edits and verification must remain
serialized; rebuild assets after source edits and finish with `check-dist`.
The scene creation control rebuilds mutated assets and runs the real browser;
the runner restores its source, so rebuild assets after that control too.

Scene helpers live in `scene-settings.ts` and `scene-controller.ts`. Keep native
Setting distinct from live output values and unmanaged null distinct from MASK.
The saved-view decoder requires every exact output/token. SceneScope captures
original Scene/fixture identities, root revision and connection epoch; it advances
only through its own result and exact SSE revision. Do not use metadata()'s
latest-root injection for reviewed scene batches. Drafts preserve untouched raw
codes and coalesce row blur/check commits. Keep operation snapshots only for the
lifetime of their draft, review or batch. See the [operator guide](scenes-browser.md).


`reassignment.ts` freezes the selected bus/root/connection and composes one
space at a time. Do not call the ordinary both-space scan or metadata()'s latest
root injection. Assignment accepts its own returned revision, including skipped
reservations, and waits for the exact SSE root. At 64 confirmed assignments,
capacity is a normal stop; no extra free-address search is required. BusActions
is keyed by number and tab so an old completion cannot start later steps after
navigation. Its operation cancellation is independent of global batch flags.

`restoration.ts` freezes original/candidate identity, exact root and connection
epoch. Candidates can already exist from normal discovery or be added by its
single-step Find action. After the exact own result plus SSE, identified recovery
updates its expected address/token and refreshes there; it must not reassert the
previous token or refresh the old address. Unique repair accepts the same root
and token. A failed refresh reports the confirmed recovery separately. Devices
unmount permanently cancels the operation regardless of later global flags.
