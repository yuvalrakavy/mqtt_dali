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

## P3c1a — pure inventory/reconciliation layer implemented

`inventory::Model` validates complete schema-1 wire snapshots, initializes the
legacy schema-2 baseline, replays immutable named history and proposes atomic
edits. It distinguishes continuity from replacement, preserves semantic group
membership through address moves, and rotates dependent group fences on address
reuse. Revisions allocate above the private high-water value. Present null or
incomplete ledgers fail validation. Imports preflight canonical targets and the
whole snapshot before creating v2, with exact document/encoded-target bounds.

A separate consumer algorithm and literal wire fixture verify offline rename
chains, rename-back, swaps, removal/reuse, preserved device record ids, intent
continuity, old-generation observation rejection and atomic rejection of invalid
or partial documents. These are independent-consumer contract tests, not actual
Store migration/binding tests. The latter remain P8.

Verified: **164 full native tests passed**, six declared negative controls caught
their removed guards, and the armv7 library/v2/emulator compile check passed.
Scoped strict Clippy is part of the checkpoint gate. New behaviors were observed
failing first, including identical-text replacement and rename-back intent
continuity. Two sequential design reviews cleared this slice after uncertain
save, transition-chain and runtime-bypass refinements. Fresh source review found
missing intermediate bus-name/index consistency and incomplete metadata checks
in the reference consumer. Three regressions reproduced those gaps; fixes were
reviewed again and approved. The complete gate below includes those regressions.
Guidance and current limitations are in `inventory-contract.md`.

This remains a pure proposal/snapshot API; the durable owner below applies its
proposals. Generation-context enforcement and runtime
state retirement ordering require the P3c2 gate before mutable daemon integration.

## P3c1b — durable configuration owner implemented

`config_store::ConfigStore` owns one runtime blocking worker and an adjacent
private lifetime lock. Save admission is nonwaiting and serialized. Both candidate
documents are preflighted before an old-mapping revision reservation is saved,
then the new mapping is atomically replaced and synced. Every public revision,
including descriptions, is reserved; v1 is never created or modified by this owner.

Timeout/cancellation preserves the active worker's slot and lock. Uncertain
replacement stays frozen until explicit recovery reloads a recognized candidate
and confirms file/directory durability. Cancellation and successful finalization
share one state transition. Startup confirms durability too. Close releases file
ownership across retained handles; a blocked worker returns false under the
caller deadline and remains part of managed-runtime teardown.

Verified on 2026-10-21:

- **182 full native tests passed**, including 15 persistence tests and three
  inventory review regressions. Local fault injection covers before/after both
  replacements, cancellation, queued/startup cancellation, failed sync/recovery,
  revision burning/restart, unknown external edits and lock/path safety.
- Eight reproducible negative controls all detected their declared removed
  guards; see `v2-persistence-controls.toml` and its recorded ledger. The restored
  full suite and wait registry passed. Strict scoped Clippy and the armv7
  library/v2/emulator compile check passed; changed Rust formatting/diff checked.
- Fresh source review found a finalization/cancellation race and cleanup armed
  before temporary-file ownership. Deterministic regressions were observed
  failing first; fixes passed re-review. Review was source-only, same model family.

This is a library checkpoint. Process-level stalled-filesystem SIGTERM tests
belong to canonical daemon integration; physical power-loss behavior and Pi
runtime are unverified. Operator-owned directories and ordinary local Unix
filesystems are assumed. No new daemon MQTT/hardware routing is admitted yet.

## P3c2 — shared request-generation fencing implemented

Shared dependency commit: `273062f05c1f0df05cb35f19bad458f2691d8dea` on
`feat/dali-managed-services` in `Projects/tracing-init-dali-v2`. The reviewed
contract and verification record are mirrored in
`superpowers/specs/2026-10-21-bridge-target-generations.md`.

