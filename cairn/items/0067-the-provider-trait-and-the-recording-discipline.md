---
id: 67
title: The provider trait and the recording discipline
type: feature
status: unmarked
milestone: world
depends_on:
- 12
created: 2026-09-10
updated: 2026-09-10
priority: p0
effort: l
area: crates/nether-world
stratum: '0'
proof: No provider can return a value it has not first written to the ledger; enforced by the type, not by review
---

One trait, one rule: an answer is written to the ledger before it is
returned. Make the recording structural rather than a convention somebody has
to remember, because somebody will not.
