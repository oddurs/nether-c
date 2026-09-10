---
id: 39
title: 'nether-ledger: the Cairn type'
type: feature
status: unmarked
milestone: ledger
depends_on:
- 38
created: 2026-09-10
updated: 2026-09-10
priority: p0
effort: s
area: crates/nether-ledger
stratum: '0'
proof: Round-trips through Display and FromStr for ten thousand random values
---

A newtype over a blake3 digest, with a stable short form for human output and
a strict parser. Ordering, hashing, serde. The most-used type in the codebase,
so it is worth getting the ergonomics right first.
