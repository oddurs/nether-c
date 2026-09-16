---
id: 202
title: Encoding allocates once
type: chore
status: unmarked
milestone: quickening
created: 2026-09-13
updated: 2026-09-15
priority: p2
effort: m
area: crates/nether-ledger
stratum: '0'
proof: Encoding a node of known size performs one allocation, asserted in a test
---

`encode` builds a `Vec` and grows it. The size is computable before the
first byte is written, and a million-node burial does this a million times.

Small, measurable, and the kind of thing that is easy while the encoder is
still four hundred lines.

## Delivery plan — 2026-09-15

### Starting point and scope

The codec has Value, Node and Stored encoding paths. Measure each before duplicating wire-format logic.

### Steps

1. Define allocation accounting around encoding only, excluding fixture construction.
2. Compute exact sizes with checked arithmetic and reserve once, sharing size/write rules where possible.
3. Cover every tag, nested/large values and overflow; compare canonical bytes with existing fixtures.

### Acceptance and evidence

- [ ] A nonempty known-size encoding allocates once without reallocating. Empty results need not allocate; report the extra sizing traversal cost.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
