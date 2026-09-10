---
id: 69
title: 'Strata 3 and 4: the disk'
type: feature
status: unmarked
milestone: world
depends_on:
- 67
created: 2026-09-10
updated: 2026-09-10
priority: p0
effort: l
area: crates/nether-world
stratum: '4'
proof: A build replayed from the ledger produces byte-identical output with the source files deleted
---

Reads become holes and every byte read is sealed. Writes are the first
genuinely irreversible thing the language can do, and the spec should sound
like it.
