# Read-only DALI identity and memory banks

Tier T2: native Part102/103 read plans, optional emulator profiles and public
read commands. This implements approved architecture §8's identity/memory family;
the owner authorized continued implementation without routine approvals. No new
product decision is required. No Store, persisted bridge identity or v1 changes.

## Purpose, alternatives and grounding

Operators need product/serial/version information and a bounded diagnostic read
of supported memory banks. The same native foundation will support Parts251–253.
Current discovery records types/topology, not memory bytes. Legacy constants name
READ MEMORY LOCATION but managed v2 has no memory work/commands/model. Existing
OutputSettings and Scenes provide explicit owned read/report patterns; physical
Work already carries one permit, deadline, stamp and lifecycle acceptance.

Choose an explicit read-only Memory Work under that owner, with raw observations
and a bank0 decoder. Rejected: add reads to every brightness poll (unnecessary bus
traffic); run a separate memory worker (duplicates UART/DTR ownership); store
observed serial numbers in saved identity (changes approved name/Origin semantics).
Use command replies/protected read reports, like fixture settings. Retained
capability metadata is a separate remaining slice; do not add another observation
cache or silently change the scalar State contract here.

## Native and public contracts

Add pure dali::memory with checked short-address target (Output or Input module),
bank byte and Window(start,count): start0..254, count1..64, inclusive end<=254.
No group/broadcast/instance memory target. Use typed DaliFrame widths, no I/O.
Part102 selects bank via C3xx and offset via A3xx, query addressed C5; version97.
Part103 uses C131xx/C130xx, addressed FExx queries3C/34. Confirm literal boundary
vectors independently of emulator decoding. No repeated or memory-write command.

Select DTR1 once, explicitly reload DTR0 for every requested location and read it
under one owned operation. This costs one extra scratch send per byte but avoids
depending on increment behavior after missing replies. Preserve byte255 as data;
silence, receive/transmit collision and unvisited values remain distinct. Do not
reuse a previous byte, pack zeros into holes or treat silence as a positive
unsupported/presence response. Reading can change scratch DTRs, never settings.

Commands on the ordinary Command lane:

- read_memory: strict {bank,start,count}; saved Output or Input module target.
- read_identity: no params, same target kinds. Query version and bank0 header,
  then read only the known common/selected layout up to the header's last address.

Both are Read-class, generation optional but supplied tokens must match. Capture
the current mapping/revision once; carry original RequestContext and trace to
reply. Both typed service and authenticated jobs use the same parser/owned path.
Fifteen-second absolute operation cap within existing SDK20s; no per-byte renewal,
retry, background poller or queued replay. Reads need no repeat-terminal profile.

Report: operation/outcome, target/target_generation, revision, bus/address/space,
bank/start/count, quality, ordered readings including raw byte/address, nullable
decoded identity/version, reason and configuration_changed:false. Whole requested
range stays explicit after missing/collided replies; transport/deadline/stamp
aborts can fail the read using existing owner behavior rather than inventing an
accepted partial capture. Complete means all requested bytes answered, not proof
of semantic identity validity. Partial/unavailable never invents values.

Bank0 decoding: raw version1 selects legacy locations0..0E with 32-bit serial;
recognized modern encoding (major>=2, excludingFF) selects through1A with 64-bit
serial/hardware version. Unknown version exposes only common GTIN/firmware bytes,
with unknown layout and null serial/hardware. Truncated/malformed headers and
missing/colliding locations remain diagnostic partial evidence. GTIN is a padded
decimal string only if representable as a14-digit GTIN; raw bytes remain visible.
Serial is fixed-width uppercase hex, never JSON floating-point. Do not claim
manufacturer lookup/check-digit validation or use GTIN/serial as persistent key.
All-FF fields are unavailable rather than fictional versions/identities; preserve
their original bytes. Firmware/hardware versions are separate major/minor values.
No universal checksum interpretation for bank0 byte1.

## Independent emulator and operator UI

