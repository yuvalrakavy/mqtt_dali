# mqtt_dali v2: design and implementation plan

Date: 2026-10-21

Status: architecture, web interface and implementation authorized by the owner. Work is underway on `feat/dali-v2` in `Home/mqtt_dali-v2`; see `docs/v2-progress.md` for measured progress. P0 has a separately reviewed contract and a tested library/check command; daemon integration belongs to P3. Detailed UART, runtime and Store migration contracts remain phase-specific verification items.

## 1. Objective and boundaries

Build an open-source wired-DALI bridge with BridgeKit MQTT, tracing-init/Logmon integration, an optional authenticated configuration website, common DALI-2 input support, color control, and a UART-compatible ATX HAT emulator. The bridge requires a broker, not a Store server. Store is one consumer.

Retain the legacy implementation on v1 for urgent fixes. v2 uses BridgeKit's MQTT interface exclusively; do not add simultaneous v1/v2 MQTT operation to one process. Keep v1 configuration usable by the v1 binary.

The owner's web design approval covers the mobile and desktop workflows, inline commissioning, groups, scenes, sign-in, and navigation. The selected implementation stack is Axum/Tokio plus Preact/TypeScript/Vite, responsive CSS, JSON HTTP operations, and SSE updates. Production browser assets are embedded in the Rust executable. Node is a build/development dependency only.

PanelScript touch-panel design and implementation are outside this effort. Shared HA components, equipment classes, device objects, bindings, and preset participation are in scope. This is not a claim of DALI certification or compatibility with every manufacturer-specific extension.

## 2. Repositories and present baseline

- Bridge: `Home/mqtt_dali`, master `db6f39f63da75a0614eb8f1a3b274a163bc4b307` when inspected.
- Legacy remote branch: `origin/v1` at that same commit. Annotated release tag `v0.1.2` exists.
- Shared runtime: `tracing-init/mqtt-bridge-kit`.
- Consumer integration: `Store/data/frameworks/ha_drivers/modules/dali.sdl`, HA component/equipment definitions, and preset capability slices.

| Existing fact | Evidence | Reached from |
|---|---|---|
| Current config is unversioned JSON; channel/group `description` supplies names | `src/config_payload.rs`, `src/setup.rs: Config::load` | `main::start_up` loads config before constructing the controller |
| UART reply parsing currently combines packet type with an expected bus | `src/hat_line.rs: parse_reply` | `DaliAtx::receive_reply` -> `get_line` -> parser |
| Commissioning already distinguishes All / WithoutShortAddress / Address | `src/dali_manager.rs: DaliBusIterator::new` | device discovery/address assignment iterator |
| Group mask queries and add/remove with verification exist | `src/dali_manager.rs: query_group_membership`, `add_to_group_and_verify`, `remove_from_group_and_verify`, `match_group` | existing setup/MQTT group operations; reuse behind the shared service |
| An in-process emulator exists, but bypasses the physical UART path | `src/dali_emulator.rs`, `src/main.rs: start_up` | `--emulation` selects a `DaliController` implementation |
| BridgeKit targets must fit one topic segment; Desired values must be scalar | `mqtt-bridge-kit/src/topics.rs`, `wire.rs`, `driver.rs` | `Topics`, Desired/Command decoders, DeviceDriver input |
| BridgeKit reports can clear retired retained State targets | `mqtt-bridge-kit/src/reports.rs: Reporter::remove` | driver reporting -> runtime publication; consumers still need explicit inventory semantics |
| Store currently builds DALI layouts using names from config | `ha_drivers/modules/dali.sdl: update_config`, `Dali` layout builders | `DALI/Config/#` -> `update_config` -> `reconcile` |
| Store binds shared component PropertyIds and propagates triggers without storing them | `docs/guides/binding-model.md`, `store_server/src/command/set_property.rs` | source writes -> trigger branch -> `set_bound_properties` |

Source inspection establishes these implementation anchors, not hardware conformance. The new transport must replace the request-local receive model instead of adding a second UART reader.

## 3. Architecture and ownership

```text
Browser -> authenticated HTTP/SSE -> shared DALI service
MQTT    -> BridgeKit DeviceDriver -> shared DALI service
                                      |
                               single UART owner
                                      |
                              ATX I4 -> DALI buses
                                      |
                         continuous event reception
```

