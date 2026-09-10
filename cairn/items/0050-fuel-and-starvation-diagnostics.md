---
id: 50
title: Fuel and starvation diagnostics
type: feature
status: unmarked
milestone: calculus
depends_on:
- 15
created: 2026-09-10
updated: 2026-09-10
priority: p0
effort: m
area: crates/nether-core
stratum: '0'
proof: A program that would unroll forever fails in bounded time naming the source span
---

Deterministic fuel accounting, and an error that tells you where evaluation
starved rather than where it happened to run out.
