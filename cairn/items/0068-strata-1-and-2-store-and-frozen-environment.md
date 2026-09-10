---
id: 68
title: 'Strata 1 and 2: store and frozen environment'
type: feature
status: unmarked
milestone: world
depends_on:
- 67
created: 2026-09-10
updated: 2026-09-10
priority: p1
effort: m
area: crates/nether-world
stratum: '2'
proof: Two burials with the same declared environment produce the same cairn
---

Reading the ledger, and the pinned environment: declared variables, a frozen
clock, the target triple. Undeclared reads are an error, not an empty string.