The DALI service owns configuration, device inventory, jobs, readbacks, event decoding, bus operations, and the command scheduler. HTTP and MQTT use the same validation and operations. Neither frontend owns the UART or implements DALI logic.

Use one fixed BridgeKit instance per HAT/controller. Nested physical devices are qualified targets. Keep web/process services available during broker or HAT outages, while hardware operations report their actual availability.

Add the previously approved, small managed process-service extension to BridgeKit before integrating HTTP. It must provide supervised startup, cancellation and bounded shutdown without duplicating logging, signals, runtime construction or watchdog behavior. Verify its fit against the current public API before editing shared code; `.start()` alone does not provide the complete `.run()` process policy.

Keep tracing-init as the single logging initialization path, with GELF/OTLP destinations and MQTT v5 traceparent propagation. Never log web passwords, cookies, session tokens, or exported credentials.

## 4. Configuration and v1 migration

Default inventory files: v1 `dali.json`; v2 `dali.v2.json`. Explicit paths must have equally unambiguous v1/v2 selection. Startup connection/security settings remain config/CLI inputs; web credential values are not part of inventory exports.

| Startup condition | Required result |
|---|---|
| v2 exists | Load v2, ignoring v1 |
| v2 absent and v1 exists | Parse and validate v1; migrate; atomically create v2; use v2 |
| Both absent | New installation with an empty inventory and validated startup settings |
| Existing v2 invalid/unreadable | Report the error; do not silently fall back or remigrate |
| Migration fails | Leave v1 unchanged; do not publish a partial v2 file |

The v2 process never writes the v1 file. The v1 binary remains able to use it. Deleting v2 intentionally forces a new import from the current v1 configuration. Later v1 changes are otherwise not merged into v2.

Use an explicit schema version and persistent configuration revision in v2. Migration preserves controller/bus/output/group names, bus indices, short addresses, group addresses and membership. Legacy `description` becomes `name`; do not copy it into the new free-text description field as if it had been a separate description.

Validate duplicate addresses/names, address ranges, membership references, encoding and unsupported schema versions. Report conflicts rather than silently normalizing or dropping items. Do not infer enabled/disabled settings from a cached power/fault status. Preserve evidence of missing fixtures and reserve their addresses. New type/capability fields start unknown and are queried without mutating hardware.

Migration does not rescan, readdress, rewrite groups, clear scenes, or publish legacy desired commands. Atomic creation must not overwrite a v2 file another process has just created. Configuration writes retain bounded filesystem waits, same-directory temporary files, and crash-safe replacement. A valid existing v2 file always wins, including after an interrupted prior attempt.

## 5. Identity, hierarchy and synchronization

Public identity remains the name; no public UUID scheme is introduced. Physical addresses are separate properties. Description is optional free text; no MQTT DisplayName is required.

```text
/Devices/DALI/HouseController
  Bus1
    Outputs/KitchenCeiling
    Inputs/WallPanel/Instances/EntranceKey
    Groups/Kitchen
    Scenes/Evening
  Bus2
```

There is no redundant Buses singleton. Outputs, Inputs, Groups, Scenes and Instances represent the relevant address/slot spaces. Different output types share Outputs; do not duplicate a multifunction physical endpoint under capability-specific containers.

The controller inventory is authoritative. A rename in the bridge renames the existing Store device object in place, preserving ObjectId and bindings. A normal rename can be matched within controller/bus/address-space/address/instance; address reuse and full reassignment require explicit identity transitions, not blind matching by address. Ancestor renames also migrate descendant target identities.

A complete, revisioned retained inventory must allow a consumer to recover after being offline. Rename events alone are insufficient. Explicit removal must be distinguishable from missing/offline hardware and from a partial snapshot. Publish a snapshot atomically at the document level, retain durable rename/reassignment/removal information needed for reconciliation, and prevent older revisions from resurrecting objects.

Proposed wire detail for review in phase P3: a reserved metadata target `__inventory` in the State lane, intercepted by the DALI consumer adapter rather than materialized as a device. Its snapshot contains hierarchy, capability metadata, revision and identity transitions. Pin its schema and retirement rules in contract tests before implementation relies on it.

