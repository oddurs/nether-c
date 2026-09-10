---
id: 55
title: The recursive descent parser
type: feature
status: unmarked
milestone: surface
depends_on:
- 18
- 54
created: 2026-09-10
updated: 2026-09-10
priority: p0
effort: l
area: crates/nether-syntax
stratum: '0'
proof: Every sample in the spec parses; malformed input produces one error, not a cascade
---

The technique is also the thesis. Written by hand from the EBNF in spec
section 04, with error recovery good enough that one mistake produces one
message.
