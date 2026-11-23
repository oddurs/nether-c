---
id: 299
uid: 3bc7611f-2e9b-4fed-8607-32494b84a63e
title: Close proof kernel soundness gaps and gate/web regressions
type: feature
status: buried
milestone: assay
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

Closed all 8 proof kernel soundness exploits, fixed commit-msg and scripts/decay regressions, added WASM null-pointer checks, hardened site/bake URL filtering, and fixed attribution scan on main/tags.