Use BridgeKit `Reporter::remove` for retired State targets. The requesting consumer owns cleanup of its retained Desired values; the bridge must not impersonate that publisher. Define stale-request rejection and consumer topic migration together, so a retained command cannot affect a different fixture after address reuse or a name change. Generic consumers must be able to implement this without Store-specific messages.

Removed fixtures leave the saved inventory only after explicit removal. Missing fixtures remain present and unavailable. Removing a device does not delete logical Home equipment; Store must expose the resulting unresolved binding. New Store topology and class migrations preserve existing output/group ObjectIds and values in place.

## 6. MQTT representation

Keep the existing BridgeKit lane grammar and single-segment target contract. Examples:

```text
DALI/State/HouseController/Bus1.Outputs.KitchenCeiling
DALI/Desired/HouseController/Bus1.Outputs.KitchenCeiling/Brightness
DALI/Command/HouseController
```

A qualified target encodes named path components with dots between them; literal dot and percent characters in components require an unambiguous, reversible encoding. Freeze the codec with collision and Unicode vectors before using it in either bridge or Store. Inputs include module and instance, for example `Bus1.Inputs.WallPanel.Instances.EntranceKey`.

Use scalar Desired properties and non-retained momentary Command messages. Color is represented by explicit scalar components, not an arbitrary JSON object smuggled into Desired. Preserve BridgeKit expiry, conflict, response/correlation and reconnect behavior. Commissioning jobs return a job identifier promptly; progress/results are queryable and event-driven. Ordinary successful transmission does not prove a fixture reached the requested state.

## 7. UART transport and command/event separation

Exactly one component reads serial bytes for the lifetime of the link. It parses all lines continuously, including idle time, and dispatches unsolicited traffic independently of pending requests. Preserve bus identifiers and frame widths. Do not clear the input buffer to obtain a response.

A pending transaction accepts only its documented response classes on the appropriate bus and within its response window. A monitored L frame is not a generic command acknowledgement. Version, boot, power changes, forward frames, backward frames and collisions remain distinct typed events. Reboots invalidate pending transactions and require state/capability refresh.

The ATX manual documents monitored H/J/L packets and N/collision responses, not a universal transaction identifier. Exact echo/response ordering and ambiguous late replies require golden manual examples and hardware traces. Do not claim that a prefix-only parser solves correlation. Ambiguous transactions fail or resynchronize before another response can be misattributed.

Serialize UART writes. Protect multi-frame DTR/DT8/commissioning sequences per bus and account for the HAT's autonomous forwarding and external bus traffic. Bound bytes, lines, waits, retries, queues, job duration, cancellation and shutdown. A failed fixture is target unavailability; it does not automatically mean the entire HAT link is lost. Bus faults and HAT faults have separate scopes.

## 8. Capability surface

| Family | Planned behavior | Phase |
|---|---|---|
| Standard outputs | Power, dimming, levels/limits, fades, power-on/system-failure settings, status and faults | P4 |
| DT7 switching | Relay-specific power behavior and capability reporting | P4 |
| DT8 color | Tc, xy, RGBWAF discovery/control, supported limits, color-aware scenes | P4 |
| DALI-2 inputs | Part 301 buttons, 302 absolute inputs, 303 occupancy, 304 light sensors; instance discovery and configuration | P4 |
| Input feedback | Supported indication/feedback capabilities exposed with truthful device metadata | P4 |
| Identity/memory | Supported standard identity and memory-bank reads, capability inventory | P4 |
| DALI Data 251–253 | Supported luminaire, energy and diagnostic data, with units and unavailable/unsupported status | P4 |
| DT1 emergency | Supported status/configuration and explicit test operations with progress/results | P4 |
| Commissioning | Refresh/add/reassign/restore/remove workflows | P5 |
| Groups/scenes | Native configuration, recall, verification, repair, copy/clear | P5 |

Manufacturer extensions, firmware update, transports other than this wired HAT, and certification work need separately documented scope. Publish an explicit support matrix; distinguish emulator coverage, hardware validation and unimplemented features.

Brightness uses normalized HA values and a documented conversion to DALI levels 0–254. DALI MASK is not 100% brightness. Preserve raw values and units where useful for diagnostics. Pace readbacks and aggregate group state as uniform/mixed/unknown from fixture observations instead of expecting a collision-free group reply.

