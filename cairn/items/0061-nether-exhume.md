---
id: 61
title: nether exhume
type: feature
status: unmarked
milestone: rites
depends_on:
- 22
- 60
created: 2026-09-10
updated: 2026-09-10
priority: p0
effort: l
area: crates/nether-cli
stratum: '5'
proof: Filling a hole produces a new cairn, and --replay reproduces it byte-identically
---

Grants a stratum, answers holes, records every answer, emits a deeper trace.
Never mutates the trace it was given — descent produces a new artifact.
