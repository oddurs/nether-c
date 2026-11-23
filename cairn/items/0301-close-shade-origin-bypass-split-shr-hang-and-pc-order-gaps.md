---
id: 301
uid: fbfd9498-55dc-448c-a4cf-d07bf5c33391
title: Close Shade origin bypass, split-shr hang, and pc/order gaps
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

Enforced Shade<T@d> origin equality across every value boundary in nether-core and nether-syntax, fixed split-shr parser hang/span, initialized recursive latent depth from Held::NONE, aligned Assign/Call evaluation order and sequential exit-pc threading, recursed name_local into Select, hoisted compound-assign indices, validated prelude arity, and fixed print::as_block.
