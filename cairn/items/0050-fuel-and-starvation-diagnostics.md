---
id: 50
title: Fuel and starvation diagnostics
type: feature
status: buried
milestone: calculus
assignee: Oddur Sigurdsson
depends_on:
- 15
created: 2026-09-10
updated: 2026-09-12
priority: p0
effort: m
area: crates/nether-core
stratum: '0'
proof: A program that would unroll forever fails in bounded time naming the source span
---

Deterministic fuel accounting, and an error that tells you where evaluation
starved rather than where it happened to run out.

## 2026-09-12

The error names the loop rather than the leaf, which is the difference between where evaluation starved and where it happened to stop. A stack of open loops and calls is kept as evaluation goes, nothing pops it on the way out of an error, and the halt picks the frame that was going round the most. It costs a Copy push and pop per loop and per call and nothing else.

## 2026-09-12

Everything printable now becomes a Diagnostic first, so there is one renderer rather than one per kind of wrongness. Fault and Halt both make one. That is what let section 6.4's fuel error and section 1.6's Orpheus error come out of the same forty lines.

## 2026-09-12

Fuel does not bound depth, and a runaway recursion overflowed the host stack long before any budget ran out — a crash, which section 6.4 forbids. Fixed in two places: 0125 made the specification admit that an implementation has limits of its own, and burial now runs on a stack it chose rather than its caller's, with MAX_FRAMES 2048 stated beside STACK 64 MiB because neither number means anything alone.

## 2026-09-12

Filed 0126. Section 6.4 and section 9.9 both define starves, for two different things, and section 9.9 knows: its own note calls the other one a third and different thing. CLAUDE.md's table settles which usage keeps the word — starved means blocked — so it is the fatal one that needs a name.
