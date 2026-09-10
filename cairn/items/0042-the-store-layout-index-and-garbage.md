---
id: 42
title: 'The store: layout, index, and garbage'
type: feature
status: unmarked
milestone: ledger
depends_on:
- 41
created: 2026-09-10
updated: 2026-09-10
priority: p1
effort: l
area: crates/nether-ledger
stratum: '4'
proof: Ten thousand puts and gets under concurrent access, with no torn objects
---

Content-addressed on-disk store. Atomic writes, an index for prefix lookup of
short cairns, and a stated position on garbage collection — even if that
position is 'never, and here is why that is affordable'.
