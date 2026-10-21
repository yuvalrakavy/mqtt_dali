# v2 implementation progress

Development checkout: `Home/mqtt_dali-v2`, branch `feat/dali-v2`.
Base: `db6f39f63da75a0614eb8f1a3b274a163bc4b307`.
The approved architecture and phase acceptance criteria are in
`superpowers/specs/2026-10-21-dali-v2-design.md`.

## P0 — configuration boundary implemented

The library `src/config.rs` and development binary `mqtt_dali_v2 --check-config`
select a valid existing v2 first, otherwise import v1 into a separate file or
initialize an empty inventory. They preserve legacy bytes and names, addresses,
ordering and group membership. New files are private, size-bounded and published
atomically without replacing another creator's destination. Invalid v2 never
falls back. Synchronous filesystem work and diagnostic output run on detached
workers with bounded caller waits. This is a check command; daemon startup remains
legacy until P3 integrates the new service and promotes the executable.

Verified on 2026-10-21:

- Native full suite: **72 passed**, including 27 migration/library/process cases
  and the existing legacy regressions; all use isolated fakes, never the live bus
  or broker. The wait registry passes.
- Strict Clippy for library, v2 binary and migration process tests: passed with
  warnings denied. New Rust files pass rustfmt; `git diff --check` passed.
- Raspberry Pi `armv7-unknown-linux-musleabihf` library/v2 compile check: passed.
  This does not verify linking, deployment or physical hardware.
- Tests were observed failing before implementation. The full-output-pipe
  regression was also observed failing before its fix.
- Five reproducible negative controls all failed for their expected reasons;
  see `v2-migration-controls.toml` and its recorded ledger. They remove alias,
  no-follow, no-replace, output-deadline and worker-detachment protections.
- Separate sequential design/contract reviews and a fresh code review completed.
  The code review requested bounded diagnostics, stronger alias tests and an
  explicit default Cargo binary; all were fixed and reviewed again with an
  approval for P0. Available reviewers are from one model family; cross-family
  review remains unavailable.

Limitations: permission checks skip root; check output is best effort after
200 ms; directory ownership is trusted. Crash-during-write/retry is tested, but
power-loss durability and malicious directory replacement are not. Input, scene
and capability fields will extend the inventory in their owning phases; this
pre-release schema rejects unknown fields rather than discarding them.

Review learning: process-exit tests must include full unread output pipes as well
as stalled workers; path-safety tests must start with absent files, since an
unrelated parse error can otherwise hide a missing guard. No shared skill files
were changed during bridge implementation.

## P1a — typed wire, monitoring and serial substrate implemented

`src/atx/wire.rs` preserves bus, width, collision and version fields; its
incremental framer bounds storage and partial-line lifetime without discarding
idle events. `monitor.rs` owns one continuous reader and serializes byte writes
under caller deadlines. Subscribers get explicit lag, persistent lifecycle status
and error/EOF/panic notification. Cancellation during a possibly partial write
prevents further writes. Close cancels an active writer and releases both stream
halves even when client handles remain.

`serial.rs` uses nonblocking Unix descriptors with raw 19200/8N1 settings on both
Pi UART and desktop PTY. Opening/configuration runs on a detached worker under a
2 s caller deadline. A local PTY test checks actual terminal settings, idle input
events and exact outgoing bytes through the production adapter.

Verified on 2026-10-21:

- Full native nextest suite: **92 passed**, including 20 new wire/monitor/PTY
  cases and all 72 previous regressions. Tests and close/write-failure regressions
  were observed failing before their corresponding implementations/fixes.
- Strict Clippy for the library, v2 binary and config process tests: passed;
  Raspberry Pi library/v2 compile check: passed. Neither is a Pi runtime test.
- Six reproducible UART negative controls all failed for their expected reasons:
  line size/lifetime, write poisoning, close cancellation/resource release and
  raw mode. See `v2-uart-controls.toml` and its recorded ledger.
- Fresh source review requested descriptor release and EINTR handling. Fixes were
  reviewed again and approved for this substrate scope. EINTR retry was inspected,
  not fault-injected; physical UART teardown remains unverified.

Review learning: resource ownership must be tested after close while public
handles remain alive. Tests also distinguish canceling a queued write from
canceling a potentially partial write, since only the latter poisons admission.

