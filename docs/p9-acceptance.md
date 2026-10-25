# P9 local release-candidate acceptance

Accepted source: `7eb906c55025631dfb3ccfa3192ac3fe35452d1f`, branch `feat/dali-v2`.
Package version: `2.0.0-dev.0`. This is a local candidate, not a published release.
The separate documentation commit records results without changing candidate code.

## Deliverables

Versioned native and ARMv7 musl archives are in the ignored `dist/p9/` directory.
Each includes the versioned bridge, independent HAT emulator, verified installer,
optional service/launcher, portable config, operator guides, scenarios, dependency
notices, manifest and checksums. No v1 executable, private site logging file,
password or deployment state is included.

| Target | SHA-256 of `mqtt_dali_v2-2.0.0-dev.0-7eb906c55025-<target>.tar.gz` |
|---|---|
| `aarch64-apple-darwin` | `cf6200d40b537e25923d311b6cb0b3bb6247c31e379ffffee7e7b2fe4ea6136e` |
| `armv7-unknown-linux-musleabihf` | `e7f2666c8620e59723fb15ea1f71d44d78bf85fcc62252d24a88b35d25ece0b1` |

Use [installation](installation.md) for config/import/service activation and
rollback. Installation never starts/enables/restarts the bridge. V1 `mqtt_dali`
and its original config/service remain separate from v2 `mqtt_dali_v2`.

## Verification

- **20/20 Python tests**: source/compiler/SDK metadata, archive integrity and
  deterministic contents, architecture, destination containment/conflicts,
  config preservation, upgrade/rollback, copy/reload failure, mocked Linux
  command boundaries, launcher/secret handling and optimized-Python refusal.
- **869/869 native Rust tests**, one opt-in browser check skipped; strict
  production Clippy passes. No Rust runtime code changed after this P9 run.
- Both embedded browser asset checks pass; unchanged frontend/browser baseline
  evidence remains in [the phase tracker](v2-progress.md).
- Seven deliberate guard removals produce their expected behavioral failures;
  [control table](v2-release-controls.toml) anchors are current. Independent mutation
  review exposed a missing wrong-compiler assertion, which was added and controlled.
- Clean local clone builds both archives using `--locked --offline`, Rust1.99.0,
  the canonical warm target and published HTTPS SDK pin `d83fa224`. No local SDK
  path patch is required. Native/ARM build times are16.84s/16.96s for the final
  application rebuilds; these are warm builds, not cold-clone speed claims.
- The archive's own installer stages successfully. Its own smoke runner passes
  twice against a private broker and TCP HAT in1.128s/0.544s: embedded UIs,
  unauthenticated API refusal, MQTT inventory/control, authenticated native
  output job, actual fixture-model/MQTT readback, bounded stop, secret privacy
  and inactive local log records. No real UART, household broker or service.
- Public running version/build exactly matches package version and source
  commit timestamp. Repacking the actual native inputs gives the identical
  archive SHA-256. ARM executables are static ELF32 ARM EABI5 hard-float, with
  no dynamic section or undefined symbols.

## Review corrections

Depth review approved the frozen implementation. Breadth review found two
failure boundaries: optimized Python could omit acceptance assertions, and a
failed systemd reload could already have selected the new binary. Both regressions
failed before their fixes; the script now refuses optimization and reload occurs
before selection. The single confirming review passes. Source reviews were
GPT-family only; they did not independently rerun the execution checks.

Actual package execution also corrected the smoke's sequencing to await MQTT
completion and changed portable logging from invalid `level="off"` to supported
`filter="off"`. BridgeKit source and other bridge defaults/pins are unchanged.
The runtime empty-log assertion verifies the portable filter behavior.

## Configuration-link ARMv7 refresh

The updated Raspberry Pi archive includes authenticated configuration links from
clean source `03b289bb5bf40f7f51258a5957b8c1e6cc730b67`:

- File: `dist/p9/mqtt_dali_v2-2.0.0-dev.0-03b289bb5bf4-armv7-unknown-linux-musleabihf.tar.gz`
- Size: 9,989,770 bytes.
- SHA-256: `eef5e109be86e399d1dc29ce78871721a81526f896641c6cae9a5277969385fe`.

The locked, offline optimized build succeeds with the pinned Rust1.99.0 compiler.
The archive sidecar and every packaged payload checksum pass. Both executables
are static ELF32 ARM EABI5, with no interpreter, dynamic section or undefined
symbols. The extracted installer stages successfully, keeps private configuration
permissions and preserves an existing v1 executable. No ARM binary was executed
on the development Mac; physical Pi/HAT and systemd acceptance remain pending.
The earlier native archive above remains at its original source revision.

Build log: `/tmp/dali-config-launch-arm-release.log`; verification report:
`/tmp/dali-config-launch-arm-verification.json`.

## MIT-licensed ARMv7 refresh

The 26 October archive supersedes the earlier ARMv7 candidate for installation:

- Source: `38204b554d7ded7f5e9c4e0e4e90146d08a309b9` (clean).
- File: `dist/p9/mqtt_dali_v2-2.0.0-dev.0-38204b554d7d-armv7-unknown-linux-musleabihf.tar.gz`.
- Size: 9,990,674 bytes.
- SHA-256: `5bc96444c7be6c28316505ae8313a8364a1c8b2dd5410a8cba1d6747dcc9d2e9`.

All20 existing package/installer tests pass; Cargo metadata declares MIT. The
locked offline optimized build passes. The extracted project license exactly
matches the approved root `LICENSE` and is covered by the manifest and checksums.
Archive/payload checksums, static ELF32 ARM checks and staged installation pass,
preserving an existing v1 executable and private config permissions. No physical
Pi/HAT execution or service activation was performed.

Logs: `/tmp/dali-mit-package-tests.log`, `/tmp/dali-mit-arm-release.log` and
`/tmp/dali-mit-arm-verification.json`.

## Remaining release and deployment work

The owner selected MIT on 26 October 2026, matching the shared SDK. The root
`LICENSE` and Cargo metadata record the choice. New release archives include the
project license alongside existing dependency license texts. Public release
approval remains separate.

Actual Linux/systemd activation, physical ATX I4 completion/timing profiles,
fixture interoperability and live Logmon delivery still need deployment acceptance.
No physical hardware certification, household migration, main merge, tag, push,
publication or service activation occurred in P9. A systemd reload failure can
leave a copied unit/release for retry while keeping the previous binary selected.

Execution logs: `/tmp/dali-p9-{review-green,native-tests,clippy,controls-final,
candidate-native,candidate-arm,candidate-acceptance}.log`; asset logs use
`web-assets`/`emulator-assets`. Reviews: `/tmp/dali-p9-{design,mutation,depth,
breadth,confirm}-review.md`. These local logs are supporting evidence; the source,
test/control definitions and build recipe are the durable reproduction path.
