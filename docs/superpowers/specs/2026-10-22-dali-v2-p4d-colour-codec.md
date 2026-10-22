# P4d1: typed DT8 colour protocol

## Purpose and chosen boundary

The approved design requires Tc, xy and RGBWAF at one DALI address, alongside
ordinary independent RGB outputs/groups. Current `dali::gear` provides only
Part 102 brightness and base queries. Add a pure Part 209 codec beside it before
integrating hardware capability discovery, readback and MQTT. This is a T2
protocol surface within the T3 migration. Standing implementation authorization
applies; there is no new owner decision. Budget: one small codec/test batch and
two sequential source-grounded design/source reviews; no SDK or UART changes.

Alternatives: JSON-to-frame code in the proxy would couple representation and
transport; an all-capabilities generic command framework would expand scope.
Typed values and explicit frame plans match the existing pure output/input
codecs and allow literal independent tests. The emulator will implement its own
semantics in the subsequent integration slice, without importing this codec.

## Contract and implementation

- Add `src/dali/colour.rs` and tests; export it from `src/dali.rs`. Reuse validated
  output short/group destinations, exposing the existing command-frame builder
  crate-wide. Writes accept short/group/broadcast; reads accept short only.
- `Temperature` accepts 1..65534 mirek; Kelvin conversion is reciprocal, rounded
  to nearest mirek and rejected outside the representable range. `Coordinate`
  accepts 0..65534, with unit 1/65536. Float conversions reject nonfinite and
  out-of-range values before rounding. RGBWAF channels accept 0..254, normalized
  0..1, independently of master brightness. Zero is a valid channel value.
- MASK is explicit: absent RGBWAF write channels emit 255; an all-absent intent
  is rejected. No implicit RGB-to-xy conversion, white-channel zeroing, power
  change, or RGBWAF control-mode change. Tc/xy normal values cannot contain MASK.
- Complete write plans load DTR0/1 little-endian for words, DTR0/1/2 for channel
  triples. Emit `ENABLE DEVICE TYPE 8` immediately before each extended command,
  including final ACTIVATE. Ordinary 16-bit frames only; no repeat-t command.
  The future executor must hold its existing exclusive transaction permit for
  the entire plan because DTR/device-type selection is shared bus state.
- Query plans identify send/query steps explicitly. Byte queries cover features,
  status, gear features, RGBWAF control and extended version. Value selectors cover
  actual x/y/Tc, RGBWAF levels/control and four Tc limits, with explicit width.
  A value read first queries actual level to refresh colour reporting, selects
  DTR0, enables DT8 and queries value; word reads then query DTR0 for the low byte.
  Byte reads do not invent a second byte. Word assembly is MSB response + LSB
  DTR0; only full 65535 is MASK, so high byte 255 alone remains usable data.
  No response/collision remains transport failure, never a valid byte/MASK.
- Preserve all raw feature/status bits. Features expose xy/Tc, primary-N count,
  RGBWAF channel count, and whether counts are within 0..6. Reserved 7 is not
  advertised as seven supported channels. Status exposes range/calibration bits
  and a single active colour type only when exactly one active-type bit is set;
  zero/multiple bits remain unknown. Primary-N is reported, not controlled here.
- Colour plans do not establish physical state, discovery, persistence, MQTT
  properties or capability support. Unknown/missing representations remain
  unavailable in later integration. Split RGB composition remains Home-owned.

## Verification and documentation

Fail-first literal frame tests cover Tc/xy/RGBWAF, full MASK boundaries,
normalization/reciprocal conversion, destination edges, per-command enable,
query widths/order, byte response failures and raw feature/status fidelity.
Behavioral guard-removal controls must fail the intended assertions. Run the
full native suite, scoped Clippy, Pi compile, format/wait/whitespace checks after
restoration. Update protocol guide, support matrix, contributor catalog and
progress; no shared skill contract changes are needed for a pure local codec.

Primary references (read, not copied): python-dali commit
`4b21c18e79d02d2a97f91d135f0eee450e8c2fb9`, `dali/gear/colour.py` and
`general.py`; BACMOVE manufacturer DALION commissioning documentation for
1/65536 xy units, 1..65534 mirek and 0..254 channels; Siemens RGBWAF guide for
unchanged channel MASK. Manufacturer limits remain device-specific.

## Self-review

Round 1 checked actual existing gear destinations, ATX 16-bit frames and primary
opcodes/selectors. Round 2 checked sibling byte/word queries, high-byte 255 versus
full MASK, invalid active-type combinations and absence of implicit channel-mode
or power changes. No new wait or service owner is introduced. This slice makes
no hardware validation claim; wire integration remains subsequent work.

## Design verification and implementation evidence

Both sequential read-only design gates approved with no material blocker. They
confirmed three distinct decoding cases: bitmap FF is raw, byte colour-value FF
is MASK, and word FF00 is known while FFFF is MASK. Reviews used the same model
family and did not execute tests. The whole-plan permit remains a later executor
requirement, not an ownership mechanism supplied by the codec.

All contract items above are present in the codec, literal tests and protocol
guide. Three frame-plan tests failed against the empty API skeleton, then all
seven focused tests passed. Initial test-only generic/ownership compilation
errors were fixed before behavioral failing-first evidence. Ten behavioral
controls all failed for their named reasons (55 seconds). Restored full native
verification passes 331 tests, strict scoped Clippy, Pi library/all four binary
compilation, formatting, whitespace, wait registry and zero doctests. Sequential
source gates approved frozen `64c05f9` with no material finding after parent diff
review. Both were read-only, same-family and did not execute tests or controls;
neither physical validation nor whole-plan execution ownership is claimed.
The useful verification pattern is a complete literal plan, independent of the
production encoder or emulator, with explicit unchanged channels and byte widths.