## 9. HA components, color and presets

Representation-specific shared components carry matching PropertyIds on device and Home equipment: ColorRGB for RGB, separate components for RGBW/RGBWAF, white temperature and xy. Use Percent 0–1 for normalized channels. Equipment combines PowerState/Dimmer and the applicable representation.

Support both:

1. DT8 color at one address: one binding for the complete component.
2. Independent RGB/RGBW outputs or groups: three/four bindings on the logical equipment's channel children.

The latter composition belongs to Home equipment; do not synthesize a non-existent RGB fixture in the physical inventory. Preserve color independently of power/master brightness, avoid double scaling and readback feedback loops. BZ.xdo's LightCubes example uses per-color DALI groups and confirms the group-binding requirement.

Native input devices expose decoded state/events through shared input/button components. Do not attach a second gesture recognizer to already classified native press/hold/double events. State starts unknown when no fresh observation exists.

DALI scenes remain opaque to Store. Keep `DaliScene` (BindTarget), `HardwareScene` (BindSource), and a shared `SceneRecall` trigger component. Store mirrors only scene identity, slot, description and appropriate status; no per-fixture scene-setting children. The bridge web UI owns editing, capture and verification of scene contents.

The HA preset engine remains the generic composition mechanism. A preset can mix ordinary fixtures across buses/protocols and optionally HardwareScene participants. Register a recall capability: recording saves participation/reference, not a trigger value or new hardware scene contents; activation recalls once. Deactivation has no automatic hardware-scene inverse. Define ordering/conflicts if one preset also controls fixtures affected by the recalled scene. Panel editors are out of scope.

## 10. Commissioning workflow

One bus page lists discovered defaults (Light1, Input1, etc.), types and current state. Add unaddressed preserves existing assignments and reserved missing addresses. Refresh queries existing addresses. Full reassign is a separate explicit bus action, scoped to outputs/inputs/both and confirmed before changes.

Blink/Stop identifies outputs; successful changed rename stops blinking and restores the prior output. Rename commits on checkmark, Enter or focus leaving; deduplicate checkmark plus blur. Validation/save failure keeps the draft, error and identification context. Use name glob filters such as Light* and Input*, case-insensitive with * and ?. Filtering never limits the bus scan and must not hide an invalid draft or reorder an edited row.

Input events highlight the matching instance without disrupting editing; show current state and recent action. Output rows provide compact power, brightness and applicable color controls.

Lost-address recovery starts from a saved missing fixture. Find unaddressed candidates, identify them with an unused temporary test address if needed, and restore the old reserved address after selection. Verify before reporting success and keep partial changes visible on failure/cancellation. No stable identity may be inferred solely from receiving an old address.

Explicit Remove from list removes saved inventory, not the fixture's hardware address. A fixture still on the bus can appear again during discovery. Keep logical Home equipment and surface its lost binding.

## 11. Groups and scenes

Output groups expose slots 0–15. Edit membership from the Groups tab or an inline fixture Groups checklist. Pattern assignment is a previewed one-time operation: Add preserves current members; Replace exactly matches the selected fixtures within that group and leaves other groups alone. Renaming later does not re-run the pattern.

Verify reads all fixture group masks, including unnamed slots, and compares with saved assignments. Review additions/removals before repair; re-read afterwards. Missing replies remain unresolved. On first adoption, review existing hardware assignments instead of assuming all empty slots should be cleared. Hardware-to-config adoption is explicit and distinct from repair.

Scenes expose slots 0–15 per fixture, aggregated by bus. Recall can target the bus or a group; group scope does not create extra hardware slots. Brightness and supported color are stored per fixture. Off and Leave unchanged are distinct; DT8 can keep one attribute while setting another. A whole-row ignore must mask all relevant attributes.

Editing stored scene values does not activate them. Capture reads selected current fixture settings, reviews overwrite differences, then stores/verifies. Preserve unselected entries. Copy/clear require destination/scope review. Verify compares saved brightness, color and ignored entries; offline fixtures remain unresolved. Stored-configuration verification is not proof that a scene is currently active.

## 12. Optional web interface and security

