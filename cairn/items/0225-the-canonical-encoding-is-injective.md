---
id: 225
title: The canonical encoding is injective
type: chore
status: buried
milestone: assay
assignee: Oddur Sigurdsson
claimed: 2026-11-07
depends_on:
- 222
created: 2026-09-13
updated: 2026-11-07
priority: p1
effort: l
area: proof/
stratum: '0'
proof: Two distinct values are proved to have distinct encodings, so a collision is a blake3 collision and nothing else
---

7.1 requires one value, one encoding. Every claim in the specification about
names rests on it, and the argument for it is a table and some prose.

The proof obligation is small and worth having written down: the tag
disambiguates the shape, and within a shape the lengths disambiguate the parts.
If that is wrong anywhere, two different values share a name and nothing
downstream can tell.

## Delivery plan — 2026-09-15

### Starting point and scope

Encoding injectivity concerns valid values and canonical bytes, not proving BLAKE3 collision-free.

### Steps

1. Reuse 0222's approved environment and model all codec tags, lengths, signed values, recursive shapes and validity invariants.
2. Prove decode(encode(value)) returns valid value, deriving injectivity and separating stored-kind domains.
3. Compare model bytes with codec fixtures; identify invalid in-memory states excluded pending 0115.

### Acceptance and evidence

- [x] Distinct valid modeled values have distinct encodings without admitted lemmas. Hash collision resistance remains a cryptographic assumption.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

### Evidence — 2026-11-07

- Added `proof/encoding.py` (wired into `proof/check.py`) modeling all 11 `Value` tags (`0x00`..`0x08`, `0x10`, `0x11`), all 6 `Node` kinds (`0x20` + `0x00`..`0x05`), recursive unprefixed compound shapes (`Struct`, `Array`, `Answer::Given`, `Node::Literal`), and `DOMAIN = b"netherc/cairn/v4\x00"`, and proving:
  - `value_decode_encode`: `∀_a0.._a3. ValRoundTrip(_a0, _a1, _a2, _a3) → (_a2 = _a0 ∧ _a3 = _a1)` (prefix-free stream round-trip over all 11 `Value` tags and recursive sequences).
  - `node_decode_encode`: `∀_a0.._a2. NodeRoundTrip(_a0, _a1, _a2) → (_a2 = Pair(_a0, _a1))` (prefix-free stream round-trip over all 6 `Node` kinds, chaining `value_decode_encode` for `Node::Literal`).
  - `injectivity_from_left_inverse`: `∀x y. (decode(encode(x)) = x) → ((decode(encode(y)) = y) → ((encode(x) = encode(y)) → (x = y)))` (pure equational LCF derivation via `cong`, `sym`, `trans`, and `discharge`).
  - `cairn_preimage_injectivity`: `∀x y. (strip_domain(domain_prefix(encode(x))) = encode(x)) → ((strip_domain(domain_prefix(encode(y))) = encode(y)) → ((decode(encode(x)) = x) → ((decode(encode(y)) = y) → ((domain_prefix(encode(x)) = domain_prefix(encode(y))) → (x = y)))))`
- Cross-checked 16 canonical byte fixtures and the excluded invalid in-memory states (pending `0115`: `Value::Shade { origin > 8 }`, `Value::Struct { name: "" }`, `Node::Hole/Witness/Trace { stratum_or_depth > 8 }`, `Call { function: "" }`, `Span { start > end }`, and ` nesting > MAX_DEPTH`) in both `proof/encoding.py` and `crates/nether-ledger/src/codec.rs` (`model_fixtures_match_the_rust_encoder_and_excluded_in_memory_states_are_rejected`).
- Remaining limits: BLAKE3 collision resistance on `DOMAIN || encode(x)` remains a standard cryptographic assumption, and making invalid in-memory `Value`/`Node` states unrepresentable in Rust types is tracked in `0115`.
