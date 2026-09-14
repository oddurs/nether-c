---
id: 214
title: What a damaged store can still tell you
type: feature
status: unmarked
milestone: ossuary
created: 2026-09-13
updated: 2026-09-13
priority: p1
effort: m
area: crates/nether-ledger
stratum: '3'
proof: A store with N corrupted objects reports exactly those N, serves the rest, and says which traces are affected
---

`get` already catches a tampered object, one at a time, when something asks
for it. There is no way to ask the store how it is.

An fsck that walks every object, verifies its cairn, and — the part that
matters — reports which *traces* are now incomplete. A corrupt object is a fact
about bytes; a broken provenance chain is a fact somebody has to act on.
