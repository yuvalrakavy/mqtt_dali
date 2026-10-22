# Private scene-row fault controls

Contributors run `v2-scene-settings-controls.toml` with the existing Store
`scripts/negative-control.py --root <bridge> <table>`. Each row changes one named
mechanism, executes a focused test, then restores source. Serialize controls,
builds and edits. The generated ledger records actual outcomes; do not substitute
plan validation for execution.

The fourteen sensitivity rows require an intended behavioral RED: public contents
leak, invalid Origin accepted, a missing attribute treated as MASK, missing
cleared-report copy, failed Remove followed by Store, failed save followed by
programming, stale evidence accepted, adoption programming hardware, repair
adopting drift, missing repair recheck, fabricated capture, unknown HTTP
parameters accepted, replaced physical authority and capture incorrectly requiring
mutation authority. Literal peers use independent native bytes and pending
colour; an emulator exercises Tc/xy/RGBWAF with both repeat terminal profiles.

The held save/bus-cycle test exercises overlapping physical admission/receive
guards. A separate case reuses the existing HAT harness with the bus cycle already
observed before enqueue: it isolates the saved candidate's original stamp while
the current bus is ready. Removing that guard must queue work and fail the case.
No new runtime hook or recovery mechanism is required.

The timeout case proves admission is Busy while work is active and that no new
UART work crosses unresolved reply ordering after the original deadline. It
does not require Busy to remain the only refusal after actor settlement. The
accepted quiet-period recovery and finite teardown behavior remain unchanged.
