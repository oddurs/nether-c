---
id: 53
title: 'Proof: one read, one hole'
type: chore
status: unmarked
milestone: calculus
depends_on:
- 49
created: 2026-09-10
updated: 2026-09-10
priority: p0
effort: s
area: crates/nether-core
stratum: '3'
proof: The stage 1 proof runs in CI on every push
---

build.nc from the spec buries to depth 3 with exactly one hole and 900-odd
nodes. Pin the shape, not the exact count, so ordinary refactors do not fail it.
