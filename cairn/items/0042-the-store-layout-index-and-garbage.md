---
id: 42
title: 'The store: layout, index, and garbage'
type: feature
status: buried
milestone: ledger
depends_on:
- 41
created: 2026-09-10
updated: 2026-09-11
priority: p1
effort: l
area: crates/nether-ledger
stratum: '4'
proof: Ten thousand puts and gets under concurrent access, with no torn objects
---

Content-addressed on-disk store. Atomic writes, an index for prefix lookup of
short cairns, and a stated position on garbage collection — even if that
position is 'never, and here is why that is affordable'.

## 2026-09-11

Done. objects/ab/cdef, refs/ab/cdef, tmp/. The two-character fan-out IS the index 7.5 asks for: a short cairn of two or more characters reads one directory rather than scanning.

Two ordering decisions worth the review time.

Reverse edges are written BEFORE the object. Crashing between them leaves an edge pointing at an object that is not there, which referrers filters and a later put repairs. The other order leaves an object whose edges were never recorded and nothing afterwards can tell that provenance is now incomplete. A visible inconsistency beats a silent one.

get verifies that the bytes hash to the name asked for. In a content-addressed store bit rot is detectable for free, so not detecting it would be a choice, and a store that serves corrupt bytes quietly is worse than one that has lost them.

Two defects found and fixed before testing: resolve reported a well-formed but absent prefix as malformed, and claimed exactly 2 matches when it early-exits and only knows 'at least 2'. Both now say what is true.

No garbage collection, per 7.6. 12 store tests, including tampering caught, an edge outliving its object, and an ambiguous prefix refused rather than guessed.
