---
id: 58
title: Diagnostics with source spans
type: feature
status: unmarked
milestone: surface
depends_on:
- 57
created: 2026-09-10
updated: 2026-09-10
priority: p1
effort: l
area: crates/nether-syntax
stratum: '0'
proof: Every diagnostic names a span, and the depth errors name the line that caused the descent
---

Depth errors are the ones people will hit constantly and they are the hardest
to phrase. 'This is @3 because line 14 descended to disk' is the shape to aim
for — always name the cause, never just the symptom.