Fixed Bridge instances (including managed startup) can opt into complete target
generation tables. MQTT metadata, queued work, configuration, commands, reads,
reports/removals and sealed publications retain their original context. Atomic
admission/table retirement prevents old intent replacing fresh queued work;
late observations cannot enter a replacement's model or conflict history.
Default unfenced behavior remains compatible. Dynamic factory fencing is
explicitly unsupported and rejected before connection.

The full kit gate passed 194 tests and the existing black-box conformance suite
passed 174 tests. Strict all-target Clippy, three rustdoc tests and the armv7
library compile check passed. Sixteen guard-removal controls each failed at its
intended behavior assertion, including admission/post interleaving, stale direct
and proving reads, original output metadata, late removals and legacy retry/cache/
partial-state compatibility. The scheduling test is controlled coverage with a
200 ms window, not an exhaustive interleaving proof. Two sequential source
reviews approved after four reproduced compatibility findings were fixed;
cross-family review is unavailable. No live broker, HAT or Pi runtime was tested.

The shared runtime spec, README, tutorial and canonical bridge skill were
synchronized. The dependency commit is still local/unpublished. Actual DALI
service routing, durable-save/retirement/publication ordering, and canonical
daemon integration remain next; this bridge has not adopted the new dependency
yet. `mqtt_dali_v2` remains the configuration-check development entry point.

## P3c3a — managed inventory service implemented

`service::DaliService` now owns the durable inventory and routing phase in a
managed BridgeKit service. Owned snapshots and bounded notifications feed
frontend handles; one nonwaiting operation permit spans each admitted save or
recovery. Public edits are metadata-only. Hardware replacement/readdress/group
membership require later verified commissioning jobs.

Reporter attachment is independent of broker/HAT availability. A durable change
retires the complete old runtime table, queues Inventory and only then exposes
Ready. Uncertain saves and failed runtime CAS remain frozen. Explicit recovery
compares the full public model, including recognized mappings recovered with
`changed: false`; runtime-only retry retains the original exact CAS precondition.
The proxy checks both persisted and actual runtime controller identity and
preserves original caller deadlines. Version and UTC build time are in snapshots.

The bridge pins all tracing-init repository dependencies to local development
commit `259d963fd448ea2512e36a00e2f22850af0bc1f7`. This includes a reviewed
single-use command-completion capability: a metadata MQTT command can return one
original-token success across its own inventory commit; all unrelated/queued old
work still retires. The dependency is unpublished; a portable fresh clone/release
still requires publication. Tokio 1.50 in the bridge lock resolves the older
lock's missing OwnedNotified API.

The independent consumer now buffers bounded State-before-Inventory observations,
separately from identity and intent. Exact-token retractions, accepted/ignored
inventory outcomes and total key/payload cache budgets are exercised, along with
an actual MQTT inventory rename preserving consumer records and owned intent.

Verified on 2026-10-21:

- **201 complete native tests pass**, including actual output/group/bus/description
  MQTT success replies, canceled-save recovery via MQTT and direct handles,
  original generation/correlation, stale requests, directory-sync/CAS failure,
  absolute caller deadlines, instance checks, bounded update lag and actual
  blocked-filesystem incomplete stop. Existing migration/UART/emulator/legacy
  regressions and the wait registry remain green.
- Nine service/consumer controls each failed at the intended assertion; the runner
  restored source and recorded `v2-service-controls.ledger.json`. Strict scoped
  Clippy, armv7 library/v2/emulator compilation and changed-file formatting/diff
  checks passed. The shared completion boundary separately passed 200 kit tests,
  174 existing unfenced conformance tests and four controls.
- Sequential source reviewers approved after three reproduced integration findings
  were fixed: own-command reply retirement, ready-after-deadline acceptance and
  actual runtime instance mismatch. Review is source-only and same model family.
  The identity rejection test has a controlled 500 ms absence window; it is not
  an exhaustive scheduling proof.

