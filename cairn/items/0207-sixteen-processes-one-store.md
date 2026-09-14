---
id: 207
title: Sixteen processes, one store
type: chore
status: unmarked
milestone: cortege
created: 2026-09-13
updated: 2026-09-13
priority: p1
effort: m
area: crates/nether-ledger
stratum: '4'
proof: Sixteen processes burying and exhuming against one store for a minute leave it consistent, checked by fsck
---

The store's concurrency tests use threads. Threads share a page cache and an
allocator; processes share neither, and the rename-based atomicity story is
about processes.

A minute of real contention, then a consistency check. This is the test that
would have caught the two ordering bugs review found in the store, and it
should exist before anybody runs a shared one.
