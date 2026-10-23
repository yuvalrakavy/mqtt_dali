# P5g: output identification with Blink and Stop

The original base-only color policy and four-/two-second start/restore budgets
below are historical and superseded by the owner-approved
[active-color restoration extension](2026-10-23-dali-v2-blink-color-restoration.md).
Current behavior captures/restores supported active color with eight-second
capture/restore/public budgets and exact color readback; pulses remain two seconds.
The original identity, ownership and shutdown rules remain in force.

## Problem and selected shape

The approved bus screen requires Blink/Stop while editing the fixture name.
A successful changed rename ends blinking and restores the captured output;
validation/save failure retains identification. P6c provides actual light controls
but no identification owner. T3: physical mutations, continuing identity through
rename and shared service ownership. Standing implementation approval applies.
Budget: roughly ten minutes design/review, 25–40 implementation/verification;
one restored suite, changed controls only. No new UART recovery or disk journal.

Grounding: the existing legacy setup/manager has no blink loop. Part 102 Intent
already supports immediate OFF and RECALL MAXIMUM; normalized DAPC may fade.
The typed output path owns one permit through write/readback/settlement. Holding
that permit for an entire blink session would block the approved inline rename.
Model records already retain an Origin across rename and retire it on removal.

Choose one service-owned identification future, polled beside the existing owner
loop, using short typed basic output operations and releasing the shared permit
between them. Alternatives: browser timer loops lose ownership on disconnect;
one indefinite physical Work prevents rename; a fixed short burst changes the
approved Blink/Stop workflow. No detached task, per-bus scheduler or persistent
identification state. At most one fixture blinks per controller.

## Commands and public state

Add shared MQTT/HTTP commands `blink` and `stop_blink`, each with an individual
output target, mandatory current generation and empty params. Both have a fixed
four-second request budget; no expected_revision since neither edits inventory.
Strict preparation rejects controller/group/input targets, retired authority,
unknown params and disabled buses. Runtime admission rechecks authority. A second
Blink while a session is active is Busy; explicitly Stop first. Stop without an
active session returns restored/no-op for the selected current output. Stop on a
different active fixture is refused. A successful Stop result confirms actual
restoration readback; a failed restore must never be reported as restored.

Blink first reads fresh actual level/status, refuses unknown/MASK/unavailable
or already-fading evidence, captures the exact raw 0..254 level, Origin,
location, physical stamp and original trace. Then immediately recalls maximum.
Only after current first-pulse readback does its command report blinking. Normal
OFF/RECALL MAXIMUM alternate at 750 ms dwell measured after each completion.
Never issue explicit fade, min/max, colour, group or scene settings commands.
Base-only identification operations do not query or explicitly set DT8 colour.
As with ordinary DALI power commands, OFF/RECALL MAXIMUM/DAPC can activate and
clear already-staged DT8 colour when the fixture has auto-activation enabled.
Do not disable that mode or attempt a colour rollback. Restoring uses OFF for captured
zero, otherwise DAPC for the captured raw level; its eventual fade is not claimed
complete. Expose the actual result, including hardware clamping.

Add nullable `identification` to owned service/HTTP snapshots, with current target,
generation and phase: starting, blinking, restoring, restored or interrupted.
Only the first three are active. Terminal state remains until the next command.
No physical stamp, Origin, path, credentials or free-text transport reason is
serialized. Snapshot/SSE updates carry phase changes; no new MQTT scalar property
or input action. MQTT command reply uses the same bounded result as HTTP.

## Ownership and restoration

The identification module owns its future, stop watch, status watch and pending
Stop response. The owner receives a small Identify control message (no hardware
permit held by that message), starts/stops the future and polls it in its normal
select loop. The future calls an internal basic-output submission path, retaining
the existing physical Work/settlement/cache publication. Each request waits fairly
for the existing semaphore under its own fixed deadline; ordinary output calls
retain their existing nonwaiting Busy semantics. A normal write to the identified
fixture is refused while identification is active so restoration cannot overwrite
new foreground intent. Reads and other fixtures retain existing behavior.

Metadata submission waits fairly for the permit within its original deadline
when identification is active; otherwise it keeps existing Busy behavior. The
waiter prevents the next blink pulse overtaking a rename. No new deadline or
retry after it acquires. Existing revision/generation checks repeat at owner
admission, so waiting does not rebase an obsolete edit.

