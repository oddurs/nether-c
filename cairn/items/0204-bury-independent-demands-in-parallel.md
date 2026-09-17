---
id: 204
title: Bury independent demands in parallel
type: feature
status: unmarked
milestone: cortege
depends_on:
- 209
created: 2026-09-13
updated: 2026-09-16
priority: p0
effort: l
area: crates/nether-bury
stratum: '0'
proof: A program with N independent demands buries on N cores and produces the same cairn as the serial burial
---

## Why this is easy here and hard elsewhere

Burial is pure. Two demands that share nothing can be evaluated at the same
time with no locking and no ordering question, because neither can observe the
other.

6.2 fixes the order demands are *evaluated in*, and that is the one thing to be
careful about: the order holes are discovered in is part of the trace. Parallel
burial has to produce the serial order, not whatever order the threads finished
in.

## Proof

The same cairn, not merely the same value. If the parallel trace differs by a
byte, the order leaked.

## Delivery plan — 2026-09-15

### Starting point and scope

Pure concurrency still has observable shared-binding, deposit, hole-span and fuel order. 0209 must settle the contract first.

### Steps

1. Identify independent subgraphs and shared work while preserving once-only evaluation.
2. Use bounded workers with serial fallback, buffering results for ordered commitment.
3. Compare serial/parallel bytes and diagnostics across shared dependencies, starvation, collapse and worker counts.

### Acceptance and evidence

- [ ] N independent demands actually use N available cores and match the serial cairn. Include concurrency evidence, not just elapsed speedup.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

## 2026-09-16

0258 settled the order question against this item: spec/06-evaluation.md §6.2 now specifies evaluation inside a demand as left to right and innermost first, because three sections were already resting on an order a fourth declined to fix -- the hole order in §7.3, the span a shared hole keeps in §6.3, and where a fuel budget runs out in §6.4. A parallel burial therefore has to charge fuel as though it had run sequentially, as well as discovering holes and keeping spans in the serial order. Its proof -- the same cairn as the serial burial -- is unchanged and is now harder to reach honestly, because a budget that binds has to cut in the same place.
