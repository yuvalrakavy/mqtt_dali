# v2 capability status

This is development status during P3, not a release or certification statement.
The v2 executable remains configuration-check only until daemon integration. No v2 physical HAT
or fixture behavior has been validated. The v1 maintenance binary is separate.

| Capability | Local implementation/evidence | Remaining work |
|---|---|---|
| v1 configuration import | P0 library/process tests; v1 bytes preserved | Integrate daemon startup in P3 |
| ATX I4 UART | Typed frames, continuous monitoring, transaction matching, managed health/recovery and real PTY/emulator tests | Canonical daemon; physical firmware transcripts; j/t hardware completion policies; durable crash/reset boundary before fixture Send |
| Base output model | Independent levels/MASK, off/steps/max/min, DTRs, groups/scenes, selected queries/configuration | P4 driver paths, faults and timed fades |
| Addressing model | INITIALISE modes/expiry, seeded randomization, search/compare/program/withdraw, duplicate address responses | P5 bridge jobs and commissioning UI |
| DALI-2 event transport | Async L records remain public during transactions; local injection | P4 Parts 301–304 decoding, discovery, state and configuration |
| DT7 switching | Pending | P4 model/driver |
| DT8 Tc/xy/RGBWAF | Pending | P4 limits, commands and color-aware scenes |
| Split-address RGB/RGBW | Pending | P4 representation-specific components and bindings |
| Input indication/feedback | Pending | P4 capability discovery and supported operations |
| Identity/memory reads | Pending | P4 model/driver |
| Data Parts 251–253 | Pending | P4 luminaire/energy/diagnostics with units/status |
| DT1 emergency | Pending | P4 status/configuration/test operations |
| Group/scene operator workflows | Pending | P5 verification, repair, assignment, recall/copy/clear |
| BridgeKit MQTT | Managed inventory service, fenced targets, metadata commands, independent consumer and ephemeral-broker tests | Canonical daemon, physical capability paths; local shared dependency commit must be published for portable clones |
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
