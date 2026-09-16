---
id: 225
title: The canonical encoding is injective
type: chore
status: unmarked
milestone: assay
depends_on:
- 222
created: 2026-09-13
updated: 2026-09-15
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

- [ ] Distinct valid modeled values have distinct encodings without admitted lemmas. Hash collision resistance remains a cryptographic assumption.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