See `service-development.md`, `inventory-contract.md` and the P3c3 contract.
This is still a service-library checkpoint. UART ownership/health and canonical
daemon startup come next. Physical capabilities, web UI, Store persisted migration,
live Logmon delivery, hardware validation and release/deployment remain pending.

## P3c3b — managed UART health implemented

Historical checkpoint: the general Boot-only recovery prerequisite below is
superseded by the owner's measured quiet-period decision in P4a.

The optional `with_uart` builder starts one Serial/Transactions owner inside the
managed service runtime. Version and enabled-bus status queries use one original
setup deadline; metadata remains available during initialization, MQTT/HAT loss
and poisoned-response waits. Hardware snapshots preserve raw version/status
facts independently of saved routing phase, including Connecting and actual
Closed. Bus power faults do not masquerade as a disconnected HAT.

Late/reset states keep the same owner. Reopen retains a reset latch and a
no-write receive fence excludes already-readable complete/partial Boot records.
Only qualifying Boot plus the transaction actor's Uninitialized state admits
initialization. This does not attribute upstream-delayed data to a physical reset.
Before fixture Send is enabled, P4 must also preserve unresolved transport
ambiguity across process crashes/startup; health queries do not claim that wider
guarantee. Firmware fields never select repeat policy implicitly.

Open/configuration uses a runtime-owned blocking worker. Open timeout is terminal
for the UART owner; actual runtime teardown still includes a blocked opener.
Owned helper results survive canceled selects, and panic results are inspected
before recovery. Hardware/configuration cleanup shares one absolute local budget.

The focused PTY/emulator cases cover late Version retirement, poisoned-owner
Boot recovery, actual transport replacement, complete/partial buffered Boot,
quiet retry after failed boundary, completed-helper panic during Boot/loss,
completed-boundary/fresh-Boot ordering, proactive observation ordering, metadata
during unanswered setup and actual descriptor release with retained handles.
The blocked-opener case confirms incomplete managed stop and no retry eligibility.

Source reviews reproduced and fixed three recovery/ownership issues: a missing
retry timer, a completed panic discarded by a simultaneous hardware event, and
a successful boundary discarded before a qualifying Boot was evaluated. Connecting
and Closed publication were corrected too. These are source-only same-family
reviews. Final verification on 2026-10-21: **215 full native tests passed**,
scoped strict Clippy and armv7 library/v2/emulator compilation passed, and changed
Rust formatting/diff checks passed. All eight controls in
`v2-uart-service-controls.toml` failed at their intended assertions; restored
source passed the full gate. No physical I4, live broker/Logmon or Pi runtime is
implied. The UART slice remains a library integration; canonical daemon startup
is next.

## P3c3c — canonical daemon and original request traces

The canonical `mqtt_dali` and `mqtt_dali_v2` alias now start the managed v2
service through BridgeKit. Explicit `--check-config` preserves the isolated
offline operation. The former main is byte-preserved apart from its test-helper
path in `mqtt_dali_v1`, a regression harness; deployed legacy fixes remain on
the separate v1 maintenance branch/tag. Package/retained Version reports
2.0.0-dev.0 and actual UTC build time. No install scripts or live services changed.

The common CLI parser preflights options and renders usage through bounded
output, then valid arguments pass unchanged to BridgeKit. The standalone offline
flag is recognized only in flag position; even a controller named --check-config
works with split or inline option values. Saved identity is checked before UART.
First startup migrates v1, restart uses v2, and deletion of v2 reimports with a
new inventory epoch while preserving v1 bytes.

Real canonical process tests cover MQTT metadata replies, clean/incomplete stop,
held logging/config/save startup, actual broker-outage-independent UART health,
full unread output pipes, idle CPU and private local OTLP export. The shared
trace defect exposed by that test was corrected in reviewed local tracing-init
commit `96a03bcb9f932d8d49d479a38cc95f9ad9e08ff6`, consistently pinned across all
shared dependencies. Validated owned causes cross queue boundaries independently
of generation authority; coalesced callers retain their own traces. The DALI
actor exports an operation span, and committed Inventory uses the new generation
with the original cause. The exact success reply retains original generation,
correlation and trace. Actual decoded OTLP JSON proves worker and actor spans
under the incoming trace at INFO, rather than an ingress-only match.

