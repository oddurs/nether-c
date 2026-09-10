---
id: 44
title: Measure dedup on two near-identical traces
type: chore
status: unmarked
milestone: ledger
depends_on:
- 43
created: 2026-09-10
updated: 2026-09-10
priority: p0
effort: s
area: crates/nether-ledger
proof: The dedup ratio is measured, recorded, and re-checked in CI so it cannot silently regress
---

Bury the same program twice with a one-line change. The fraction of shared
nodes is the entire argument for content addressing; if it is not high, the
node model is wrong and we would rather find out now.