Web is disabled by default. Enabling requires a nonempty password supplied through config/CLI; no shared default password. Reject missing/empty credentials before opening a listener. Controller name and version/build are deliberately visible on the sign-in screen.

Use random server-side sessions and HttpOnly cookies, with appropriate SameSite/Secure attributes for the deployment, login rate limiting, CSRF/origin protection for state-changing operations, no tokens in URLs, and no browser-local-storage credential persistence. Support HTTPS deployment directly or through a documented reverse proxy. Configure trusted proxy handling explicitly.

Every protected route, API and SSE connection requires authentication. Unauthenticated page navigation goes to sign-in; APIs return 401 with no inventory. Permit only the minimal sign-in/bootstrap assets and selected controller/version identity before authentication. Arbitrary internal return URLs are not accepted. Existing SSE connections must stop exposing data after session expiry/revocation, not merely authenticate once at connection creation.

Proposed session inactivity default: 30 minutes, with an absolute lifetime and bounded session count defined during security implementation. Background SSE traffic alone must not keep an abandoned browser session alive. Logout, password change through startup config/restart, and bridge restart invalidate the applicable sessions. Job completion after session expiry does not authorize new requests or replay a command after login.

The website operates independently of MQTT availability. Missing HAT access shows degraded status and saved inventory; controls requiring hardware explain their unavailable state. The first installation must not fall into the old interactive terminal setup path when web is disabled.

## 13. Approved navigation and presentation

```text
Sign in (controller name + version/build)
  Controller overview
    Bus page
      Devices (all identification/configuration inline)
      Groups (membership, pattern assignment, verification)
      Scenes (edit/capture/recall/verify/copy/clear)
      Bus actions (full reassign, bus settings)
  Settings (startup summary, backup/import, web-access information)
  Diagnostics (connections, faults, jobs, activity, bus traffic)
  Sign out
```

Mobile uses compact rows and sibling tabs. Desktop adds persistent bus navigation, wider device tables, and group/scene slot lists beside fixture settings. The owner explicitly approved the website after reviewing both formats. Preserve system fonts, responsive CSS, inline editing, accessible labels/focus handling and stable live updates.

Current visual references are the session's `dali-navigation-design.html`, `dali-bus-commissioning.html`, `dali-groups-design.html`, `dali-scenes-design.html`, and `dali-desktop-design.html`, with rendered PNGs in the Codex visualization directory. These are mockups with sample data, not production code or a tested authentication implementation. Package selected durable screenshots with the public documentation in P9.

## 14. Emulator and verification infrastructure

Build a standalone ATX HAT emulator exposing the same UART ASCII interface over PTY on macOS/Linux. Production bridge transport connects to that PTY; no test-only shortcut around the UART parser. Refactor useful existing device simulation into a shared model only where it does not make the emulator and driver share their protocol mistakes.

The emulator has independent fixtures/buses/addresses/groups/scenes, DT8 state, input instances, capability data, bus/HAT faults and configurable delays/collisions/truncation/reboots. Provide deterministic randomization/time controls and private test controls. Its interactive controls can inject switch/sensor activity and faults. Apply the same optional authenticated web-access policy if its controls are exposed over HTTP.

Keep golden protocol transcripts independent of bridge serialization. Check transport boundaries with interleaved events/replies on different buses, events without commands, back-to-back input activity, late replies, echo, collisions, reboot, malformed data, cancellation and broker outages. Emulator-only success does not constitute physical switch/color/emergency hardware validation.

## 15. Cross-subsystem invariants

| Datum | Bridge / UART | MQTT | Store / UI |
|---|---|---|---|
| Name | Canonical saved identity | Encoded qualified target + rename transition | Device renamed in place; logical Home name independent |
| Physical address | Separate scoped location | Metadata, never sole lifetime identity | Binding survives verified reassignment; address reuse is explicit |
| Missing endpoint | Keep reservation/inventory | Unavailable state | Keep object and expose unavailable state |
| Removed endpoint | Explicit inventory change | Tombstone/complete snapshot + State retirement | Remove device only; preserve logical equipment |
| Scene | Settings owned by bridge/fixtures | Opaque scene target and recall command | Metadata and shared recall component only |
| Configuration write | Persisted revision, bounded atomic save | Canonical inventory revision | No optimistic claim of hardware verification |
| Input event | Continuous receive and decoded instance | Non-retained event plus fresh state | One event, stable row, no synthetic duplicate gesture |
| Web session | Server-authorized operation | No change to broker authentication | No route/API/SSE bypass or replay on re-login |

