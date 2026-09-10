---
id: 57
title: Depth inference
type: feature
status: unmarked
milestone: surface
depends_on:
- 47
- 56
created: 2026-09-10
updated: 2026-09-10
priority: p0
effort: l
area: crates/nether-syntax
stratum: '0'
proof: Every sample program in the spec compiles with all depth annotations deleted
---

The ergonomics risk, addressed directly. If people must write `@3`
everywhere they will leave, so the test is that the annotations can all be
removed and the program still checks.
