# Build and verify a release

The release builder creates a versioned archive containing only `mqtt_dali_v2`,
`dali_hat_emulator`, their service launcher/installer, portable configuration,
documentation and emulator scenarios. It does not deploy, tag or publish anything.
The package version remains a development candidate until a release is approved.

## Prerequisites and source

Use Git, Python 3.11+, Rust 1.99.0 from `rust-toolchain.toml`, and a linker/std
target for the selected OS. Cargo.lock is committed. All SDK packages pin the
same HTTPS Git revision, `d83fa224bf04216dcde4be4102b1b2dc98fd7010`, available
from the SDK's public `feat/dali-managed-services` branch. No sibling path checkout
or unpublished local SDK commit is required. Other bridges keep their own pins.

```sh
git clone https://github.com/yuvalrakavy/mqtt_dali.git
cd mqtt_dali
git checkout <approved-v2-commit-or-tag>
rustup target add armv7-unknown-linux-musleabihf
python3 scripts/release.py --target armv7-unknown-linux-musleabihf
```

The maintained v1 branch does not contain this v2 build recipe. Select the approved
v2 ref explicitly. The Pi cross-build uses `arm-linux-gnueabihf-ld` and
`arm-linux-gnueabihf-ar` from `.cargo/config.toml`. Install a compatible binutils
toolchain or build on the target with the appropriate linker configuration.
ARMv7 musl avoids a deployment glibc version requirement; it still needs a
compatible Linux CPU/kernel. For a 64-bit Pi OS choose
`--target aarch64-unknown-linux-musl` and its Rust target/linker. Other targets
are accepted only when the builder and installer recognize their executable format.

For native development use `--target aarch64-apple-darwin` (Apple Silicon) or your
Linux target. macOS archives can be run directly for testing; the systemd installer
only activates filesystem installation on Linux. It supports staging elsewhere.

Reuse an existing warm cache by setting `CARGO_TARGET_DIR` to that repository's
canonical target directory. Do not create a new target directory for every check.
`--offline` adds Cargo's offline build mode when all dependencies are cached.

Embedded browser assets and npm lockfiles are committed. Node 24.11+ is needed
only to rebuild/check browser assets, not to install or run a binary package:

```sh
npm --prefix web ci
npm --prefix emulator-web ci
npm --prefix web run check-dist
npm --prefix emulator-web run check-dist
```

If changing browser sources, run each affected project's build/test commands and
commit its generated dist changes before making the release.

## Artifact contract

The builder requires clean source and uses `cargo build --locked --release` for
the two named executables. An explicit `--allow-dirty` creates a development-only
artifact with `-dirty` in its name and manifest. Never publish that as a clean
release. Existing output names are refused instead of overwritten; use `--output`
with a new directory for a second build of the same identity.

`release.json` records source commit, dirty status, SDK revision, Rust toolchain,
target, Cargo.lock digest, commit timestamp and each payload file's digest/mode.
`SHA256SUMS` covers the payload and manifest; a sidecar covers the whole archive.
The build sets `SOURCE_DATE_EPOCH` to the source commit timestamp. Archive order,
timestamps and ownership are normalized. Identical build inputs yield a
deterministic archive; compiler output across different hosts/toolchains is not
promised to be byte-identical. There is no release-signing implementation.

## Local acceptance before publication

Run package/installer/launcher tests without root or host service calls:

```sh
python3 -m unittest discover -s tests -p test_release.py -v
```

These cover payload verification, architecture, destination containment/conflicts,
fresh/repeated/upgrade/rollback installation, interruption before selection,
configuration preservation, v1 coexistence and literal launcher handling.
Use `python3 install.py --root /tmp/dali-stage` from an extracted package for an
additional filesystem inspection. A staged result does not prove systemd activation.

Run the native Rust regression suite through cargo-nextest, production Clippy,
asset checks, a locked ARM release build, and packaged MQTT/TCP-emulator acceptance.
The acceptance script runs the extracted binaries and launcher against its own
loopback broker, verifies authenticated web access and actual native fixture state,
then stops only its own processes:

```sh
python3 scripts/release_smoke.py /path/to/extracted/package
```

It requires the Mosquitto broker and CLI tools on PATH. Run it with a package for
the current host. It creates private temporary state rather than touching a real
UART, deployed broker, service, or household configuration.

A clean-clone locked build should resolve the pinned SDK and use the same warm
target cache. Inspect ARM ELF architecture/linking as part of cross-build checks.
Record actual results and remaining limits in [the progress tracker](v2-progress.md).
Real Linux/systemd activation and physical I4/fixture acceptance remain separate
deployment checks; emulation cannot establish either one.

Publication requires the owner's release/license decision, reviewed release notes,
checksums and artifacts. Physical limitations must remain explicit in those notes.
