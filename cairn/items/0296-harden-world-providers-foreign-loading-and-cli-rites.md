---
id: 296
uid: 8bec3025-f507-49d6-bfc7-e711a4b07e66
title: Harden world providers, foreign loading, and CLI rites
type: feature
status: buried
milestone: warden
assignee: Oddur Sigurdsson
created: 2026-11-23
updated: 2026-11-23
closed_at: 2026-11-23
priority: p2
stratum: '8'
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

Hardened disk symlink containment and bounded reads, HTTP/1.1 chunked decoding and deadlines, foreign RTLD_LOCAL on macOS, replay verification, and CLI rite error handling.
