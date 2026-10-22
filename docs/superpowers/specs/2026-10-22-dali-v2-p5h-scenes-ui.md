# P5h3: protected Scenes screen and reviewed multi-row operations

## Purpose and shape

Finish the approved Scenes screen using the accepted private scene backend
(`33ea901`, acceptance `44bc14c`). Reuse the existing served Preact application,
bus tabs, NameField, dialogs, Progress, BridgeKit command jobs and SSE authority.
The owner already approved the mobile/desktop mockups and continued implementation.
No PanelScript, Store schema, physical recovery or dependency change belongs here.

The current Scenes tab is a placeholder. `model.ts` already replays arbitrary
ledger locations but omits scene rows; Controller has only device/group helpers.
`groups.tsx` supplies the existing pattern/verify/dialog conventions. The approved
`dali-scenes-design.html` supplies sixteen slots, compact per-fixture action and
colour controls, name filter, capture review, scoped copy/clear and verification.

One small backend reader is needed: saved settings currently require native
`read_scene`, which also queries the HAT and cannot display saved intent while
hardware is unavailable. Add explicit protected `read_scene_settings` on a named
scene, returning one current private saved view without UART IO. This is a command
reply, never retained Inventory. It uses the same strict shared MQTT/HTTP path;
no additional endpoint, data file, worker or cache service.

## Shared invariants and fast private reader

| Datum | Backend | Browser | Operator meaning |
|---|---|---|---|
| Saved contents | Private v2 file, named scene and continuing output Origins | Scoped private reply, never merged into SSE Inventory | Absent row is unmanaged |
| Authority | Original scene token + exact root revision for writes | Draft/review freezes scene/fixture keys, tokens and revision | External changes require another review |
| Physical evidence | Typed native Stored, independent of desired/actual | Verification/capture is explicit and bounded per fixture | Save is not verification; unknown is unresolved |
| Result revision | Own accepted edit advances root while mappings remain stable | Await SSE, require its exact own revision before next step | Never silently rebase onto an external edit |
| Selection | Named slot plus explicit fixture set | Freeze filter matches and all values in preview | Unselected rows/slots stay unchanged |
| Cancellation | Existing per-row deadline and settlement | Stop after current step; page/logout/slot lifetime fences later submissions | Completed saves remain saved |

`read_scene_settings` is Read-class, named Scene target, no parameters. Optional
MQTT generation must match when supplied; HTTP requires its usual generation,
session and CSRF. Validate Phase::Ready and current Scene identity without
requiring enabled bus or ready UART. Read the existing immutable Snapshot once.
Return scene target/generation, root revision, bus/slot and one entry for each
configured output with current fixture target/generation and nullable saved
Setting. No native/actual capability or verification claim is included.
The client checks the reply's exact output set/tokens and rejects duplicates or
missing entries as an incomplete view; an omitted reply row must not fabricate
an unmanaged fixture. Preserve received raw Settings without lossy display edits.

Share a pure typed helper through Operation/PreparedCommand and the MQTT proxy;
dispatch before requiring CommandCompletion. Use the normal two-second local
job budget, no service message/physical permit. Test hardware-unavailable reads,
tokenless MQTT, strict parameters/namespaces, complete mapping and continued
public snapshot privacy. Unnamed slots have no saved rows by construction;
browsing them creates nothing. A user must explicitly name/create a destination.

The complete reader can exceed the existing 32 KiB HTTP result budget: 64 valid
output targets may each occupy 4096 bytes, before settings and envelopes. Give
only this command a 1 MiB result budget; other commands retain 32 KiB. Prove a
dense, long-name scene through the real HTTP job path without truncating rows.
Job history stays bounded at 32 entries and four active jobs. Publish result-free
job summaries through SSE and GET /api/jobs, and fetch full results only through
GET /api/jobs/{id}. The client waiter fetches a completed summary's full result.
This keeps a history of large results from overflowing the existing 4 MiB SSE
event bound. Diagnostics already uses summaries only. Test both the bounded
summary book and completed-result retrieval, including failed jobs and logout.

Privacy means native contents never enter retained Inventory, HTTP inventory
snapshots or SSE snapshot events. Explicit command results are protected operator
data. All password-authenticated operators have the same configuration authority;
there is no new session-specific ownership policy. Removing results from summary
feeds is a transport-size decision, not an isolation boundary between operators.

## Screen and editing

