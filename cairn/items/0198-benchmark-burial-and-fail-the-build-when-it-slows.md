---
id: 198
title: Benchmark burial, and fail the build when it slows
type: chore
status: unmarked
milestone: quickening
depends_on:
- 199
created: 2026-09-13
updated: 2026-09-13
priority: p0
effort: m
area: crates/nether-bury
stratum: '0'
proof: A commit that makes a fixed burial 10% slower fails CI, and the number is in the PR
---

## Problem

There is not one benchmark in this repository. The Decay Rule stops the trusted
core growing and nothing stops it getting slower, which is the other way a
project rots.

## Proposal

A fixed corpus of programs, timed, with the numbers committed. A regression
gate like `.decay-ceiling`: a file holds the budget, CI fails if a burial
exceeds it, and lowering it is an ordinary commit.

Measure work rather than wall time where possible — steps, allocations, bytes
encoded — because a wall clock says more about the machine than the code.
0044 already proves the shape-not-duration idea works.
