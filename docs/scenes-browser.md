# Configuring native scenes in the bridge

For the person configuring a DALI installation through the bridge's protected
web interface. Open your controller, select a bus, then **Scenes**. The title
shows `controller/bus`. On a phone, use the scene selector; on a desktop, use the
sixteen-slot list. Slots are numbered 0–15 and belong to this bus.

Browsing a slot changes nothing. Give an unnamed slot a name with **Create
scene** before saving settings. Edit an existing name inline: the checkmark,
Enter or moving focus saves it. The actions button beside the name edits its
description or removes its saved name and settings. Removing the name preserves
the scene already stored in the fixtures.

## Edit fixtures on one screen

Each fixture shows its name, address, saved/unmanaged status and **Blink**. Stop
identification before writing settings or recalling a scene. Filter names with
patterns such as `Kitchen*`; `*` matches any sequence, `?` one character, without
case sensitivity. A dirty or failed row stays visible even outside the filter.
The filter selects fixtures for the reviewed operations below.

Choose the fixture's action:

| Action | What a later recall does |
|---|---|
| Off | Switch this fixture off |
| Brightness | Set the entered brightness percentage |
| Keep brightness | Preserve brightness, while optionally changing colour |
| Leave unchanged | Preserve both brightness and colour |

**Unmanaged** means the bridge has no saved expectation for that fixture/slot.
An explicitly saved **Leave unchanged** row is a different state: the bridge
expects that fixture to ignore this scene, and can verify or repair that setting.

Colour is independent of brightness: keep colour, white temperature in Kelvin,
CIE x/y, or RGBWAF channel percentages. Blank colour attributes mean **MASK**:
keep that attribute on recall. Selecting a colour representation with every
attribute blank still selects that representation. RGBWAF supports normalized
control with confirmed channel assignments; unsupported modes remain inspectable
under **Native values**. Read capabilities there to refresh observed support.
The fixture confirms support before the bridge saves a participating setting.

Save a complete fixture row with its checkmark, Enter or by moving focus outside
that row. Moving between its colour fields does not save an incomplete entry.
Untouched native values survive display conversion exactly. Invalid or failed
drafts remain available to correct, explicitly retry, or discard. If another
configuration change invalidates the draft, discard and review the current row.

Saving programs native scene memory and reads it back; it leaves current light
output unchanged. A confirmed save followed by an incomplete hardware operation
is shown as **Saved · unconfirmed**. Inspect and explicitly repair it; no batch
automatically retries or continues after that result. Saved settings remain
readable with a disabled bus or unavailable HAT; physical operations require it
to be ready.

## Review several fixtures

Every preview freezes fixture identities, selected settings and the configuration
revision. Check the listed before/after values before **Apply reviewed settings**.
Unselected fixtures and other slots are preserved. Stop after the current step
using the progress control; already completed saves remain saved. Changing slot,
leaving the page, signing out, reconnecting, or an external configuration edit
prevents later submissions from an old batch. Review again before continuing.

- **Capture current** reads the matching fixtures' current output and shows the
  proposed saved settings. Unresolved candidates are excluded. Apply uses these
  frozen candidates; it does not sample the lights again after your review.
- **Copy saved** copies managed settings from the selected named source slot
  into the current named destination. It preserves unmanaged source rows rather
  than inventing ignored settings for them.
- **Clear selected** saves Leave unchanged for the matching fixtures and removes
  their native participation in this slot. It does not remove their names or
  change the lights.
- **Verify scene** reads every configured fixture for this slot. **Verify all
  16** includes unnamed slots. Results distinguish matching, different, unmanaged
  and unresolved data; a raw reply is not automatically a writable setting.
  **Review adoption** saves representable native settings. **Review repair**
  programs saved expectations. These actions select the current slot and current
  name filter, then require a before/after review. The bridge checks the reviewed
  native evidence again before saving or repairing it. Completed reads remain
  visible when verification stops; stale results require fresh verification.

## Recall and automation

Choose **Entire bus** or a numeric group beside **Recall**. Recall changes the
lights and reports that the command was sent. It does not claim an active scene;
read individual fixtures to obtain current output. Fixtures with ignored
participation preserve their current output.

The bridge keeps fixture settings privately. Public Inventory and Store scene
objects contain metadata and recall bindings; a higher-level preset may combine
this native DALI scene with other buses or lighting systems. PanelScript touch
panel design is separate from this configuration interface.

Contributors: see [scene protocol](dali-scenes-contract.md),
[browser development](browser-development.md), and
[fault controls](v2-scenes-ui-controls.toml). Local emulator evidence is not
physical fixture certification.
