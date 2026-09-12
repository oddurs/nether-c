---
id: 65
title: nether graft
type: feature
status: buried
milestone: rites
assignee: Oddur Sigurdsson
depends_on:
- 22
- 61
created: 2026-09-10
updated: 2026-09-12
priority: p1
effort: l
area: crates/nether-cli
stratum: '4'
proof: Changing one line re-buries only the affected subgraph, demonstrated by node count
---

Substitute a subtrace by cairn and re-bury only what changed. Incremental
rebuild with correct invalidation, which content addressing gives us for free
if the node model is right.

## 2026-09-12

What a graft substitutes is a hole. A trace that has already been answered has the answer folded into its residue, so replacing it there would change nothing -- answering a hole differently is the operation a build tool wants, and it is what makes the reuse count mean anything.

## 2026-09-12

The proof is an equality: grafting a hole with an answer reaches exactly the trace that exhuming against a world which would have said the same thing reached, 4 reused 0 recomputed. Two routes, one name, nothing made twice. That is the whole argument for content addressing and it is now a test.

## 2026-09-12

Section 7.3.2's two derived facts now live in one place, crates/nether-cli/src/closing.rs. Three rites write a trace and working the depth out at each of the three is how each of them came to be wrong at least once -- graft made it three.
