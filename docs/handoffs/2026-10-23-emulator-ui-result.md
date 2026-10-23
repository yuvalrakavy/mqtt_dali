# Emulator UI implementation result

Implementation branch: `feat/dali-emulator-ui`.
Worktree: `/Users/yuval/Documents/Projects/Home/mqtt_dali-emulator-ui`.
Baseline: `7ddc1ac9a6de05357121e3c6307822224e71dde3`.
Feature commit: `d28a5d708206c9f030c5de59b1d73565ba1b84be`.
No merge, push, deployment, production configuration, real HAT or real broker
operation was performed. Implementation stayed in the assigned worktree.

## Delivered

- Explicit numeric `--hat-listen IP:port`, with one raw ATX client and persistent
  native model across disconnected/slow/failed clients. Port0 ready records;
  fresh Boot/parser/output session at reconnect; retained physical state and
  discarded old replies/events/faults. Default PTY and existing socket APIs remain.
- Separate explicit loopback `--web-listen IP:port`, numeric Host checks on all
  routes, exact Origin/CSRF on mutations, bounded handlers/owned shutdown and
  committed Preact assets. No new Cargo dependencies or `src/lib.rs` changes.
- Responsive Live installation / Starting scenario / Advanced browser. Scenario
  create/edit/import/export/reset/capture, live add/remove, stable IDs, unknown
  native types through full records, native output/colour/contact/feedback views,
  semantic physical input controls, and explicit raw compatibility controls.
- Strict compatible optional version1 capture fields for native gear settings,
  colour actual/control/scenes, input `initial_settings` and `initial_physical`.
  Capture excludes transient registers/latches/transport/time/history. Drafts
  remain separate from the saved start and live model. Import errors preserve
  native state; dirty JSON conflict/reconnect/draft-retention safeguards are tested.
- Semantic native filter/enable/source scheme and button/sensor timers on one
  model clock. Stale deferred reports and disabled periodic timers are covered.

Operator/API/schema details are in [emulator-web.md](../emulator-web.md).

## Checks actually run

All Cargo checks used isolated
`CARGO_TARGET_DIR=/Users/yuval/Documents/Projects/Home/mqtt_dali-emulator-target`,
APFS-cloned while the root session explicitly held its cache idle.

- `cargo test --offline --target aarch64-apple-darwin --lib --test hat_emulator --test wait_registry`:
  **653 library passed, 1 pre-existing ignored; 9 standalone passed; 1 registry passed**.
- Final four TCP/reset tests passed after strengthening both-profile coverage to
  actually send a repeated native settings command and query readback; these
  use the production transaction dispatcher and semantic native events. Reconnect
  testing observes actual disconnect state, rather than an arbitrary yield count.
- Fail-first semantic initial-value and stale-deferred-value regressions were
  observed failing, then passed after their fixes. Six original deep-review
  findings and its nested-profile followup were fixed and re-reviewed cleanly.
- `cargo clippy --offline --target aarch64-apple-darwin --lib --bin dali_hat_emulator --test hat_emulator --test wait_registry -- -D warnings`: **passed**.
- `cargo check --offline --target armv7-unknown-linux-musleabihf --lib --bin dali_hat_emulator`: **passed**, including final embedded assets.
- `npm --prefix emulator-web run build` (TypeScript + Vite), `check-dist`, scoped
  Rust formatting and staged diff whitespace checks: **passed**.
- Actual Chrome phone390×844 and desktop1440×1000 workflows: **passed**. Includes
  installation building, dimmer/DT7/DT8/input instances, native raw TCP updates,
  semantic press/long/release/full10-bit sensor event, live add/remove/cancel,
  address loss, capture/reset, saved download/import, invalid full/nested JSON,
  captured instance add/remove, dirty JSON conflicts, and actual process restart
  at the same endpoint with CSRF refresh and local draft retention.

Latest browser artifacts:
`/var/folders/jv/t_kwbmsx4l19_13z1f9lg2040000gp/T/dali-emulator-browser-Z3GarO/`
(`phone-live.png`, `desktop-live.png`, `desktop-advanced.png`). The runner owns
and closes its disposable process, private socket, raw peer and Chrome context.

Broad repository `cargo fmt --check` and Clippy `--all-targets -D warnings` were
also attempted. They expose existing unrelated legacy/v1/test formatting/lint
violations. Those files were left untouched; scoped changed-target checks pass.

## Integration

Cherry-pick the feature commit; this result document is a separate docs-only
commit. Shared-file conflict surface is only `docs/wait-registry.md`:

- Add `emulator-tcp-serve`, `emulator-reset-fence`, `emulator-web-serve`,
  `emulator-web-handler`, `emulator-web-close` rows.
- Retain root's newer feedback waiters. Add `accept` to `wait-methods` because
  the emulator's Listener trait implementation makes that a local method name;
  dependency TcpListener accept must remain recognized/tagged.
- Regenerate Waiters after integration. Actual new function names include
  `Emulator::listen_tcp`, session `run`, `Server::bind`, `Server::close`,
  `protect`, and `<Listener as Listener>::accept` in `web_transport.rs`.

Root-owned `CLAUDE.md`, progress/support matrices and master spec were not edited.
Update their old emulator-handoff status and link the new operator guide during
integration. No connector changes are required; use the existing `--hat-tcp` seam.

## Limits and remaining integration evidence

The browser uses an owned literal raw ATX peer; production Transactions is tested
against the actual new listener, including repeated settings and semantic events.
The combined managed bridge-tab discovery/assignment/rename and MQTT State flow
over this listener was not rerun here; use root's existing managed/browser harness
after integration. Existing full-library managed and standalone PTY evidence is
retained, but is distinct from that combined acceptance.

Only loopback HTTP is supported. Mobile evidence is a real browser with phone
viewport, not a physical iPhone/Safari network test. Timers have 20 ms granularity
plus scheduler delay; large jumps coalesce repeats/reports. Semantic queried
values above 32 bits require raw byte controls. Priority/arbitration physics,
physical fades and actual HAT certification remain outside the software profile.
Advanced profile editing uses strict JSON where no dedicated form is provided.