The raw `write` receipt means only bytes accepted by the kernel. P1b supplies the
exclusive transaction owner above it.

## P1b — transaction dispatcher implemented

`Transactions` privately owns the monitor and a continuously running dispatcher.
Capacity-one admission covers queued and active work. The same reader fences old
buffered/partial lines; framing and the first accepted-write sequence boundary
share the split-stream lock. Explicit drain emptiness is distinct from retry
Pending. Async input events, other-bus replies and echoes cannot complete a query.
L bit 16 distinguishes commands from input events.

Fully written canceled/timed-out exchanges retain their remaining terminal count.
Corruption, lag, interference or a potentially partial write requires a later boot;
an arbitrary reply or version message cannot reopen admission. Initialization is
explicit. Unverified j/t completion is refused; verified/emulated repeat profiles
can declare one or two terminals. Close resolves an active caller and releases
the stream. No transaction result reports physical state confirmation.

Verified on 2026-10-21:

- Full native nextest suite: **118 passed**, including 46 ATX cases and all
  migration/legacy regressions. Production Serial plus Transactions exchanges
  actual bytes through a local PTY with interleaved input and other-bus traffic.
- Strict scoped Clippy passed; Pi library/v2 compile check passed; diff whitespace
  and Rust formatting checked. The wait registry passes after its required tags,
  local-method declaration and explicit expiry outcomes were completed.
- Six new negative controls all failed for their expected assertions: retry/empty
  confusion, incomplete-line boundary, caller and actor expiry, lost late slot,
  and unverified repeats. See `v2-transaction-controls.toml` and recorded ledger.
- Sequential design review restored a dedicated actor to keep idle events drained.
  Fresh code review found drain/first-write and ready-result deadline races; fixes
  and focused regressions were reviewed again and approved. Cross-family review
  remains unavailable. Physical HAT firmware behavior remains unverified.

Review learning: generic AsyncRead Pending may mean retry/cooperative yield rather
than an empty buffer. Correlation requires a stronger explicit drain contract.
Deadline tests must separately exercise actor completion and a caller resuming
after a ready result, since either check can hide a missing guard in the other.

## P2 — independent standalone ATX emulator implemented

`src/emulator` owns strict physical scenarios independently of bridge inventory,
with separate UART parsing/formatting and literal vectors. Its base gear model
supports levels/MASK, steps, limits, selected status/configuration queries, DTRs,
groups/scenes and commissioning address selection/search/program/withdraw. Model
time can be realtime or frozen/advanced; seeded randomization is reproducible.
Duplicate addresses affect all selected fixtures and conflicting replies collide.

`dali_hat_emulator` creates a private PTY and optional private Unix JSON-control
socket. It uses tracing-init, bounded detached startup/output workers, bounded
queues/timers/clients, persistent UART write offsets and independent transport
deadlines. Controls inject events/faults, inspect actual fixture state and simulate
address loss, bus power changes and HAT reboot. Reboot preserves fixture DTR and
initialization state and delimits truncated output. Both PTY descriptors are
close-on-exec and released before successful close.

Verified on 2026-10-21:

- Full native nextest suite: **146 passed**, including 24 emulator library tests
  and four standalone process tests, plus all 118 previous regressions. Real PTY
  tests use production Serial/Transactions and verify independent snapshots.
- Process checks cover private socket permissions, refusing existing paths,
  invalid/oversized/unfinished controls, and SIGTERM with full unread output.
  Model tests cover range versus exact selection, binary search, repeat windows,
  expiry, RESET retention/isolation and unsupported operation behavior.
- Strict scoped Clippy passed. Pi library/v2/emulator compile check passed.
  Wait registry and formatting passed. Cross-linking and Pi runtime remain P9.
- Six emulator negative controls failed for their declared assertions: repeat
  gating, exact selection, reboot delimiter, retired controls, timer bounds and
  partial-line lifetime. See the control table and recorded ledger.
- Sequential design/code reviews completed. Code findings for truncated-record
  recovery, mixed-width repeat invalidation and STEP UP/RESET semantics were
  observed failing in regressions, fixed and reviewed again with approval.
  A further two-fixture RESET test closes the review's nonblocking coverage gap.
  Reviews are source-only and from one model family; physical behavior unverified.

