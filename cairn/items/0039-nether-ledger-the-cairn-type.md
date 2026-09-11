---
id: 39
title: 'nether-ledger: the Cairn type'
type: feature
status: buried
milestone: ledger
depends_on:
- 38
created: 2026-09-10
updated: 2026-09-11
priority: p0
effort: s
area: crates/nether-ledger
stratum: '0'
proof: Round-trips through Display and FromStr for ten thousand random values
---

A newtype over a blake3 digest, with a stable short form for human output and
a strict parser. Ordering, hashing, serde. The most-used type in the codebase,
so it is worth getting the ergonomics right first.

## 2026-09-11

Done. Newtype over a blake3 digest under the versioned domain separator. Strict FromStr: 64 lowercase hex characters, uppercase rejected rather than folded, because two spellings of one name is the beginning of two names. Display, Debug (short form), Ord and Hash. Round-trips for 256 generated values; a test asserts the domain separator is actually applied, since forgetting it would leave every cairn a bare blake3 and nobody would notice.
