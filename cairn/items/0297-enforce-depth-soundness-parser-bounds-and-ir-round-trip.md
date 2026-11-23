---
id: 297
uid: eac43758-576d-4fb9-a59e-84dbe1e0a82c
title: Enforce depth soundness, parser bounds, and IR round-trip
type: feature
status: buried
milestone: calculus
assignee: Oddur Sigurdsson
created: 2026-11-23
updated: 2026-11-23
closed_at: 2026-11-23
priority: p2
stratum: '0'
---

## Problem

## Proposal

## Starting point and non-goals

Name existing implementation/tests; distinguish missing work from work already delivered.

## Delivery steps and dependencies

1. Resolve specification changes first, in their own PR.
2. Implement the smallest slice; name the paths and prerequisite items.
3. Verify success, failure and boundary behavior.

## Which stratum does this reach?

## Acceptance criteria

- [ ]

## Evidence to close

Set `proof` to the observable gate. Record the tested commit, exact checks and
remaining limits here. Human or elapsed-time evidence cannot be replaced by CI.

## 2026-11-23

Fixed Held::without, implicit-flow control dependence, lazy cycle-safe blame, deferred Declare assertions, Look type check, opaque/descend/block freezing, Prim::refusable, report gutters, lexer CR/number/escape bugs, parser nesting/>> /trailing-comma bugs, lowering duplicate/cycle/Shade/landing/place bugs, and printer round-trips.