## 16. Implementation phases and acceptance ledger

Every phase starts by extracting its mandated surfaces/tests and ends with a scoped review. A phase is not complete because a different route demonstrates similar capability. Proposed new files may be split as implementation warrants; this table names owners and responsibilities, not verbatim code.

| Phase | Main files / repository | Demonstrable completion and required verification |
|---|---|---|
| P0: release boundary and config | bridge `Cargo.toml`, new `src/config.rs`, development `src/bin/mqtt_dali_v2.rs`, migration fixtures/tests; daemon startup integration in P3 | v2 wins when present; v1-only auto-migrates; deleting v2 reimports; malformed/failed/concurrent/interrupted cases preserve v1 and never silently replace v2; no hardware writes |
| P1: transport foundation | bridge `src/dali_atx.rs`, `src/hat_line.rs`, new serial transport/dispatcher modules; existing parser tests | One UART reader; typed frames; documented response matching; late-reply resync; bounded waits; idle events delivered; Pi UART and desktop PTY use the same parser |
| P2: ATX emulator | new emulator binary/library and PTY harness; refactor `src/dali_emulator.rs` as appropriate | Production bridge connects via PTY; independent golden vectors; injected input events and faults; deterministic addressing/group/scene/device data |
| P3: BridgeKit and shared service | tracing-init process service hook + tests; bridge driver/service/jobs modules replacing custom runtime in `src/mqtt.rs`; contract tests | Existing BridgeKit conformance passes; HTTP/process service survives MQTT outage; inventory/codec/rename-retirement schema frozen and exercised by an independent consumer; logging/shutdown remain bounded |
| P4: capability engine | bridge DALI output, DT8, input, memory/data/emergency modules; emulator models | Each family in section 8 has explicit supported/unsupported behavior, meaningful emulator cases and published hardware-validation status; scalar color API and correct MASK handling |
| P5: commissioning/groups/scenes | shared service/config operations, scheduler and jobs | Approved add/refresh/reassign/restore/remove, pattern membership, verify/fix, native scene capture/edit/recall/copy/clear through the same API; partial-failure and cancellation results verified |
| P6: optional authenticated HTTP | bridge web server/session/API/SSE modules and build integration | Disabled means no listener; missing password rejected; authenticated routes; direct URL/API/SSE bypass tests, CSRF, expiry/logout/restart tests, broker/HAT outage usability |
| P7: approved browser UI | new `web/` Preact/TypeScript/Vite/CSS and embedded assets | Mobile and desktop workflows operate against the PTY emulator; inline rename/blur/checkmark/filter behavior; menus open unclipped; persistence after reload; live input identity; no fake-success controls; Playwright functional pass |
| P8: Store integration and migration | `ha_drivers/modules/dali.sdl` split if needed; HA component/equipment files; preset capability/slices; explicit persisted-data migration and fixtures | Revisioned inventory sync with in-place rename/move; existing ObjectIds/BoundObject references survive; DT8 one-binding and split RGB/group bindings work; opaque DaliScene/HardwareScene recall participates in generic mixed-protocol presets |
| P9: documentation, skills and release validation | bridge guides/README/CLAUDE, tracing-init API/tutorial docs for shared hook, Store MQTT/binding guides and applicable skill sources/catalog | Reproducible installation, v1/v2 rollback, startup/web security, supported-device matrix, commissioning workflows, emulator usage, contract details and limitations; pin dependencies, cross-build Pi and run scoped gates; publish release only after approval |

P0–P7 are substantial bridge/application work. P8 touches persisted Store data and uses Store's T3 migration procedure, isolated dev-track lifecycle and migration safety checks; do not implement it as an incidental SDL rename. No kernel/boot change is presumed necessary; if one becomes necessary, revise that phase explicitly.

Build/test order follows dependencies: P0 -> P1 -> P2 -> P3 -> P4/P5 -> P6 -> P7 -> P8 -> P9. Implement P4 by capability slices so each has a usable, tested vertical path. Do not call the full capability surface complete while specialist rows are only declared in metadata.