Limits: immediate direct/scene levels, no timed UP/DOWN/fade physics yet. DT8,
semantic inputs, emergency and memory/data models belong to P4. The synthetic
version/repeat profiles are not real-firmware evidence. Unix socket cleanup is
best effort after exit. Usage and capability boundaries are in `emulator.md` and
`support-matrix.md`. Browser emulator controls follow the protected HTTP phases.

Review learning: track UART record boundaries from accepted bytes across frames,
including deliberate faults; HAT reset must not reset separately powered gear.
Independent models need adjacent opcode/default tests as well as roundtrips.

## P3a — qualified target codec frozen and implemented

`src/targets.rs` provides typed construction/parsing for output/input/group/scene
targets and input instances, with a separate inventory route. Uppercase UTF-8
percent escaping preserves names exactly while keeping dot-separated targets
inside one MQTT topic segment. Strict canonical decoding rejects aliases and
invalid names; the 4096-byte encoded bound counts separators and namespaces.

Verified: four focused tests cover literal/collision/Unicode/metadata vectors,
invalid forms and exact 4096/4097-byte output/instance boundaries. Three meaningful
cases were observed failing before implementation. Three negative controls caught
removed canonicality, dot escaping and encoded-size guards. Separate design and
code reviews approved the slice; review additions cover lowercase valid-name
escapes and the exact instance bound. Full native suite: **150 passed**; strict
scoped Clippy and Pi compile checks passed. No MQTT/Store caller uses this codec
yet; complete topic/packet validation belongs to integration.

The remaining P3 work includes shared managed-service lifecycle, authoritative
inventory/request-generation fencing, the DALI service and canonical v2 daemon.
The managed-service design is grounded in current BridgeKit's run/start split;
it retains the single shared logging/signals/watchdog policy.

## P3b — managed process service implemented in BridgeKit

Shared checkout: `Projects/tracing-init-dali-v2`, branch
`feat/dali-managed-services`, commit `a3db09e` (local, unpublished).
`ProcessService`, `ServiceContext` and managed run/start APIs keep process-owned
resources independent of MQTT/device link lifetimes. Readiness validates the
proxy once; an independent heartbeat watcher closes admission after service
death. Startup cancellation retains cleanup, and all component teardown shares
one absolute deadline. Success confirms future, local-task and runtime-resource
destruction; blocked helpers/destructors cannot claim a clean exit.

Verified in the shared checkout: **160 kit tests** and **174 conformance tests**
passed, strict all-target Clippy for both crates passed, and the armv7 Linux musl
library compile check passed. Conformance used explicit `RUST_LOG=info`; the
inherited `warn` filter hid INFO in two existing log assertions. Six negative
controls detected their declared removed protections, and the restored full kit
suite passed. Design and fresh core reviews approved after readiness/admission,
runtime-teardown and absolute-deadline fixes. Reviews are source-only, same model
family; physical hardware remains unverified.

The bridge has not adopted this dependency yet. Portable dependency pinning is
required before daemon integration is final. Inventory reconciliation and stale
request fencing are the next P3 contract boundary. The shared spec and its
verification record are mirrored in `superpowers/specs/2026-10-21-dali-v2-p3b-services.md`.

## Remaining phases

| Phase | Status | Next concrete boundary |
|---|---|---|
| P1 UART | Implemented and verified locally | Physical firmware transcripts remain a hardware-validation item; conservative unverified policy is the default |
| P2 ATX emulator | Implemented and verified locally | P4 capability extensions and physical validation remain explicit |
| P3 BridgeKit/service | Target codec and shared managed lifecycle implemented; integration pending | Inventory/retired-request contract and canonical v2 daemon |
| P4 capabilities | Pending | Outputs, color representations, DALI-2 inputs, explicit capability support |
| P5 commissioning | Pending | Addressing, identification, names, group/scene workflows |
| P6 authenticated HTTP | Pending | Optional server, authentication and shared-service API |
| P7 browser UI | Pending | Approved responsive interface and browser acceptance |
| P8 Store integration | Pending | Scoped dev-track and persisted-data migration preserving bindings |
| P9 release/docs | Pending | Cross-build, deployment/rollback guides and hardware status matrix |

No v2 release, production installation, live Store migration, push or merge has
been performed. The legacy master checkout and v1 maintenance branch retain their
existing application source.
