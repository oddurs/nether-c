---
id: 52
title: 'Property test: depth is monotone'
type: chore
status: unmarked
milestone: calculus
depends_on:
- 47
created: 2026-09-10
updated: 2026-09-10
priority: p1
effort: m
area: crates/nether-core
stratum: '0'
proof: Ten million generated programs, no counterexample to depth monotonicity
---

Generate arbitrary well-typed programs; assert that no evaluation step ever
lowers the depth of a value, and that application depth is exactly the max of
its parts. The central invariant deserves a generative test, not examples.
