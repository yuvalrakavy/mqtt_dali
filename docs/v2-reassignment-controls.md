# Reassignment behavioral fault controls

For bridge contributors. Run the existing Store control runner against this
checkout with the same local target cache as the restored tests:

```sh
CARGO_TARGET_DIR=/path/to/warm/target python3 /path/to/Store/scripts/negative-control.py \
  --root /path/to/mqtt_dali-v2 /path/to/mqtt_dali-v2/docs/v2-reassignment-controls.toml
```

The fourteen rows deliberately break durable activation, permit retention,
original/final physical evidence, absence checks, input repetition, unknown
output-profile refusal, independent raw input addresses, and frontend original
roots, selected-space refresh, uncertain-result stopping, capacity and frozen
review. Require the declared behavioral diagnostic, not an unrelated compile,
timeout or harness failure. Results live in the adjacent ledger.

Serialize mutations, source edits and verification in the checkout. The runner
restores source after every row. Finish with restored native/frontend tests,
format/wait checks, strict scoped lint, Pi cross-check, control-plan checks,
asset byte-check and the real phone/desktop browser workflow. The browser test
covers actual tab-lifetime cancellation; the unit tests also cover reconnect,
external revisions and independent cancellation flags.

The initial permit row failed its Busy assertion but lacked the table's custom
message. Its diagnostic was corrected and that row rerun; this was a test-report
correction. See [progress](v2-progress.md) for final source and acceptance logs.
No physical HAT or live Store validation is claimed.