Keep `controller/bus`, Devices/Groups/Scenes siblings and existing navigation.
Desktop uses the existing side slot list; phone uses a compact slot selector.
Show all 0..15 slots with names or Unnamed. Named scenes reuse NameField with
checkmark/Enter/focus save and continuing Origin keys. For unnamed slots, allow
an explicit default-name Create action or a typed name committed explicitly;
mere focus/browsing must not create a name. Description/removal reuse existing
metadata operations; removal explains that native fixture contents remain.

Show compact fixture cards with name/address, Blink/Stop, saved/unmanaged status,
action selector (Off, Brightness, Keep brightness, Leave unchanged) and applicable
colour fields. Keep brightness is a level MASK, independent of colour. Whole-row
Leave unchanged masks both types. Do not display an unmanaged row as a saved
ignored row. Offer Tc, xy and RGBWAF with explicit Keep colour and nullable
attribute MASKs; the backend confirms support before any save. Do not infer
all supported native families from the configured public component.

Use familiar percent/Kelvin/xy labels with checked conversion to the accepted raw
Setting. Preserve untouched native fields exactly; formatting a value must not
change it on the next save. Blank optional attribute fields mean MASK. RGBWAF
uses six named channels and normalized control; unsupported modes are not offered.
Known raw values outside an ordinary display range remain inspectable/editable
without silent clamping. Native stored words and masks are not live intents.

Drafts retain original scene/fixture authority and root revision. Show a small
checkmark for a dirty row; save on explicit check/Enter or focus leaving the row,
not when moving between colour fields. Coalesce duplicate blur/check events,
disable conflicting pending edits, and preserve invalid/failed drafts. Commit
one complete Setting, never a partially typed family. A successful result updates
private data only after its own exact SSE revision is accepted. A saved-but-
unconfirmed result stays visible for inspection/repair; it is not a success badge.

The private view is keyed by scene Origin and exact root revision. Discard late
replies after slot/page/session changes; external revisions invalidate it. Re-read
saved data cheaply without starting native verification. Preserve dirty drafts
with a conflict message instead of overwriting/rebasing them.

Add a distinct SceneItem/SceneRow type (slot, no physical short address), extend
Bus.scenes and ledger Location.slot, and narrow physical-only row props/callers in
devices.tsx as needed. NameField and metadata methods share only common identity
fields. Do not invent a short address for a scene to satisfy the old Item type.
Bind private reads and batches to the captured client Views epoch as well as
page/slot lifetime; reconnect must not continue an old reviewed batch. The cheap
saved reader uses identity/revision checks without busReady/continuation's enabled
requirement, so saved configuration is visible on disabled/unavailable buses.

Recall has explicit entire-bus/numeric-group scope, uses original scene authority
and current reviewed revision, and says sent. It changes lights; stored editing
does not. Do not infer active scene or automatically activate ignored fixtures.
Identification uses existing Blink/Stop, and must stop before settings/recall.

## Reviewed operations

Name glob filters fixtures. Bulk operations first freeze matched fixture Origins,
tokens and desired Settings, then display exact before/after entries and scope.
Use an explicit per-row request sequence, cancellation/progress, original Scene
token and strict expected revision. The existing metadata helper uses latest
revision implicitly, so new scene helpers must take captured revision explicitly.
Advance it only through this batch's accepted responses and exact SSE snapshots.
External edits, replacement, stale replies or unconfirmed outcomes stop the batch.

Capture chooses fixtures, reads actual candidates one by one with
`capture_scene_row`, and reviews frozen values/differences before Set. Unknown
candidates remain unresolved and cannot be selected for overwrite. Applying
does not recapture. Copy freezes source **saved** Settings and destination before
values, then Set on selected destination rows. Do not copy unmanaged rows as
ignored defaults. Clear previews explicit ignored Settings for selected rows;
name/create an unnamed destination explicitly before any write. Preserve other
fixtures, slots and the current lights.

Verify scene reads every configured fixture for that slot; Verify all reads all
sixteen slots with progress and cancellation, including unnamed slots. Compare
saved expectations and typed stored evidence. Display matching, different,
unmanaged or unresolved; complete raw data is not automatically writable.
Offer reviewed adoption for representable native rows and repair only where a
saved row exists. Repair/adopt send the exact reviewed Stored, and the backend
rechecks it. Selected multi-row actions use the same fenced batch runner. No
automatic repair or replay follows failure/reconnect.

Each page/operation has a captured lifetime and cancellation flag, independent
of future operations. Leaving a slot/page or logout prevents later batch steps;
already admitted work follows the normal job lifetime. Async results cannot
replace a newer selected scene or report. Browsing/filtering remains usable
while local data is shown; writes require live authority and ready hardware.