Add optional memory profile on Output/Input modules: version(default8), banks
with unique byte bank IDs and nonempty up-to255 nullable-byte arrays. Bound to16
banks per fixture; null locations produce no reply and FF stays numeric. Header
bytes are explicit scenario data, including malformed/truncated cases for tests.
Do not auto-rewrite raw memory or infer identity from emulator id/name/address.
No profile preserves existing scenario bytes and memory queries are unanswered.
Native READ increments DTR0 up toFF for implemented banks; absent banks do not
execute. Model literal opcodes independently of bridge codec. Profile survives
capture/export/reboot/reset and addressing, unchanged by reads. Existing generic
scenario JSON editor can configure it; document a working identity profile.

Add Device information to output and input-module action menus. One protected
read-only dialog shows captured product/serial/firmware/hardware, layout/quality
and explicit Refresh identity. An advanced Memory section selects bank/start/count
and shows exact bytes and missing/collision evidence. Use current responsive Modal
and frozen original target/generation/revision scope; navigation/removal/rename or
generation retirement cancels pending view authority. A read failure clears old
current report. Reports are labeled captured reads, not live scalar State. No
editable hardware identity fields, automatic rename or touch-panel work.

## Verification and implementation phases

1. Fail-first independent scenario/native memory behavior, checked codec literals
   and pure decoder tests. Cover output/input address0/63, banks0/255, offsets0/FE,
   count1/64, overflow/invalid types, legacy/modern/unknown version, FF/zero bytes,
   exact64-bit serial, truncated header, holes/collisions and unchanged old profiles.
2. Implement owned Memory request/run/accept/finish using existing Physical Work,
   Hat and owner pattern. Require original generation/revision, Ready, stamp and
   deadline at acceptance. Memory reads do not cut group/output/feedback caches.
   Strict native/MQTT/HTTP tests prove same address in both spaces is distinct,
   missing/collided replies cannot shift subsequent bytes, both terminal profiles,
   headerless/correlated traces, retired mapping and native/config preservation.
   Mid-checkpoint/self-review before adding UI.
3. Protected dialog with focused frontend authority/window/format tests and real
   phone/desktop Chrome against the independent TCP emulator. Show identity and
   raw bytes, failures/unknowns, menus/scroll, refresh, navigation retirement and
   unchanged fixture/config state. Existing full workflows remain valid.
4. Documentation/skills phase: identity/memory reference and operator guide with
   scenario example, MQTT/service pointers, CLAUDE, support matrix and progress.
   No Store skills or user memory changes are required for this bridge capability.
5. Scoped mutation tests (space/offset framing, missing-byte reuse/layout/serial
   decoding, final acceptance/window bounds), freeze/fresh source gate; final
   native/frontend/browser, strict Clippy, Pi, wait/format/assets/whitespace and
   affected control plans. Same-family/source-only review limits and physical
   validation remain explicit. Continue remaining bridge work after acceptance.

Self-review: optional profile/report only, no saved bridge schema or scalar State
change. Bounded reads reuse one physical owner, typed widths and existing acceptance.
Version-gated decoding handles the actual legacy difference; unknown evidence is
not capability support. No output/configuration writes, dependency bump or new
worker/cache/recovery policy. Public generic consumers need only MQTT command
replies. User-level ways-of-working and staged-design-review govern the T2 slice.

Sources (primary implementation/vendor references, verified2026-10-23):
[python-dali bank0 layout and legacy version](https://github.com/sde1000/python-dali/blob/master/dali/memory/info.py),
[Part102 commands](https://github.com/sde1000/python-dali/blob/master/dali/gear/general.py),
[Part103 commands](https://github.com/sde1000/python-dali/blob/master/dali/device/general.py),
[memory read semantics](https://github.com/sde1000/python-dali/blob/master/dali/memory/location.py),
[Infineon DALI2 stack, §5.2](https://community.infineon.com/gfawx74859/attachments/gfawx74859/XMC/12401/1/AP32400_DALI2_0_Control_Gear_Stack_v1.pdf),
[Beckhoff input memory reads/DTR ownership](https://infosys.beckhoff.com/content/1033/tcplclib_tc3_dali/7939838091.html).
