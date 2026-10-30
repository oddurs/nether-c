---
id: 201
title: The store's read path does not go to disk twice
type: feature
status: buried
milestone: quickening
assignee: Oddur Sigurdsson
claimed: 2026-10-30
created: 2026-09-13
updated: 2026-10-30
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

- [x] One retained object requires one filesystem read across 1,000 gets. Memory stays bounded and cached bytes always match their cairn.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

## 2026-10-30

Buried. Store keeps what get has read, hashed and decoded, in a map shared by every clone of one store. Only objects that checked are kept; an absence or a corruption is asked about again, since either can be repaired. Once a value is kept, a later change to its file cannot reach that reader: the name is the content and the content was checked. The bound is 64 MiB of encoded bytes, and the eviction policy is starting again when an object would not fit. The lock is held to look and to keep, not across the disk read, so concurrent readers do not queue behind it. Store::disk_reads counts the reads, so the claim is measured. Tests in store.rs: a_thousand_gets_of_one_object_read_the_disk_once (disk_reads 1, a clone shares it), a_failure_is_asked_again_and_not_remembered (absent then present; corrupt then repaired), what_was_read_is_what_is_returned_whatever_the_disk_says_later, and what_is_kept_stays_inside_its_bound (two half-bound objects, bytes stay under the bound, the newest is kept). Limits: per process, and has() still asks the disk.
