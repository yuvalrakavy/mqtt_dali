# P9: installable v2 release candidate

Implementation of the approved v2 architecture, with the owner's additional
requirement that v1 `mqtt_dali` and v2 `mqtt_dali_v2` can be installed together.
T2: deployment paths and upgrade behavior are new operational interfaces.
No real machine deployment, service activation, publication, tag, or household
migration is authorized by this phase's local checks.

## Shape and evidence

Use an ordinary versioned tar archive and opt-in systemd unit. A distro package
would add repository/signing/distribution work before the first release; a shell
installer alone would omit checksums and a versioned rollback target. Existing
legacy installers remain v1-only because they rewrite networking and reboot.

Verified: the existing exact SDK pin d83fa224bf04216dcde4be4102b1b2dc98fd7010 is
available on origin/feat/dali-managed-services. Change Cargo's HTTP Git URL to
HTTPS and regenerate the committed lockfile at that same revision. Shared SDK
code and other bridge pins do not change. Rust 1.99.0 is installed; pin that
toolchain and document its ARMv7 musl target/linker. Committed web assets and npm
lockfiles already exist. `built` honors SOURCE_DATE_EPOCH; the build script needs
an explicit rerun-if-env-changed declaration for it.

## Files and release contents

- `scripts/release.py`: locked release build of only mqtt_dali_v2 and
  dali_hat_emulator, source commit/time and toolchain/target metadata, deterministic
  tar member order/ownership/time, SHA256SUMS and release.json manifest. Fail on
  dirty source by default; an explicit development option marks dirty artifacts.
  Use canonical CARGO_TARGET_DIR; do not clean or build in a new cold cache.
- `packaging/install.py`: Python3 installer, package verification and versioned
  directories, staged `--root DIR` mode for testing, existing-config preservation.
- `packaging/start`: POSIX launcher with quoted arguments, no eval; finds the
  sibling versioned executable and uses explicit env-to-option mapping.
- `packaging/mqtt_dali_v2.service`, `packaging/bridge.env.example`, portable
  `packaging/logging.toml`; a console default plus documented Logmon endpoints.
- `README.md`, `docs/installation.md`, `docs/release.md`, current guide links and
  support/progress updates. Source/runtime API guides in the shared SDK and Store
  are already present; verify their links rather than duplicate their content.
- Installer/release unit tests and a packaged real MQTT/TCP-emulator acceptance.
  License file/manifest field depends on the pending explicit license choice.

The binary archive includes bin/, libexec/start, packaging installer/config/unit,
README, docs, emulator scenarios, release.json and hashes. It excludes the legacy
binary, private logging config, local state/passwords, node_modules and build cache.
Package metadata names the exact source and SDK revisions and Cargo.lock digest.
This is a locked build recipe and deterministic archive for identical inputs;
cross-toolchain/cross-host compiler byte identity is not claimed.

## Installation and upgrade

Installed immutable releases live under
`/opt/mqtt_dali_v2/releases/{version}-{commit}-{target}`; current is an atomic
symlink. `/usr/local/bin/mqtt_dali_v2` and dali_hat_emulator link through current.
Only the v2 unit is installed at `/etc/systemd/system/mqtt_dali_v2.service`.
Existing v1 binary/service/configuration are never edited or deleted.

Configuration: `/etc/mqtt_dali_v2/bridge.env` and logging.toml; state:
`/var/lib/mqtt_dali_v2/dali.v2.json`, optional read-only import dali.json. Create
only absent config files (mode0600 for env); upgrades preserve their bytes. Live
Linux install creates a dedicated mqtt-dali-v2 service account and writable state,
then daemon-reload only. Never enable/start/restart a service automatically.
Staging under a non-root destination does no account/systemctl/host modification.
The installer preflights checksums and structure before touching the destination.
Release content is copied to a temporary sibling before the version/current switch.
Same release installs converge; conflicting content under a fixed release ID fails.

