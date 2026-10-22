# Scene browser and saved-reader fault controls

For contributors verifying the protected Scenes implementation. Run the existing
Store control runner against this bridge checkout:

```sh
CARGO_TARGET_DIR=/path/to/warm/target python3 /path/to/Store/scripts/negative-control.py \
  --root /path/to/mqtt_dali-v2 /path/to/mqtt_dali-v2/docs/v2-scenes-ui-controls.toml
```

The table contains eighteen intentional faults: five native reader/job cases,
twelve frontend model/authority cases, and one real browser case. Each must fail
with its declared behavioral diagnostic. An unrelated compile or timeout failure
does not prove the guard. The runner restores each source mutation and records
results in the adjacent ledger. Serialize source edits, builds and controls.

Coverage includes optional read-token/Ready refusal, every configured output in
the saved view, the large-reader budget, compact summaries/full-result retrieval,
untouched native values, coalesced row saves, complete private replies, original
root/connection/lifetime fences, frozen capture/copy settings, incomplete native
evidence, unconfirmed writes, consumed revisions and stale adoption.

The real browser control removes the late-creation lifetime guard, rebuilds
assets and demonstrates that the old completion erases the next slot's name
draft. It needs the normal local Chrome/Playwright dependencies. Its source is
restored automatically; its generated assets are rebuilt separately. Finish
with:

```sh
npm --prefix web run build
npm --prefix web run check-dist
```

Accepted source is `77e7ae1`. All eighteen current rows have intended-RED evidence:
the initial sixteen in `/tmp/dali-scenes-ui-controls`, consumed-revision and
refreshed external-edit rows in `/tmp/dali-scenes-ui-revision-controls`, and
stale adoption in `/tmp/dali-scenes-ui-adoption-fixed`. The thirteen prior browser
controls and the existing HTTP encoded-result control also ran successfully.
Restored/final evidence is linked from [progress](v2-progress.md#p5h3--protected-scenes-screen-and-reviewed-operations).

These tests cover the private broker/PTy emulator and deterministic models. They
do not certify physical firmware/fixture behavior or untested interleavings.
