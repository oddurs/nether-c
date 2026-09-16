---
id: 200
title: Burying twice does no work the second time
type: feature
status: unmarked
milestone: quickening
created: 2026-09-13
updated: 2026-09-15
priority: p0
effort: l
area: crates/nether-bury
stratum: '1'
proof: Burying an unchanged program against a warm store spends zero fuel and produces the same cairn
---

## Why this is the whole point

Every value already exists at its content address. A burial that has been done
once has named everything it produced, and a second burial of the same source
should find all of it and evaluate nothing.

That is not an optimisation. It is the thing content addressing was for, and
0044 measured 99.93% node reuse for a one-line change — which is the same
property seen from the other side.

## What it needs

Burial to consult the store before evaluating, which means burial holds
stratum 1, which means `bury` grants it by default and says so.

## Watch out for

Fuel. A burial that evaluates nothing spends no fuel, and 8.2 reports what was
spent — so the number becomes evidence rather than trivia.

## Delivery plan — 2026-09-15

### Starting point and scope

Content addressing alone does not supply a sound burial cache key. Zero warm-cache fuel must be reconciled with deterministic accounting in §6.4 before implementation.

### Steps

1. Specify keys including source, semantics/version, grants, recorded answers and relevant limits; separate lookup from evaluation fuel.
2. Define behavior for smaller budgets, changed answers and missing/corrupt closure objects.
3. Implement bounded verified lookup and invalidation tests; retain fresh world queries unless replay is requested.

### Acceptance and evidence

- [ ] A supported unchanged burial spends zero evaluation fuel and returns the same cairn; changed inputs cannot reuse stale results. Resolve the fuel/spec conflict in a separate specification PR first.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
