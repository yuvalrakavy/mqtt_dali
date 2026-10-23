# Blink active-color restoration and capability-aware controls

Status: implemented and verified on `e45975d` (23 October 2026).
Tier T2: extend the existing owned identification operation; no new scheduler,
transport policy, persisted schema, public message shape or Store implementation.
The owner also identified the unconditional Color control on ordinary dimmers.

## Settled behavior and chosen implementation

Capture fresh stable level/status and native active color before the first pulse.
Known non-DT8 gear uses level restoration. DT8 must provide a complete restorable
active Tc, xy or normalized RGBWAF value, including installed channels and its
RGBWAF control byte. Unknown DT8 presence, incomplete values, Primary-N or an
unsupported control mode refuse Blink before any power write. No guessed color.
User approval covers restoring active color; pending temporary color instructions
and inactive family registers are not saved or replayed. Automatic-activation settings remain unchanged.

Reuse output::Observation and its native color discovery, the identification
future, original Origin/stamp and output::WriteIntent::Combined. Add an internal
codec color intent form with optional captured RGBWAF control; ordinary RGBWAF
intents remain byte-for-byte unchanged. Reuse scenes::RgbwafControl validation and
staging commands; never store/remove/recall a hardware scene. Explicit captured
normalized control allows restoration even if staged activation changed the
current control mode. Public property writes do not acquire this capability.

At Stop, changed rename, expiry or startup cancellation, apply the captured color
with the base restoration under the existing output owner. OFF precedes activation
for originally-off fixtures; positive DAPC follows activation. Require accepted
fresh readback whose active family, native values and (for RGBWAF) control equal
the captured snapshot. Mismatch/failure ends interrupted; never report restored
from a sent command. Base readback still reports hardware clamping as before.
An unknown capture produces no restoration writes because no pulse began.

Discovery/readback can exceed the original two-second base step: retain two-second
pulse operations but allow eight seconds each for initial capture and restoration,
capped by the original caller deadline. Blink/Stop request budgets become eight
seconds. Expiry remains 120 seconds plus one bounded restoration. Original token,
Origin, physical stamp, fair admission, cancellation and foreground-write guards
remain in force. No retry, journal or extended shutdown; only captured RGBWAF control is restored.

Alternatives rejected: restoring level alone violates the approved requirement;
disabling auto-activation changes fixture configuration; separate scene storage
or a second hardware path duplicates existing ownership and consumes scene slots.

The browser renders both Color entry points only when native capabilities say
DT8, or current discovery confirms DT8 before native capabilities are available.
An explicit negative capability wins over hints. Unknown and ordinary dimmer/
relay rows show an empty cell to preserve table alignment; Read current values
remains available. Configured representation alone does not prove support.

## Grounding and verification

| Existing mechanism | Reached from | Evidence |
|---|---|---|
| capture/pulse/restore and identity checks | Identify command manager | src/service/identification.rs run / identification_output |
| native active color and features | output::execute with include_colour | src/service/output/colour.rs discover / read_values |
| activation/base ordering and accepted cache | Work::Output | src/service/output.rs execute; service/proxy::accept_output |
| raw control staging | scene plans | src/dali/scenes.rs Colour::staged_frames / RgbwafControl |
| unconditional row/menu controls | bus Devices render | web/src/devices.tsx DeviceRow |
| capability projection and generation fence | owner State -> SSE values | src/service/proxy/colour.rs capabilities; web/src/model.ts values |

Fail-first tests: independent native fixtures with pre-staged different Tc, xy,
RGBWAF values/control, including original off and dimmed states; assert restored
active color, raw level and unchanged saved configuration/settings. Unknown or
unsupported captures make no power writes. Inject ignored color restoration to
prove post-readback comparison rejects false success. Retain rename, expiry,
startup cancellation, stamp/replacement and actual clamping regressions.
Include a paced complete capture/restoration under the original eight-second
budget. Existing basic test peers must explicitly report non-DT8 capture evidence.

Real Chrome checks both viewports: dimmer and relay have neither Color entry,
DT8 controls remain accessible before and after reload/read, placeholders preserve
Blink/menu alignment and supported color editing still works. Add assertions to
the existing committed browser workflow; no new browser server or poller.

Run focused native tests, frontend build/byte check, the actual browser workflow,
then full native tests, scoped production Clippy, Pi builds, changed formatting,
wait registry and affected control-plan validation. Fault controls cover capture,
color restore/match, request budgets and UI capability guards. Use one design
verification agent, then fresh source review over the final change; same-family
models are the available limit. Keep checks and mutation runs serialized.

## Documentation and skills

Update identification operator/contributor guide, original identification spec's
superseded color/budget guarantees, support matrix, CLAUDE and progress. Correct
current claims while preserving historical acceptance evidence. No memory or
skill changes needed: this is bridge-specific behavior. Skills used are
ways-of-working, memory-recall and staged-design-review; Store UI/SDL/PanelScript
skills do not apply to this standalone Preact bridge. Physical HAT verification
remains pending and local emulator/Chrome evidence is not hardware certification.

Self-review: native active value capture and apply already exist; RGBWAF control
must be captured explicitly because pending activation can alter it. Separate
pulse and color-operation budgets avoid forcing the paced color path into a
base-only timeout. No unresolved user decision remains.

Design verification: GPT-6.1 Sol/high APPROVE (same-family, source-only).
The review requires captured intents to pass ordinary preflight before pulsing,
and fresh RGBWAF control queries in post-write read_values rather than reusing
pre-write discovery control. Both requirements are included in implementation
and regressions. No unresolved design blocker.

Final acceptance:783 native tests, the complete real phone/desktop TCP-HAT Chrome
workflow,89 frontend tests, four new and two reanchored named RED controls,
strict production Clippy, Pi builds, waits, assets, formatting and control plans
pass. GPT-6 Astra/high final source review APPROVE with no substantive blocker.
The direct initial unsupported RGBWAF-mode/Primary-N refusal cases remain a
nonblocking test-coverage gap (source guards inspected). Reviews use the same
model family; physical HAT/fixture timing remains unverified. See v2-progress.md
for measured checks and private-demo evidence.
