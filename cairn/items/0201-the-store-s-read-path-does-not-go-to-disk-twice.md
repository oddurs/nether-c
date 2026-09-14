---
id: 201
title: The store's read path does not go to disk twice
type: feature
status: unmarked
milestone: quickening
created: 2026-09-13
updated: 2026-09-13
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
