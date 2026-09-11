---
id: 115
title: Make the encoding's invalid states unconstructible
type: feature
status: unmarked
milestone: calculus
created: 2026-09-11
updated: 2026-09-11
priority: p2
effort: m
area: crates/nether-ledger
stratum: '0'
proof: Every invariant in 7.1.1 is unrepresentable in the type, and put needs no validation pass
---

## Problem

`Value` and `Node` can express states the canonical encoding forbids: a struct
with no name, a stratum past the bottom of the lattice, a call with no
function, a span that ends before it starts.

0114 closed the hole by validating in `put`, which is the right fix for the
symptom — an object that could never be read back no longer reaches disk — but
it is a check at the door rather than a door that does not open.

## Proposal

Move the invariants into the types. Private fields with constructors that
return `Result`, so an empty struct name is a compile-time impossibility rather
than a runtime refusal, and `put` can drop its round trip.

## What it costs

Every construction site gets noisier, including the tests, which currently read
very plainly. Worth doing when the surface stops moving; doing it now would
mean rewriting it again once the calculus has opinions about what a value is.

## Acceptance criteria

- [ ] Every clause of 7.1.1 is unrepresentable rather than rejected
- [ ] `put` no longer decodes what it just encoded
- [ ] The decoder still rejects the same inputs, since bytes arrive from strangers
