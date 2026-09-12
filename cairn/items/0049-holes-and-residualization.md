---
id: 49
title: Holes and residualization
type: feature
status: unmarked
milestone: calculus
depends_on:
- 41
- 48
created: 2026-09-10
updated: 2026-09-12
priority: p0
effort: l
area: crates/nether-core
stratum: '0'
proof: A program with one read produces exactly one hole, carrying its full dependency graph
---

What survives burial, and in what form. A hole carries its call, its stratum,
its source span and everything that depends on it.

## 2026-09-12

Burial residualises seal rather than folding it, because naming a value means encoding it and hashing it and nether-core does not depend on the ledger. Wiring the two together is this item: when a hole gets a Node it also gets a cairn, and seal of a known value becomes a literal Cairn at the same moment.