## Parts and evidence

| Part/phase | Files | Required proof |
|---|---|---|
| Private saved-view command | scene settings helper, proxy, authenticated HTTP tests | Real MQTT/HTTP reader without UART; exact authority/strict payloads; no public contents |
| Bounded job transport | jobs book, client waiter and focused tests | Dense full result succeeds, summary feeds omit results and remain bounded, explicit lookup preserves result/error and stream authority |
| Scene model/controller | web model/controller plus focused new scene helper module | Origin rename/recreate, frozen drafts/reviews, explicit revision, cancel/stale session, no unknown-to-default conversion |
| Scenes UI | new scenes.tsx/row/editor/review helpers, app routing and scoped CSS | Sixteen slots, inline name/row commits, masks/colour, native recall and all reviewed workflows |
| Build and browser | committed dist, existing browser.mjs/fixture | Unit tests, check-dist; real 390x844 and1440x1024 scenarios with disk/native state, privacy, unselected preservation, failure and no activation; inspect screenshots and console/CSP |
| Sensitivity and acceptance | current table extensions/new scene UI controls | Intended behavioral failures for saved-view authority, candidate/copy freezing, batch revision/cancel and protected assets; restored lint/Pi/wait/format/plans, source reviews, native suite |
| Docs/skills/catalog | browser/scene/service/web guides, matrix, progress, CLAUDE | Operator workflow and exact capability/limits; assess bridge-local skill guidance, no Store or user-memory writes |

Before code: finish grounded self-review and both fresh design lenses. Standing
owner implementation approval applies. All author-facing surfaces above belong
to this phase. Physical certification, full readdress/restore and Store presets
remain later phases. No new runtime recovery or shared SDK change is needed.

Self-review covers every local/native reader, unmanaged versus ignored rows,
required nullable attributes and display roundtrips, native versus configured
family, identity versus address/slot reuse, stale private replies and dirty
drafts, duplicate focus/check commits, frozen capture/copy before-values, exact
own revision advancement, cancellation/reconnect and unconfirmed outcomes. The
saved reader is a real missing presentation primitive; it avoids physical reads
just to display configuration and permits offline inspection. The existing
ConfigStore, PTY/browser fixture, job dispatcher, dialogs and epoch fencing are
reused. No infrastructure gap requires user input.

## Acceptance record

Accepted locally at frozen source `77e7ae1`, following initial `f92c496` and the
consumed-revision correction `9dac765`. Both fresh design reviews and both final
source reviews approve. Source reviewers are same-family and source-only; a
cross-family review was unavailable. Their two grounded source findings were
fixed with failing-first tests and inverse controls: valid reserved revisions
may skip numbers, and adopted results with unresolved native evidence must keep
the saved revision while stopping as unconfirmed.

All **583 native tests** across eleven binaries pass; the opt-in browser case
is run separately and passes at 390×844/1440×1024 through 325 owned HTTP commands.
The final frontend run passes **26 tests**, deterministic build/check-dist,
and inspected screenshots. Four new native tests cover the saved-only reader,
including ConfigStore restart and real retained packet privacy. All eighteen
current scene UI fault controls produce intended behavioral failures; the prior
thirteen browser controls and existing HTTP result bound also remain sensitive.
The restored 64-case scene/job/codec run, scoped strict Clippy, Pi library/four
binaries, changed-file formatting, affected control plans, wait validation and
full base diff whitespace pass. Logs: `/tmp/dali-scenes-ui-acceptance`,
`/tmp/dali-scenes-ui-restored-final`, `/tmp/dali-scenes-ui-controls`,
`/tmp/dali-scenes-ui-revision-controls`, `/tmp/dali-scenes-ui-adoption-fixed`.

Operator documentation lives in `docs/scenes-browser.md`; reference, contributor,
capability and progress guides plus bridge-local CLAUDE guidance are updated.
No new standalone/global skill is needed: the existing browser guide carries
the scene authority/result-budget/lifetime procedure and CLAUDE links it. No
Store catalog, shared skill or user-memory write belongs to this bridge slice.
Physical HAT/fixtures, actual drift repair through the browser, Store integration,
SDK publication and deployment remain outside this evidence. Browser repair
proves matching/no-change review; drift repair retains its accepted native and
literal-peer coverage. Full reassign and lost-address restore are subsequent
approved commissioning work.
