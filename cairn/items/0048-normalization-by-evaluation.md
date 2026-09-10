---
id: 48
title: Normalization by evaluation
type: feature
status: unmarked
milestone: calculus
depends_on:
- 20
- 46
created: 2026-09-10
updated: 2026-09-10
priority: p0
effort: xl
area: crates/nether-core
stratum: '0'
proof: fib(30) burns to a literal; the residual contains no call
---

The partial evaluator. Pure code evaluates away entirely; anything that needs
the world residualises. This is burial.
