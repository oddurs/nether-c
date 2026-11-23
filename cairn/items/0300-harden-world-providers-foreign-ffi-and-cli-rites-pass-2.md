---
id: 300
uid: e99a5f10-b0a6-40be-a6c3-f5dcefe9aa09
title: Harden world providers, foreign FFI, and CLI rites (pass 2)
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

Bounded total symlink hops in Disk::resolve, stopped Disk::remove dereferencing final symlinks, hardened HTTP/1.1 framing and per-syscall deadlines, capped foreign FFI retry allocation, fixed graft witness/answer isolation, made exhume --replay read-only, propagated lamp provenance store errors, and switched memo writes to O_EXCL.
