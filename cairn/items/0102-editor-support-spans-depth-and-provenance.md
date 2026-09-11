---
id: 102
title: 'Editor support: spans, depth and provenance'
type: feature
status: unmarked
milestone: after
depends_on:
- 58
created: 2026-09-11
updated: 2026-09-11
priority: p1
effort: l
area: tools/
stratum: '3'
proof: Hovering a value in an editor shows its depth and walks its provenance, in a real editor, on a real trace
---

Depth is the language's whole point and it is invisible in a plain text buffer.

An editor that shows the depth of the expression under the cursor, and the
`descend` responsible for it, turns the central idea from something you reason
about into something you can see. The same is true of provenance: `nether lamp
--provenance` in a terminal is good; the same walk inline in the editor is
what makes people understand it.

Language Server Protocol is the obvious vehicle. The spans are already
load-bearing throughout the compiler because every diagnostic and every hole
points at source, so most of what a server needs will already exist.
