# Development retrospectives

Each feature entry records time/evidence, catches, friction, tier, delegation,
the number of unconsolidated entries, and one concrete improvement.

## 2026-10-25 P9 release candidate (T2)

- time: design/implementation totals were not separately measured; final warm
  native/ARM application builds16.84s/16.96s, native test execution21.536s,
  final packaged smoke1.128s/0.544s. Full logs are named in p9-acceptance.md.
- catches: design=2 preflight clarifications; mutation=1 missing compiler assertion;
  breadth=2 failure-boundary findings; runtime=2 smoke/logging assumptions;
  user=0 this phase; post-merge=not applicable (not merged).
- DG/IG: no weighted score or prior-window baseline was recorded; raw counts
  above are retained rather than inventing comparable scores.
- friction: source URL correction rebuilt SDK dependents once; subsequent builds
  reused the canonical cache. No cold cache, global clean, or repeated Rust gate.
- tier-call: T2 was appropriate for new installation/upgrade interfaces.
- delegation: Astra/high design+depth; Sol/medium mutation+confirmation;
  Sol/high breadth. Same GPT family. Implementation remained inline.
- unconsolidated: 1
- improve: before claiming a packaged default, run one actual launcher/config
  probe; the empty-local-log assertion now enforces the logging claim. Test
  fallible host calls before the final selector and preserve assertion execution.
