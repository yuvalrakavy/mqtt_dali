# Retain numeric edits in input settings

Tier T1: correct an existing form interaction; no new wire or persistence contract.
The owner has authorized continued implementation of the approved bridge/UI.

## Confirmed problem and choice

The actual input-settings browser flow entered1000ms before changing Event
priority, then saved. The independent model retained the old500ms (raw25 instead
of50); all other requested values matched. The same test passes when clicking a
checkbox commits the number first. Evidence:
`/tmp/dali-input-edit-early-red.log` (behavioral failure),
`/tmp/dali-input-edit-red.log` (blur-first pass). Another attempted probe stopped
earlier on an existing scene Busy, so it is not evidence about the input form.

`input-settings-ui.tsx` updates number fields only on change and merges a captured
draft. A different control can rerender before that number commits. Sensor hold
also has invalid intermediate values (2 while typing20), which must remain
editable without becoming a valid native setting. The fixture-settings form
already demonstrates typed text on input and functional draft updates.

Options: require explicit blur before other edits; normalize every keystroke to
the native display; retain text immediately and separately parse the native
draft. Choose the last: it preserves ordinary editing and existing save-time
validation. No new workflow, framework or dependency is needed.

## Scoped implementation and checks

1. Keep typed timer/sensor strings until refresh. On input, update those strings
   and the native draft through functional setters. Merge nested timers, sensors,
   filters and groups from current state. Capture DOM values before deferred
   setters; do not retain event/currentTarget references.
2. Keep original scope, request parser, native units, validation, save/readback,
   enabled/unsupported behavior and Cancel/navigation semantics. Invalid numeric
   text remains invalid and cannot submit a mixed hardware patch.
3. Strengthen the existing real-browser flow: enter numbers before dropdowns,
   retain intermediate sensor hold text, then save/reopen/restore all timer and
   sensor families. Preserve unchanged native/inventory assertions. Run the
   input flow earlier so unrelated scene operations cannot hide this regression.
4. Build and byte-check embedded assets, run all Node tests and the complete
   phone/desktop browser flow; inspect fresh screenshots. Add one measured
   browser fault reverting input handlers to blur-only change, restoring source
   and assets afterward. Freeze and obtain fresh source-only review. Update
   browser guide/progress with actual evidence and limitations.

Self-review checked every numeric family and nested edit site in this form.
Partial timing/sensor availability and unsupported hold retain their existing
guards. No backend/SDK/Store/live hardware changes, new tests mirroring a state
helper, automatic retry, or unrelated UI refactor. Real browser evidence is the
regression oracle; native compilation is asset integration only.
