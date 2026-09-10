---
id: 65
title: nether graft
type: feature
status: unmarked
milestone: rites
depends_on:
- 22
- 61
created: 2026-09-10
updated: 2026-09-10
priority: p1
effort: l
area: crates/nether-cli
stratum: '4'
proof: Changing one line re-buries only the affected subgraph, demonstrated by node count
---

Substitute a subtrace by cairn and re-bury only what changed. Incremental
rebuild with correct invalidation, which content addressing gives us for free
if the node model is right.
