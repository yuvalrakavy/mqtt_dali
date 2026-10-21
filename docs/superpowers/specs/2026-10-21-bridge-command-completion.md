# A command that commits its own inventory change

Source-grounded integration correction to target generations, not a new MQTT
wire grammar. A DALI rename currently commits its file and calls Reporter CAS
inside execute. That retires the active command's own old target/instance token,
refuses its reply, and discards its eventual success. Queued work must still be
retired, so preserving every active command is insufficient.

## Exact active completion capability

- Mint an opaque, cloneable `CommandCompletion` for each taken momentary command
  (including WithReply), using unique allocation identity rather than a reusable
  counter. Keep it beside the mailbox's current command. Reads do not get one.
- Scope that capability only around the actual contextual execute future using
  Tokio task-local scope. A new `Reporter::command_completion()` returns a clone
  only from that scope; validation, helpers, service actors and later calls do
  not inherit it. Capture in the proxy and explicitly forward it to its owner.
  No driver trait method or RequestContext equality/coalescing semantics change.
- Add `Reporter::replace_generations_for_command(expected, next, &completion)`.
  Under the existing mailbox -> Reports lock order, first require that exact
  capability still belongs to the active momentary command and is unused. A
  missing/stale/foreign/already-used capability refuses before any table change.
- Then perform the same complete-table CAS and retirement. Preserve only that
  active command's final completion; consume its capability on successful CAS.
  Retire all queued/unrelated old commands, writes, configurations, observations
  and runtime caches exactly as before. The ordinary replacement API continues
  refusing active retired commands too.
- Completion keeps the original target-generation and correlation data. It may
  finish once with success or failure; it cannot authorize another operation,
  a second commit, an old observation, or old target access. Existing stop,
  disconnect and watchdog refusal can still cut off that active command.
- Clearing/taking the active command also clears its capability. A later command
  cannot acquire the old capability even with identical target/token/text.

This is a trusted in-process driver capability, not MQTT authentication. No new
wire field is added and remote clients cannot mint it. Capturing one does not
prove persistence: the owner must still freeze, confirm durability, CAS, queue
inventory and then expose Ready. HTTP edits use the ordinary replacement API.

## Verification

First reproduce the actual MQTT rename reply failure in mqtt_dali. Shared tests
must prove one old-context success for the exact active committing command,
queued old command retirement, stale/foreign/reused capability rejection without
table changes, read/helper/no-scope absence, ordinary replacement retirement,
and stop's once-only refusal. Use existing ephemeral FakeBroker tests and actual
consumed metadata/correlation data. Add controls removing active identity and
exact completion preservation. Run shared full tests, unfenced conformance,
strict Clippy, docs, Pi compile and wait registry. Keep README/tutorial/runtime
extension docs and canonical bridge guidance synchronized, then pin the bridge
to the resulting reviewed local commit. Publication remains separate.

Core reviews are sequential and source-only; source is frozen while reviewed or
while negative controls run. Cross-family model coverage is unavailable here.

## Verification record

The DALI integration regression first received a `refused`/cut-off reply after a
successful rename. Dedicated shared tests now cover exact active capability,
failed-CAS reuse, one successful use, retained unused capability versus an
identical later command, validation/read/helper scope absence, original MQTT
reply token/correlation, queued retirement, and stop before the owner's CAS.

The corrected kit suite passes 200 tests, including its wait registry and compiled
tutorial examples. The existing unfenced black-box conformance suite passes all
174 tests; its run used `RUST_LOG=info`. Strict all-target Clippy, three rustdoc
tests and the armv7 library compile passed. Four controls in
`mqtt-bridge-kit/docs/completion-controls.toml` each failed at their intended
assertions; the runner restored source and recorded the ledger. Tokio's declared
minimum is now 1.50, avoiding the existing OwnedNotified API build failure when
a downstream lock selected 1.41; the bridge integration also checks with 1.50.

Two sequential source reviewers approved the implementation. These checks use
ephemeral FakeBroker and local fault injection, not production MQTT, Pi runtime
or physical HAT hardware. This remains a local development extension; no upstream
publication, release or deployment is included. Repository reference/tutorial
documents are updated; remote Claude artifact mirrors are not edited here.
