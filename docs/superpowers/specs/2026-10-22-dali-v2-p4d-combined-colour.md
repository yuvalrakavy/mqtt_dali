# P4d4: automatic activation and combined colour/power operations

## Problem and scope

The reviewed colour service currently rejects gear reporting automatic activation
and has no combined colour/base write. That internal foundation cannot yet serve
the full scalar MQTT light interface. Complete the common owned service behavior
first, preserving the original permit, physical helper, deadline, stamps, final
acceptance and native input reader. This is T3 because it extends one physical
operation and its uncertainty/success contract. Standing implementation approval
applies. One service/emulator batch, focused fail-first tests, controls and two
sequential design/source gates; no shared SDK or UART recovery change.

This slice adds typed combined operations and independent emulator automatic
activation. MQTT scalar fields, persisted representation selection, partial-xy
intents, colour-control mode configuration, settings/scenes and Store components
remain subsequent work. No new physical certification claim.

## Operation contract

- Preserve `apply_output`, `apply_colour` and both read APIs. Add
  `apply_colour_with_output` accepting already validated colour and base intents.
  Extend the internal Copy write enum with a combined variant and inspection
  helpers. All frames still belong to one output request/HAT job and one original
  deadline; no second operation, permit, queue or frontend cache.
- Colour mutation requires a fresh successful gear-features reply, but accepts
  either value of its automatic-activation bit. Unknown/missing/collided features
  still reject before mutation. Existing type, feature, physical assignment and
  normalized RGBWAF-mode checks remain. Do not change gear features or operating
  mode implicitly. This intentionally supersedes P4d3's temporary manual-only
  mutation restriction; it is not a promise to undo failed DALI writes.
- For a combined request, read base/capability/actual colour and complete all
  preflight validation before either power or colour mutation. Unsupported colour
  must not partially turn off/on an otherwise valid base intent.
- An off intent (explicit OFF or zero DAPC) executes before the complete colour
  plan. Explicit OFF is immediate; zero DAPC retains configured fading and does
  not prove the fixture is already dark when ACTIVATE follows. An on/positive-brightness
  intent executes after the complete colour plan, which includes ACTIVATE, so
  the new colour transition starts before the positive base command. This orders
  commands without waiting for completed colour/level fades. Colour-only
  operations retain ACTIVATE and never send a base intent. Add a pure base-intent
  `is_off()` inspection instead of re-decoding a generated address frame.
- Before the first mutation, clear pre-write base values and colour actual
  values/raw value replies/status; mark them incomplete. Keep capability evidence
  from this same operation. After all writes, read new base level/status and
  actual colour using the existing typed helper. Success requires current base
  readback plus usable values for the requested active colour representation.
  Device clamping is allowed; no exact-value or completed-fade assertion.
- Transmission or readback failure after mutation returns Uncertain through the
  existing output completion/settlement path. No rollback, cleanup command or
  retry is added. DALI operations can be partially applied; with automatic
  activation, staged effects from an interrupted operation can also become
  visible at a subsequent power command. A later base operation reads actual
  colour, rather than promising unchanged colour after an earlier uncertain write.
  The owner has explicitly asked to avoid rare-condition recovery infrastructure;
  ordinary device uncertainty remains visible instead of being hidden by retries.
- Base-only requests retain their existing execution/success behavior. All read
  flags, contexts, renamed-target fencing, final epoch invalidation and original
  caller deadlines remain unchanged. The proxy's 6 s known-colour read budgets
  and original aggregate cap remain; this library API accepts an explicit budget.

## Independent emulator

Add an optional `auto_activation` Boolean to each colour profile, default false
and omitted when false so old scenarios/snapshots keep their current shape.
QUERY GEAR FEATURES reports the selected bit. In the independent physical model,
supported immediate base power commands trigger its existing activation routine
when enabled, including DAPC MASK (which leaves master brightness unchanged).
Cover OFF, DAPC, recall min/max and the already modeled immediate steps. Explicit
ACTIVATE works with either setting and clears temporary state as before.

Timed fades, colour-aware scenes, gear-feature configuration and DT8 reset remain
outside this emulator increment; do not imply the new flag implements those
operations. No production codec/service is imported into emulator semantics.

Primary protocol anchors: Beckhoff's
[gear-features command](https://infosys.beckhoff.com/content/1033/tcplclib_tc2_dali/1012421771.html)
documents activation by lamp-power commands (except enable-DAPC-sequence), and
[ACTIVATE](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/6424507147.html)
documents transfer of temporary values and a colour transition. The pinned
python-dali codec remains the opcode/value reference. Literal expected frames
and physical emulator state provide independent executable checks; physical
firmware timing and actual fading remain unverified.

## Files and proof

Change pure `dali/gear` inspection and tests; `service/output` write plumbing and
its colour preflight/helper; independent `emulator/scenario`, `colour`, `model`
and literal tests. Reuse the current private PTY/FakeBroker fixtures and literal
peer. Do not change config/inventory/identity or MQTT property contracts here.

Fail-first evidence: automatic-activation profile reports bit 1 and applies staged
colour on a base command; current service rejection reproduced before allowing
known enabled mode; combined ordering tested through independent literal UART
frames for off/zero/on/positive level, then through physical emulator state for
Tc/xy/RGBWAF and independent master level. Unsupported/unknown preflight sends
neither base nor colour mutations. Delayed/cancelled combined operations retain
the same admission, original trace/token and native input handling. Incomplete
post-write colour cannot confirm a complete base write as a combined success.
An interrupted staged operation is explicitly uncertain, and emulator evidence
shows possible later automatic activation without any retry/rollback machinery.

Retire only the old manual-only policy control, replacing it with unknown-feature
rejection and enabled-mode acceptance controls. Retain representation, assignment,
mode, requested-kind, ownership, epoch, native event and paced-budget controls.
Run meaningful affected controls, restored full native suite, strict scoped
Clippy, Pi library/all binaries compile, changed-file format/whitespace/wait and
doctests. Update colour/emulator guides, support matrix, contributor catalog and
progress, recording exact review/test limits. No shared skill change is needed
because this adds DALI-specific behavior only.

## Self-review

Checked the existing service read-before-colour-preflight path, base-write ordering,
single HAT helper/settlement, native event handling, final acceptance and proxy
budgets. Whole-plan execution and final real readback remain the one mechanism.
Combined validation precedes OFF as well as ON; temporary values cannot be
mistaken for post-write actuals. Failed writes are never claimed atomic or undone.
No persistent gear-setting workaround or new failure-recovery framework is used.

Design gate 1 approved with a wording correction now incorporated: OFF and zero
DAPC have different fade semantics; command ordering does not promise darkness
or a completed colour transition. No frames or recovery mechanism were added.
Review was read-only/same-family and performed no runtime/hardware checks.

Design gate 2 approved the amended contract with no material blocker, confirming
preflight-before-mutation, cleared pre-write evidence, ordered commands, actual
readback and explicit partial/later-effect uncertainty. The pending public MQTT
representation preference is independent of this typed service increment.
This review was also read-only/same-family with no builds/tests/mutations.
