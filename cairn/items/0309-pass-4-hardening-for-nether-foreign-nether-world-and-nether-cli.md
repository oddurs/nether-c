---
id: 309
uid: d811bc4a-a595-4fd0-a575-1d318ad64800
title: Pass-4 hardening for nether-foreign, nether-world, and nether-cli
type: feature
status: buried
milestone: warden
created: 2026-11-24
updated: 2026-11-24
closed_at: 2026-11-24
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

## 2026-11-24

Hardened foreign shared-object loading, HTTP/1.1 framing and 101 handling, replay base-witness validation, and graft/memo/exhume/lamp/strata validation and exit codes (Pass-4 findings 31-40).
