---
id: 207
title: Sixteen processes, one store
type: chore
status: unmarked
milestone: cortege
created: 2026-09-13
updated: 2026-09-15
priority: p1
effort: m
area: crates/nether-ledger
stratum: '4'
proof: Sixteen processes burying and exhuming against one store for a minute leave it consistent, checked by fsck
---

The store's concurrency tests use threads. Processes have separate address
spaces but still share the OS page cache. The rename-based atomicity story
must survive independent processes and interrupted writers too.

A minute of real contention, then a consistency check. This is the test that
would have caught the two ordering bugs review found in the store, and it
should exist before anybody runs a shared one.

## Delivery plan — 2026-09-15

### Starting point and scope

Store tests already use threads. Separate processes also share the OS page cache; the missing proof is process contention and interruption.

### Steps

1. Start sixteen child processes against one disposable store for at least sixty measured seconds.
2. Mix overlapping puts, reads and reverse-edge queries; capture seeds, exits and completed operations.
3. Use 0214's read-only checker or a specified equivalent after completion; separately exercise terminated writers.

### Acceptance and evidence

- [ ] Committed objects and reverse edges remain consistent after real contention. Timeouts, killed children and partial work cannot silently count as success.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