Shared restored verification passed 211 kit tests, 174 existing conformance
tests, strict all-target Clippy in both crates, rustdoc and armv7 compilation.
Eight shared controls failed at their intended assertions; README/tutorial,
runtime design and canonical bridge skill were synchronized. Two sequential
source reviews approved after reproduced attribution gaps were fixed. An
existing proving-read test race was corrected with an actual connected barrier.

Daemon source review approved after fixing an Inventory-publication wait, a CLI
mode/value ambiguity and a displaced Markdown separator. The new daemon controls,
changed recovery control and relocated legacy/migration controls are recorded
beside their ledgers. Final restored daemon verification passed **225 native
tests**, scoped strict Clippy, armv7 library/canonical/alias/emulator/legacy-harness
compilation, changed Rust formatting, wait registry and diff checks. The explicit
test-feature logging-start case also passed. All **60** controls in this batch
failed at their intended assertions: five daemon, one changed recovery, five
relocated migration and 49 relocated legacy cases. Restored source passed after
all mutations. No physical behavior or Pi linking is implied by compilation.

See `running-v2.md` and `service-development.md`. The dependency commit is local;
portable upstream clones still require publication. HTTP/capabilities, Store
migration, physical I4/Pi runtime and live Logmon validation remain pending.
Fixture Send is still disabled pending the P4 capability engine. The earlier
crash-evidence prerequisite is superseded by the owner's P4a recovery policy.
Available source reviewers are same family; no cross-family independence is claimed.

## P4a — measured quiet recovery implemented and verified locally

The owner chose measured UART silence instead of the proposed persistent
clean/dirty journal. No journal, state directory or first-start reboot requirement
was implemented. One continuous Tokio AsyncFd reader still processes raw
19200/8N1 input and publishes complete records during recovery. The new
`--uart-quiet-ms` option accepts 1..5000, default 250, before opening resources.
Every nonempty read restarts quiet acquisition, including partial/malformed
records; its total budget is the interval plus the existing setup allowance.

Exclusive actor recovery uses a guarded reader reset with original deadline,
cancellation identity and expected raw activity, then checks the sealed boundary
before entering Uninitialized. It retires complete-write timeouts/receive loss;
V/D initialization is still explicit. Known partial-write poison remains
Boot-only and is captured after actual teardown before a port reopen. Real joins
and lifecycle-owned panic cleanup release descriptors even with retained handles
or canceled close callers. Journal-only clean receipts/history were removed.

Behavioral RED→GREEN tests cover the timer, raw input, complete-write/corruption
recovery, automatic startup/reconnect, invalid CLI values and teardown bugs.
Controlled reader/actor tests cover canceled/expired reset, new activity after
proof, input visibility and partial poison. A canonical process test feeds input
before launch through inventory readiness, measures the configured interval,
kills an unanswered process and restarts with an old version buffered, without
Boot. The partial-write EOF test delivers owner closure before actor cancellation
and proves post-join poison survives into Boot-only reopening.

Two sequential design reviews approved the replacement. First full source review
approved production behavior and requested removing an independent-MQTT timing
race from the subprocess test; the feeder now starts before process launch.
Final source review additionally required caller cancellation to reach the reader
without actor polling. A shared synchronous authorization guard and final actor
check now enforce it; deterministic regressions were observed RED before each
fix. A final stress probe exposed starvation of the quiet timer by ready event
traffic. The actor now checks its deadline before each select, independently of
caller polling; that regression was also observed RED before the fix. Both
reviewers approved the corrected source. Reviews use the same model family.

Verified on 2026-10-22:

- Full native nextest: **246 passed**, including legacy and canonical process
  regressions. Strict scoped Clippy and Raspberry Pi library/all four binary
  compile checks passed. The feature-gated blocked-logging SIGTERM test passed;
  doctests passed (none defined). The wait registry passes.
- **16 new recovery negative controls** all failed for their intended behavioral
  assertions. The 25 existing UART, transaction, UART-service and daemon controls
  also behaved as declared. The affected transaction controls were rerun after
  the final deadline guard. Tables and ledgers preserve the actual evidence.
- Source formatting and whitespace passed. The full restored native suite passed
  all **246 tests** after the guard-removal mutations.

Physical I4 timing, Pi runtime, live Logmon and upstream publication remain P9.

Review learning: a cancellation flag must reach the resource owner directly,
actual task joins must precede resource-release claims, and a biased event loop
must check its total deadline independently of lower-priority timer polling.

## P4b1 — typed base-output codec implemented and verified locally

Two sequential design reviews approved a pure Part 102 codec. It validates
short/group addresses, normalizes ordinary levels within 0..254, creates basic
write/query frames and preserves query-specific MASK/type/status meanings plus
no-response/collision outcomes. Twelve literal-vector tests pass; the initial
new-API build could not compile until its types were implemented. This was not
a behavioral regression RED. Thirteen guard-removal controls subsequently failed
for their intended behavioral assertions, establishing sensitivity to removed
bounds, address/control bits, max/MASK distinctions, positive-level handling,
intent validation, no-response meaning and status polarity. The ATX integration
vector uses the production encoder.

Verified on 2026-10-22: all **258 native tests passed**, including the full restored
suite after controls; scoped strict Clippy and Pi library/all four binaries compile
checks passed. Formatting, whitespace, wait registry and doctests (none defined)
passed. Two sequential read-only source reviews approved the final codec; the
only requested cleanup was a stale design-status sentence. Reviews remain
same-family, and physical fixture behavior is unverified.

No MQTT fixture writes, configuration changes or emulator codec sharing are
introduced. Physical execution, original-generation admission, readback and
fair collection of jobs/events remain the next integration boundary. Reference:
[dali output contract](dali-output-contract.md).

## P4b2 — bounded dispatch and helper collection

Deterministic fail-first probes reproduced queued admission being stranded by
continuous input, completed HAT helpers being starved by renewed observations,
and normal EOF becoming fatal when its query result preceded public delivery.
Transaction dispatch now prefers queued requests after stop; ordinary command
drain/fence/deadline checks remain intact. The HAT owner consumes a bounded prefix
of 129 public receive attempts, then actually joins a completed helper and retires
the stored handle without a suspension between those steps. Queued bus facts keep
priority over earlier query snapshots. Processing work returns promptly even
when it leaves the hardware snapshot unchanged.

A Closed query or status starts owned teardown; its actual join determines
whether a paced retry is allowed. Late EOF cannot abort an owned Close helper.
Actual helper/actor failures remain fatal and post-join partial-write poison is
preserved. Five new tests cover cancellation/expiry, renewed input plus completed
panic/newer bus data, actual PTY EOF ordering, late EOF, status ordering and an
actual aborted actor with delayed public evidence. Six new guard-removal controls
all failed for their intended assertions. The test fixtures use private streams
and PTYs and remain bounded.

The full native suite passed **263 tests**; strict scoped Clippy,
Pi library/all four binaries compilation, wait registry, formatting, whitespace
and doctests (none defined) passed. All 30 existing transaction, UART-service and
quiet-recovery controls failed for their intended assertions. After restoration,
the full native suite again passed **263 tests**. Both sequential source reviews
approved without material findings. Reviews use the same model family.

This closes scheduling prerequisites; physical property execution and readback
remain pending. No live HAT, broker, Store or deployment changed.

## P4b3 — owned output operations and actual fixture readback

