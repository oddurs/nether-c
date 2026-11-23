---
id: 298
uid: 630fc19d-5b9a-465a-955c-61181f0a6519
title: Harden bury, ledger, and wasm correctness
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

Fixed burial rollback, flow short-circuiting, control-dependence write depths, local mutation settling, ledger atomic writes, bundle topological ordering, and wasm exact-layout allocator and trace depth parity.