After every await and before every physical request, resolve the original Origin
at its exact output bus/address in the latest Ready model. A crate-private Model
accessor exposes this existing record, without changing the persisted schema.
Continuing rename/binding change switches the private restoration target to the
new binding under the captured Origin and ends pulsing. This is an internal
continuation, never a refreshed public request. Removal/replacement, disabled bus,
uncertain lifecycle or changed physical epoch interrupts and performs no further
write to a potentially different fixture. Every post-capture physical Work must
also require the captured stamp at hardware admission; the normal original token
check remains required. This closes the send-to-owner race.

Successful changed rename commits through the existing save/activation path,
then stops pulsing and starts bounded restoration. The metadata reply means
durable rename as before; snapshot identification may briefly show restoring.
The UI must retain that status until restored/interrupted, rather than claim that
the rename reply proves restoration. Failed saves/validation and no-op rename do
not end blinking while the original routing remains Ready. Uncertain save freezes
routing and interrupts rather than inventing safe physical authority.

Stop, successful rename and a two-minute maximum session duration request one
restoration attempt under a fresh two-second cleanup ceiling, capped by a Stop
call's original deadline. First-pulse failure/cancel attempts restoration once if
the original physical evidence and Origin are still current. A pulse failure ends
identification and reports interrupted; no automatic retry/reconnect. A closed
browser does not orphan a task: the bounded service future continues to expiry.
After successful Blink reply its caller lifetime is irrelevant. During startup,
caller cancellation ends the session and restores if mutation might have begun.

Service cancellation destroys the owned future before closing HAT/config/web;
it initiates no new restoration writes during teardown. Restoration on process
crash/shutdown is not guaranteed and no prior level survives a restart. This
slice does not expand the established 1.5-second shutdown/UART recovery policy.

## Verification, controls and documentation

Fail first using independent PTY emulator fixture state, then implement. Cases:

* Known prior off/dimmed levels, real alternating writes, Stop readback restoration;
  no explicit DT8 colour/settings writes. A staged-Tc auto-activation fixture
  demonstrates the existing power-command activation effect. Unknown/fading capture
  refuses before mutation. No-op Stop never writes; malformed/stale/wrong targets
  refuse. Duplicate start and wrong-fixture Stop refuse.
* Successful fixture/bus rename preserves Origin and restores at new binding;
  invalid rename/save failure and no-op rename keep blinking. Metadata queued
  behind a held pulse gets the permit before the next pulse; no public stale
  token is refreshed. Replacement/disable/physical epoch change never restores
  into a new fixture/session. Normal writes cannot be overwritten by restoration.
* Expiry and startup cancellation restore once while healthy; Stop failure stays
  interrupted. Original trace including authoritative absence crosses every pulse
  and restoration. Cancellation collects the future before owner shutdown.
* Real HTTP jobs and snapshots show starting/blinking/restoring/terminal states;
  protected routes and existing SSE authorization behavior remain unchanged.

Each claimed guard gets a behavioral negative control where practical: captured
level, no fade mutation, current Origin/stamp, failed-rename continuation,
metadata fairness, foreground-write exclusion, stop/readback success and bounded
expiry. Controls must hit named assertions, not compile errors or timeout-only
test harness failures. Run focused service/proxy/HTTP tests, then full native
suite (four test processes), scoped strict Clippy, Pi four-bin compile, doctests,
changed-file format, wait validation and affected control plan checks. Review the
frozen source in two sequential gates. Same-family review is the available limit;
physical HAT/fixture behavior and browser workflows remain explicit pending work.

Document operator Blink/Stop/rename behavior and limits, contributor ownership,
new commands/public snapshot shape, support matrix, wait registry, controls and
progress. No SDK, Store or PanelScript implementation changes.

Self-review round 1 checked the real Part 102 immediate commands, physical Work
permit/stamp ownership, metadata activation and private ledger records. Round 2
checked cancellation/failed-save arms and adjacent colour/public-write paths:
retain the original epoch for restoration, refuse active foreground writes, and
state asynchronous restoration separately from durable rename success.

Both sequential design gates approve the amended design (117 and about 55 seconds,
same-family/source-only). The first caught the DT8 staged-colour contradiction;
the corrected guarantee and real staged-Tc regression are included. The private
Claim contains public Status plus Origin/location, allowing both submission and
owner checks to follow rename continuity without exposing that claim. Final
post-await observations reuse existing pure snapshot acceptance; rejected restore
evidence always reaches terminal cleanup.
