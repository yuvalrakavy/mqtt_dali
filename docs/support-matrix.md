# v2 capability status

This is development status during P4/P5, not a release or certification statement.
The canonical v2 executable runs BridgeKit with metadata and UART health. No v2 physical HAT
or fixture behavior has been validated. The v1 maintenance binary is separate.

| Capability | Local implementation/evidence | Remaining work |
|---|---|---|
| Output identification | Managed Blink/Stop, fresh prior-level capture, actual restoration, continuing rename and original physical-epoch guards; PTY/MQTT/HTTP, paused-time and real phone/desktop browser tests | Physical firmware/fixture timing |
| v1 configuration import | Library and canonical process tests; v1 bytes preserved; v2 precedence and reimport | Persisted Store-object migration in P8 |
| ATX I4 UART | Typed frames, continuous monitoring, measured quiet recovery, bounded request/helper progress under input traffic, managed health and real PTY/process crash-restart tests; known partial-write poison survives in-process reopen | Physical timing/firmware transcripts; j/t completion policies; no persistent inference of partial writes across process death |
| Optional HTTP | Password-required startup, cookie sessions, Origin/CSRF, protected inventory/status/actual scalar snapshots, fixed expiry/revocation; bounded configuration/commissioning and scalar read/apply jobs; fair native-input/job/snapshot SSE with loss/cutover checks; embedded Preact screens and private socket/process/emulator/browser tests | Remaining capability jobs; HTTPS proxy deployment not physically exercised |
| Base output model | Independent emulator levels/MASK, off/steps/max/min, DTRs, groups/scenes and selected queries/configuration; validated base-output codec, service and MQTT PowerState/Brightness with actual readback and real PTY/emulator evidence | Settings/fault extensions and timed fades; group colour/scalar bindings |
| Addressing model | Addressed discovery, unaddressed output/input assignment, full reassignment, identity-preserving lost-address adoption and sole-candidate repair; individual addressed clearing retains saved records/reservations and supports explicit recovery; independent consumers, literal UART/model and browser tests | Physical timing |
| DALI-2 inputs | Named modules/instances and durable rename; native MQTT state/events and receive cutover; exact 1..255-bit values; checked event-setting plans and independent setting/value/latch model; strict MQTT/HTTP/typed common settings, Part301 button timing and all nine base Part302–304 sensor parameter bindings with actual readback; frozen protected browser settings with exact units/minima and unsupported-hold handling; literal UART/PTY/MQTT/phone/desktop tests | Semantic stimulus generation, feedback bindings/settings, unresolved group/source routing, newer sensor sensitivity/range capabilities, calibrated units and physical validation |
| DT7 switching | Pending | P4 model/driver |
| DT8 Tc/xy/RGBWAF | Typed values/plans, independent physical emulator, owned discovery/execution/actual readback and selected-family scalar MQTT; separate colour quality/capabilities, partial xy/RGB channels, manual/automatic activation, combined power/colour ordering and normalized RGBWAF assignment preflight; durable representation selection; private nonactivating scene rows | Physical validation, Primary-N control, alternate modes/settings |
| Split-address RGB/RGBW | Pending | P4 representation-specific components and bindings |
| Input indication/feedback | Pure feature/feedback codec, opt-in independent visible on/off profile, strict managed MQTT/HTTP/typed module/instance read/set with fresh support/presence and actual confirmation; compact protected browser toggle and literal/PTY/MQTT/phone/desktop tests | Scalar Desired bindings, parameter settings and broader profiles, group selectors and physical validation |
| Identity/memory reads | Pending | P4 model/driver |
| Data Parts 251–253 | Pending | P4 luminaire/energy/diagnostics with units/status |
| DT1 emergency | Pending | P4 status/configuration/test operations |
| Output group membership | Saved-group creation/rename, captured per-fixture read, one-bit assignment, reviewed hardware adoption and full-mask repair; browser pattern preview/Add/Replace and bus-wide verification; independent literal peers/emulator/private MQTT, consumer and browser tests | Physical validation |
| Output group power/dimming | Strict owned native group OFF/MAX/DAPC and individual membership/level/status capture; explicit saved-inventory coverage, uniform/mixed/empty/unknown aggregates; MQTT/HTTP/typed commands and compact protected Groups controls; independent literal/PTY/MQTT/client and real phone/desktop tests | Scalar Desired bindings, DT8 group colour and physical validation |
| Native scene protocol | Typed slots/levels/repeated settings, nonactivating Tc/xy/RGBWAF staging and stored reports; independent sixteen-slot colour model and standalone PTY programming/query/recall/Remove; durable named slots and private per-Origin desired rows; shared MQTT/HTTP Set/Adopt/Repair/Capture and stored verification | Physical validation, fades and Primary-N |
| Scene operator workflows | Durable named slots; owned stored brightness/Tc/xy/RGBWAF reads, native bus/group recall, private row save/program/readback, reviewed adoption/repair and actual capture candidate; literal/PTY/MQTT/authenticated HTTP and real phone/desktop cases; offline saved reader, reviewed capture/copy/clear/adoption/repair composition and all-slot verification | Physical validation; browser repair uses matching evidence, native drift repair has service/literal coverage |
| BridgeKit MQTT | Canonical daemon, managed inventory service, fenced targets, metadata commands, base output properties, original trace and independent consumer/ephemeral-broker tests | Remaining capability paths; local shared dependency commit must be published for portable clones |
| Protected web UI | Embedded responsive Preact sign-in, overview/bus setup, Devices, Groups, Scenes, Settings and Diagnostics; real phone/desktop workflows including naming, inputs, DT8, pattern groups, native group power/dimming with mixed/unknown readings, scenes, full reassignment, lost-address adoption and confirmed individual address clearing | Remaining capability workflows and physical deployment acceptance |
| Store device/preset integration | Approved design; implementation pending | P8 schema and persisted migration |

The emulator's J/N/X/Z records describe transport outcomes. N is not a positive
device capability response. Unsupported model operations leave state unchanged;
they must not be used to claim device support. Direct and scene levels currently
change immediately; timed UP/DOWN is unsupported until the fade slice.

Manufacturer extensions, firmware update, other physical transports and DALI
certification are outside the approved scope. Emulator coverage never substitutes
for physical validation. See [emulator usage](emulator.md) and the
[phase ledger](v2-progress.md) for reproducible local checks and remaining work.
