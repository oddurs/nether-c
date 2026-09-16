---
id: 198
title: Benchmark burial, and fail the build when it slows
type: chore
status: unmarked
milestone: quickening
depends_on:
- 199
created: 2026-09-13
updated: 2026-09-15
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

## Delivery plan — 2026-09-15

### Starting point and scope

Interpreter step-budget tests already exist; a general performance gate does not. Use 0199's workload, separating deterministic work from machine time.

### Steps

1. Pin corpus, answers, release profile and runner class; measure baseline variance.
2. Define the 10% wall-time comparison and noise policy before gating; report steps, allocations and bytes too.
3. Add a scripts/task check and prove a deliberately slowed fixture fails while unchanged repeats pass.

### Acceptance and evidence

- [ ] A reproducible 10% regression fails and baseline/candidate numbers appear in the PR. Counters alone do not establish the wall-time claim.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
