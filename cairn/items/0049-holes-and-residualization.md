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
updated: 2026-09-10
priority: p0
effort: l
area: crates/nether-core
stratum: '0'
proof: A program with one read produces exactly one hole, carrying its full dependency graph
---

What survives burial, and in what form. A hole carries its call, its stratum,
its source span and everything that depends on it.