The unit uses the service account, supplementary dialout for UART, state working
directory, EnvironmentFile, Restart=on-failure, TimeoutStopSec=30 and UMask=0077.
Its launcher requires instance/broker, passes explicit state/logging paths, selects
UART or TCP and repeated-command profile, and only enables web when configured.
The password remains in MQTT_DALI_WEB_PASSWORD, not process argv. No default
password and no unauthenticated web listener are introduced.

## Rollback and data

Operators stop the active bridge before switching a HAT. Keep previous version
directories; reinstall/select the prior verified v2 package and explicitly restart.
Do not rewrite current inventory automatically during binary rollback. Restore a
reviewed pre-upgrade state backup if an older v2 schema needs it. The installer
preserves configuration; the guide includes backup/restore commands.

For v1 rollback, stop/disable v2 and start the maintained legacy service using its
untouched binary and original v1 configuration. v2-only renames/settings do not
magically become v1 configuration. Import a copy of v1 to the v2 state directory;
normal precedence and explicit delete-to-reimport remain unchanged. No restore,
addressing or other physical operation is performed by installation or rollback.

## State/guard enumeration and verification

| State/input | Operation and exit | Evidence/test |
|---|---|---|
| Source clean/dirty, SDK pin present/mismatched | Build accepted or refused before cargo; manifest reflects source | Metadata/dirty-source tests; actual clean clone locked build |
| Missing/wrong binary, tampered package | Refuse before destination writes | Content/checksum tests and wrong-target validation |
| Absent/live/staged destination | Install immutable release; create only missing files | Fresh staged install and host-command refusal assertion |
| Existing config and release | Preserve config; converge or refuse differing same ID | Upgrade/idempotence/tamper tests |
| Install interrupted before switch | Old current remains; retry can remove own temp directory | Injected copy failure, assert old link/config remain |
| Current switch complete | Report installed paths; operator decides enable/start | Fresh and upgrade symlink checks |
| Launcher values contain spaces or shell syntax | Preserve literal arguments, never eval | Fake executable argv capture |
| Web disabled/missing credentials/configured | No web options or explicit validation, password via env | Launcher and actual protected endpoint acceptance |
| Bridge stopped/running, operator rollback | Explicit manual stop/select/restart | Staged previous/current package check and rollback guide |

Inspect source/destination paths, config preservation and unit commands during the
design review. Design gate: one fresh-context pass, bounded; build the actual
package test-first after that. Final source gate: mutation then frozen source
review, at most one confirming fix round. All available models are GPT-family.

Run Python installer/package tests and launcher tests, native scoped/full checks
appropriate to build/dependency changes, both embedded asset checks, native and
ARMv7 release builds, real packaged emulator/MQTT/auth smoke, and a clean-clone
build using the existing warm target. Inspect ARM ELF headers and unresolved
symbols. A real systemd start and physical I4 acceptance remain explicitly pending
if no Linux test host/HAT is available. Publish only after owner approval of the
concrete artifacts and remaining limitations.

## Design gate disposition

Fresh Astra/high review found the design sound and buildable. Both preflight
clarifications are implemented: normalized regular source members and destination
containment, and refusal of unrelated destination leaves before any host change.
TCP selection omits UART, launcher values remain literal, and config/password
handling and private-umask read permissions have tests. SDK source pin stays
unchanged. Review record: /tmp/dali-p9-design-review.md (GPT-family coverage).

## Implementation checks before final source review

The independent mutation reader exercised seven guards. Six initially failed as
expected; the pinned-compiler guard exposed a missing wrong-version assertion.
That test is added. All seven recorded controls now fail for behavioral assertions;
the control plan reports no stale anchors. Table: docs/v2-release-controls.toml.

Actual clean-clone native and ARM archives build at d98c445. The first packaged
smoke passed, while a later run exposed harness sequencing: native level changes
before MQTT request completion, so an immediate exclusive web operation can be
Busy. The runner now waits for confirmed MQTT State before submitting the next
operation, and waits for final MQTT State before checking its result.

BridgeKit pins file/GELF sink creation in its logging bootstrap. The portable
config filters those two sinks off and enables console/journal records; an empty
rotating file is still created. Logmon opt-in is documented with destination
levels and OTLP endpoint. No shared SDK source or other bridge behavior changes.
