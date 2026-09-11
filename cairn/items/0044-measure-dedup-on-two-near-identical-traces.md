---
id: 44
title: Measure dedup on two near-identical traces
type: chore
status: buried
milestone: ledger
depends_on:
- 43
created: 2026-09-10
updated: 2026-09-11
priority: p0
effort: s
area: crates/nether-ledger
proof: The dedup ratio is measured, recorded, and re-checked in CI so it cannot silently regress
---

Bury the same program twice with a one-line change. The fraction of shared
nodes is the entire argument for content addressing; if it is not high, the
node model is wrong and we would rather find out now.

## 2026-09-11

MEASURED: 99.93%. 40,958 nodes over 8,192 sources; editing one source renames 28 nodes and reuses 40,930. The test asserts a floor of 99% and fewer than 64 changed nodes, so a regression fails CI rather than being noticed later.

The first version of this measured 50% and the node model was not at fault — the generator was. It chained every node to its predecessor, so one change renamed everything downstream. A build graph is a tree: a leaf edit reaches its own nodes and its ancestors and nothing else, which is 2 nodes per level over 13 levels plus the leaf. Worth recording because a proof that measures its own harness is worse than no proof.
