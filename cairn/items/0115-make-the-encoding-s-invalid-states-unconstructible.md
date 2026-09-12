---
id: 115
title: Make the encoding's invalid states unconstructible
type: feature
status: unmarked
milestone: after
created: 2026-09-11
updated: 2026-09-12
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

## 2026-09-12

NOT NOW, and the item said so itself: 'worth doing when the surface stops moving; doing it now would mean rewriting it again'. Checked rather than assumed, and the encoding has not stopped moving.

spec/07-ledger.md changed four times in the last week and twice today: Answer and Refusal got tags (#88), and Node::Trace changed what it holds, which bumped the domain separator to v2 (#89). There are 185 construction sites for the types this would lock down. Doing that refactor on the week the format moved twice is the thing the item warned against.

Also moved out of The Core Calculus, and the reason is not that it was in the way. Its area is crates/nether-ledger and it was filed during ledger work; it is about the ledger's type safety, not about the IR or the depth checker. The Core Calculus is the IR, the checker, normalization, holes, fuel and the Orpheus rule — all of which are built. This belongs with the ledger, and the ledger is buried, so After the Burial is where it goes alongside the conformance suite.

The trigger to pick it up: the domain separator has not moved for a release, or a second implementation exists and 0105's conformance suite is holding the format still.

0114's validation in put closes the hole meanwhile. It is a check at the door rather than a door that cannot open, and that is the right amount of safety for a format that is still being written.