## 17. Verification and review requirements

Retain applicable v1 no-hang regression coverage; update wait-registry rows and negative controls for new waits. Run native Mac Rust tests and process/broker tests, BridgeKit conformance after shared changes, Linux/Pi compile checks and actual Pi hardware checks where hardware exists. Sample idle CPU and use event-driven waits rather than polling loops.

Migration cases cover missing/both/invalid files, groups omitted in v1, names requiring encoding, duplicate addresses/references, unsupported versions, permission and interrupted-write failures, deterministic retries and no v1 modification. Store migration tests assert exact ObjectIds, binding references, migrated values and no recreation or live-home/template drift.

Browser acceptance runs at phone and desktop viewports against the real HTTP service and UART emulator. Exercise auth at actual endpoints, including direct protected URLs, live-stream revocation, session timeout without SSE extension, and no accidental resubmission after re-login. Inspect actual resulting config/hardware model, not just a success message.

Mixed-preset acceptance includes outputs on different DALI buses, a DMX light, a Lutron light and an opaque hardware scene. Verify recall ordering/conflicts, no trigger replay on binding/restart/reconnect and no scene rewrite when recording a preset.

Use `developing-mqtt-bridges` for runtime/contract work; Store `ways-of-working`, `using-dev-tracks`, `building-sdl-modules`, `boot-schema-change` for persisted Store migration, `using-store-test` for DSL acceptance, and documentation/catalog skills for their respective guide/catalog changes. `testing-admin-ui` applies only if Store `ui/` changes are actually introduced; Preact bridge UI uses its own browser tests. PanelScript skills remain outside scope.

Use sequential fresh-context review agents for the design/deep gates required by `ways-of-working`. Its gate-agent authorization applies; implementation delegation remains excluded unless explicitly requested. Two separate reviewers checked the foundation design and P0 contract before implementation; a third reviewed the P0 code. All exposed model options are from one model family, so cross-family independence is unavailable. Reviews of later phase contracts remain outstanding.

## 18. Open verification items and next decisions

1. Freeze UART response matching and hardware firmware expectations from the manufacturer manual plus real transcripts. Handle metadata discrepancies explicitly; no prefix-only ACK assumptions.
2. Freeze complete inventory, identity-transition retention, request retirement and canonical target encoding with Store and independent-consumer tests in P3. This is an implementation-design review item, not a change to the approved hierarchy or MQTT lane grammar.
3. Specify imported existing Store topology migration and scope-aware rename/move before P8, with T3 review. No live Store writes are authorized by this design document.
4. Define hardware-scene versus direct-fixture precedence within one HA preset before adding the recall capability. The website and opaque scene representation do not decide this policy implicitly.
5. Physical DALI-2 input, DT8 and emergency/data validation depends on hardware availability. Expose support/test status honestly; emulator fixtures provide initial coverage.
6. Select exact dependency versions and serial implementation at implementation start from current supported releases; keep version/build reporting tied to the actual executable.

## 19. Self-review record

A consolidation pass checked this record against the owner's later corrections: separate v2 file with automatic v1-only migration; explicit password and optional website; approved Preact stack; opaque Store scenes; generic HA presets; separate RGB bindings including groups; controller inventory authority; focus-leaving as save; and desktop/mobile web approval. These decisions supersede earlier mockup text or proposals where they differ.

This records the approved design and phase boundaries. The progress ledger identifies reviews and tests actually performed; neither those checks nor emulator coverage imply physical hardware validation or release readiness. The open items in section 18 must be closed by their owning phase before the relevant code depends on them.

## Sources

- ATX I2/I4 HAT manual: https://atxled.com/pdf/AL-DALI-HAT-I.pdf (serial/monitoring details, pages 6–9; confirm deployed firmware).
- DALI Alliance quick-start guide: https://www.dali-alliance.org/data/downloadables/3/4/8/dali-quick-start-guide_public-v1_1_april-2023.pdf (address spaces, groups and scene semantics).
- BridgeKit current source and README, and Store `docs/guides/mqtt-bridge-contract.md`, `docs/guides/binding-model.md`.
- OWASP session management: https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html.
