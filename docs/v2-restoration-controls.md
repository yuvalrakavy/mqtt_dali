# Address recovery fault controls

For bridge contributors. The adjacent TOML table deliberately breaks input Origin
continuity, confirmed save/permit/stamp guards, sole-candidate proof, permanent
queued-action revocation, and browser original authority, route/root fencing,
confirmed Stop and refresh-at-adopted-address behavior.

Run the existing runner with the warm bridge target cache:

```sh
CARGO_TARGET_DIR=/path/to/target python3 /path/to/Store/scripts/negative-control.py \
  --root /path/to/mqtt_dali-v2 /path/to/mqtt_dali-v2/docs/v2-restoration-controls.toml
```

Serialize source edits, builds, controls and browser runs. Every row must fail for
its declared diagnostic and restore its source. Finish with restored tests,
affected control-plan checks, scoped strict lint, Pi cross-check, changed-file
format, wait registry and built-asset checks. Real phone/desktop acceptance is
required for workflow changes. Individual native address clearing is separate
from this identity-mapping recovery operation.
