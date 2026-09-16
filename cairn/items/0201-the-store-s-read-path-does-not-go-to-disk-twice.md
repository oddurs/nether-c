---
id: 201
title: The store's read path does not go to disk twice
type: feature
status: unmarked
milestone: quickening
created: 2026-09-13
updated: 2026-09-15
priority: p1
effort: m
area: crates/nether-ledger
stratum: '1'
proof: Reading one cairn a thousand times touches the filesystem once, measured
---

`get` reads and decodes on every call. A burial that consults the store for
every value it might already have will do that constantly.

An in-process cache keyed by cairn, which is sound for free: the bytes behind a
cairn cannot change. That is the one caching problem content addressing makes
trivial, and it would be a waste not to take it.

## Delivery plan — 2026-09-15

### Starting point and scope

Store::get currently reads, hashes and decodes every call. Cache verified immutable content, not filename trust or permanent absence.

### Steps

1. Define per-store ownership, byte bounds and eviction; specify behavior after cached objects disappear or become corrupt on disk.
2. Add the smallest verified cache without changing canonical bytes or silently caching failures.
3. Count filesystem reads for 1,000 hits; test eviction, corrupt first reads and hostile replacement.

### Acceptance and evidence

- [ ] One retained object requires one filesystem read across 1,000 gets. Memory stays bounded and cached bytes always match their cairn.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
