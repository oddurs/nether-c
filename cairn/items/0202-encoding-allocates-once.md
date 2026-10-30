---
id: 202
title: Encoding allocates once
type: chore
status: buried
milestone: quickening
assignee: Oddur Sigurdsson
claimed: 2026-10-30
created: 2026-09-13
updated: 2026-10-30
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

- [x] A nonempty known-size encoding allocates once without reallocating. Empty results need not allocate; report the extra sizing traversal cost.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

## 2026-10-30

Buried. One traversal writes both the size and the bytes: Value and Node encode into a Sink, which is either a Vec or a Count that keeps only how much it was given. encode runs it once into a Count, reserves exactly that, and runs it again into the Vec, so the size asked for is the size written by construction, not by a second function kept in step. A Vec reallocates only when it would outgrow its capacity, so an encoding that ends exactly at its capacity allocated once: every_value_is_encoded_into_exactly_what_it_reserved (every tag, a 4 MiB array, nesting to MAX_DEPTH) and every_node_is_encoded_into_exactly_what_it_reserved (every kind) assert capacity == len. The workspace forbids unsafe, so a counting allocator was not available; capacity == len is the same fact. The sizing pass costs a traversal and pays for itself: a million encodings of mixed traces and nested arrays took 133 ms against 359 ms before, release build, same 400831220 bytes out. A first draft sized with separate encoded_len functions; it was 12 lines longer and could drift from the encoder, so it lost. Core +29, to 8598.
