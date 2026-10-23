# Emulator control surface

The approved product is a separate simulated installation and browser, connected
to the bridge by raw ATX TCP. The native model remains authoritative. Rebuilding
it at each TCP accept was rejected because that loses physical state; sharing
bridge inventory was rejected because it defeats independent observation.

Implementation proceeds in three stages:

1. Keep one model actor alive with an optional replaceable connection. TCP accepts
   one client, emits a fresh Boot, and drops all connection output on disconnect.
   Existing generic/PTy sessions keep their EOF behavior. Add strict scenario
   controls, live structural edits preserving untouched devices, and explicit
   capture/reset. Validate before mutation. Reset retires TCP before processing
   any subsequent IO against replacement state; PTY reset drains old receive
   bytes and emits Boot after clearing partial input/output.
2. Add semantic physical stimuli interpreted by the existing native instance
   settings and monotonic model clock. Keep raw injection explicitly advanced.
   Document limits rather than inventing unsupported device behavior.
3. Serve separate embedded Preact assets from an explicitly enabled loopback
   listener. Reject non-loopback web binds; check Host and exact Origin for
   mutations plus a process-random CSRF token obtained only same-origin. No
   browser-supplied paths. Poll bounded snapshots with disconnection feedback.
   Draft edits are local; Apply starting scenario explicitly resets installation.
   Live add/remove uses distinct operations; capture is an explicit action.

Verification covers literal raw TCP fragmented/coalesced commands, competing
clients, reconnect retention and no output replay, invalid scenario atomicity,
capture/reset distinction, unchanged PTY/socket compatibility, semantic input
enable/filter/scheme/timing, HTTP foreign-origin/host rejection, real desktop and
phone browser workflows, asset reproduction, Clippy and Pi compilation.

Shared-file needs: wait-registry rows for TCP accept and HTTP owned service.
No Cargo dependency or public library module changes are expected.
