# v2 capability status

This is development status during P4, not a release or certification statement.
The canonical v2 executable runs BridgeKit with metadata and UART health. No v2 physical HAT
or fixture behavior has been validated. The v1 maintenance binary is separate.

| Capability | Local implementation/evidence | Remaining work |
|---|---|---|
| v1 configuration import | Library and canonical process tests; v1 bytes preserved; v2 precedence and reimport | Persisted Store-object migration in P8 |
| ATX I4 UART | Typed frames, continuous monitoring, measured quiet recovery, bounded request/helper progress under input traffic, managed health and real PTY/process crash-restart tests; known partial-write poison survives in-process reopen | Physical timing/firmware transcripts; j/t completion policies; no persistent inference of partial writes across process death |
| Base output model | Independent emulator levels/MASK, off/steps/max/min, DTRs, groups/scenes and selected queries/configuration; validated base-output codec, service and MQTT PowerState/Brightness with actual readback and real PTY/emulator evidence | Group execution, settings/fault extensions and timed fades |
| Addressing model | INITIALISE modes/expiry, seeded randomization, search/compare/program/withdraw, duplicate address responses | P5 bridge jobs and commissioning UI |
| DALI-2 inputs | Async L records remain public during transactions; standard codec/query frames; saved named modules/instances with durable MQTT rename and generation retirement | Hardware discovery, state/events, configuration, feedback and semantic emulator controls |
| DT7 switching | Pending | P4 model/driver |
| DT8 Tc/xy/RGBWAF | Pending | P4 limits, commands and color-aware scenes |
| Split-address RGB/RGBW | Pending | P4 representation-specific components and bindings |
| Input indication/feedback | Pending | P4 capability discovery and supported operations |
| Identity/memory reads | Pending | P4 model/driver |
| Data Parts 251–253 | Pending | P4 luminaire/energy/diagnostics with units/status |
| DT1 emergency | Pending | P4 status/configuration/test operations |
| Group/scene operator workflows | Pending | P5 verification, repair, assignment, recall/copy/clear |
| BridgeKit MQTT | Canonical daemon, managed inventory service, fenced targets, metadata commands, base output properties, original trace and independent consumer/ephemeral-broker tests | Remaining capability paths; local shared dependency commit must be published for portable clones |
| Protected web UI | Approved design; implementation pending | P6/P7 service/authentication/browser tests |
| Store device/preset integration | Approved design; implementation pending | P8 schema and persisted migration |

The emulator's J/N/X/Z records describe transport outcomes. N is not a positive
device capability response. Unsupported model operations leave state unchanged;
they must not be used to claim device support. Direct and scene levels currently
change immediately; timed UP/DOWN is unsupported until the fade slice.

Manufacturer extensions, firmware update, other physical transports and DALI
certification are outside the approved scope. Emulator coverage never substitutes
for physical validation. See [emulator usage](emulator.md) and the
[phase ledger](v2-progress.md) for reproducible local checks and remaining work.