The shared service now reads an individual saved output and applies validated
power/brightness intents through its sole UART owner. Metadata and physical
operations share admission. A frontend timeout does not release that admission:
the owner keeps it until actual actor retirement or transport teardown. One
original deadline reaches transaction admission, receive fences and every new
write poll; cleanup has a fixed ceiling. No new frontend owns a UART clone.

Post-write actual-level/status queries provide observed results. Missing fixtures,
MASK, collisions and partial samples remain typed evidence, not an invented off
or echoed request. UART and per-bus counters invalidate stale work even when a
watch value returns to Ready before the owner polls. Mapping changes retain only
exact surviving tokens. Cache and reply share the same final validated sample.

Two source-review findings were reproduced behaviorally before correction:
an expired cleanup could accept a ready permit, and a bus transition could leave
a stale cache paired with successful current readback. Both regressions now pass.
Both sequential source reviews approved; they use the same model family.

The initial full native gate passed **282 tests**, plus strict scoped Clippy,
Pi library/all four binary compilation, wait registry and whitespace checks.
All **19 new guard-removal controls** detected their intended behavioral faults.
All **42 existing affected controls** also behaved as declared: transaction,
UART monitor, service lifecycle, quiet recovery and dispatch progress. The
status-ordering control now targets the current early-close detection site;
its original failure assertion is preserved. After restoration, all **282 native
tests** passed again; strict Clippy, Pi compilation, formatting, whitespace and
doctests (none defined) passed. Hardware validation remains pending.

This is a library execution boundary. Physical MQTT properties, group execution,
color and inputs remain subsequent P4 work. See [the service API](output-service.md).

## P4b4 prerequisite — explicit request context for SDK reads

Shared commit `153d38051f5197b060fa4281efe6136cffee7419` adds the optional
`read_state_with_request_context` hook. Queued refresh and individual conflict
reads now pass their owned original context; compatible defaults preserve
existing contextual/plain drivers and independently captured observation tokens.
Four focused tests include actual ephemeral-broker State/reply traces and
explicit absence across a driver-owned task without OTel. Both dispatch
regressions failed behaviorally before the worker fix.

Both sequential design/source reviews approved; reviews remain same-family.
The restored shared gate passed **215 kit tests** and **174 full conformance
tests**, all-target strict Clippy, Pi library compilation, formatting and wait
registry. Three new and ten affected existing controls detected their intended
faults. All four DALI dependencies now pin this exact commit; DALI's full
**282 native tests**, strict scoped Clippy and Pi library/all four binaries
compile checks pass with the new pin. Physical MQTT property execution is next.

The revision remains local and unpublished. The bridge-development skill and
shared README/tutorial describe the new method; no Store code changed.

## Remaining phases

| Phase | Status | Next concrete boundary |
|---|---|---|
| P1 UART | Implemented and verified locally | Physical firmware transcripts remain a hardware-validation item; conservative unverified policy is the default |
| P2 ATX emulator | Implemented and verified locally | P4 capability extensions and physical validation remain explicit |
| P3 BridgeKit/service | Codec, lifecycle, inventory, persistence, fencing, managed service/UART and canonical daemon implemented | Capability paths; local shared dependency publication before portable clones/releases |
| P4 capabilities | Quiet recovery, typed output codec and individual output service implemented | MQTT output driver paths, color representations, DALI-2 inputs and explicit capability support |
| P5 commissioning | Pending | Addressing, identification, names, group/scene workflows |
| P6 authenticated HTTP | Pending | Optional server, authentication and shared-service API |
| P7 browser UI | Pending | Approved responsive interface and browser acceptance |
| P8 Store integration | Pending | Scoped dev-track and persisted-data migration preserving bindings |
| P9 release/docs | Pending | Cross-build, deployment/rollback guides and hardware status matrix |

No v2 release, production installation, live Store migration, push or merge has
been performed. The legacy master checkout and v1 maintenance branch retain their
existing application source.
