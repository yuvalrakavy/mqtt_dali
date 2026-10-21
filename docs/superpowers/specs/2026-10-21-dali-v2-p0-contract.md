# P0 configuration contract

This narrows section 4 of the approved v2 design for the first implementation slice.
The user authorized implementation on 2026-10-21. P1/P3 protocol decisions remain
gated separately; no legacy MQTT startup is exposed by the new v2 entry point.

- Worktree: `/Users/yuval/Documents/Projects/Home/mqtt_dali-v2`, branch `feat/dali-v2`.
- Public library module: `mqtt_dali::config`. `ConfigPaths` has separate `legacy` and
  `current` paths; defaults `dali.json` and `dali.v2.json`.
- v2 envelope: `schema_version: 2`, `revision: 1` initially, a 32-hex-character
  `inventory_epoch`, `name`, optional `description`, and `buses`.
- Every fresh file creation/import generates a new inventory epoch. It is an
  inventory reset marker, not a new public device identity. Deleting v2 resets the
  epoch. P3 must detect this reset rather than compare revisions across epochs.
- Bus fields: `bus` (UART index 0..3, independent of array order), `name`, optional
  description, `enabled` (true on migration), `outputs`, and `groups`. Runtime bus
  power/status is not persisted as desired configuration.
- Outputs have `short_address` 0..63, `name`, optional description. Groups have
  `group_address` 0..15, `name`, optional description and `members` (short addresses).
  Inputs/scenes and capability metadata will be added by their owning phases;
  P0 refuses unknown fields rather than silently discarding data on a later save.
- Names are preserved byte-for-byte, case-sensitive and unique among siblings in
  each namespace. Reject empty/whitespace-only names, control characters, `/`, and
  the path steps `.` and `..`. Output/group names may repeat across namespaces or
  buses. Unicode, internal spaces, dots and percent signs are preserved.
- Controller name must additionally fit BridgeKit's fixed-instance identifier
  rules; encode component names separately in P3. Do not normalize identifiers.
- Reject duplicate bus indices/names, output/group addresses/names, duplicate
  group members and references to absent outputs. Legacy `groups` may be omitted.
- Existing v2 is opened first. Only a genuine NotFound permits legacy fallback;
  invalid, denied, directory, symlink/dangling-symlink and unsupported-version v2
  are errors. Read regular files only and cap input and serialized migration size
  at 4 MiB. Open with O_NOFOLLOW/O_NONBLOCK and validate the opened descriptor;
  a final-component symlink/FIFO swap must neither be followed nor block the read.
- Reject v1/v2 paths that are lexically equal, resolve to the same existing file,
  or whose resolved parent+filename are equal. Reject symlink configuration inputs
  to keep atomic publication and later-save semantics unambiguous. Do not write v1.
- Parse/validate the complete legacy config before creating v2. Preserve names,
  array order, bus/address numbers and member order. Ignore only documented cached
  BusStatus; it never enables/disables a bus by inference. Initial capabilities
  remain unknown. Both files absent creates a minimal inventory with the supplied
  startup name (default `DaliController`) and no buses; no stdin/hardware/MQTT work.
- Serialize a complete valid v2 document into a unique, create_new, mode-0600
  temporary file in the destination directory. Flush/sync it, atomically hard-link
  it to the absent destination (no-replace semantics), remove the temporary name,
  and sync the directory. Hard-link failure other than AlreadyExists is an error;
  no unsafe overwriting fallback. On AlreadyExists, load and validate the winner.
  Unsupported filesystems report the limitation. Ignore orphan temporary files.
  Configuration directories must be controlled by the operator: hostile replacement
  of parent directories or temporary names is outside this file-loader contract.
  A directory sync failure after publication reports an error and may leave the
  complete new file visible; subsequent startup validates and uses it.
- Library filesystem functions are synchronous and explicitly require callers to
  run off async workers under a deadline. A temporary `mqtt_dali_v2 --check-config`
  development entry point calls the loader on a worker with a 3 s deadline and never starts
  hardware, TUI or legacy MQTT. Without --check-config it reports that the runtime
  is not wired yet. P3 will promote the new runtime to the canonical mqtt_dali
  binary; v1 stays on the maintenance branch.
  The check process detaches the worker rather than joining it. Output is best
  effort on a separate descriptor writer bounded at 200 ms, including error/help
  paths, so an undrained output pipe does not prevent exit.
- CLI path options: `--config` for v2, `--legacy-config` for import source;
  `--name` applies only when both files are absent. Existing/imported names win.
- Meaningful tests: preservation/default groups, v2 precedence even over corrupt
  v1, deletion/reimport, malformed/unsupported/unknown v2, invalid legacy references
  and namespaces, file-size/symlink/path-alias rejection, failed publication with
  unchanged legacy bytes, orphan temporary files, synchronized concurrent creators,
  mode 0600, and a subprocess with both files absent and stdin closed.
- Do not claim daemon-startup migration is complete until P3 actually calls this
  configuration boundary. Record the P0 library/check-mode acceptance separately.
