---
id: 302
uid: 8dc9ddb0-a7a2-4010-bff5-349b4a364522
title: Harden burial rollback, residuals, and ledger store imports
type: feature
status: buried
milestone: warden
assignee: Oddur Sigurdsson
created: 2026-11-23
updated: 2026-11-23
closed_at: 2026-11-23
priority: p2
stratum: '1'
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

Fixed Pass-2 burial soundness and ledger store integrity issues: checkpoint rollback now restores globals and drops rolled-back Deposit nodes; global evaluation preserves starved act globals as references instead of duplicating side effects; residualization avoids double-wrapping Shades and preserves answered deep Shade origins; assign evaluates index paths before values, computes accurate residual depth, and records Answered/Refused updates in settling; lowers tracks loop and function exit control-dependence depths; walk_writes uses a HashMap with a recursion cap; Prim::Concat enforces a 64 MiB cap; Store::import_entry validates topological references before writing objects; Store::resolve filters bucket directories; Package::load_pkg checks domain version before field shape.
