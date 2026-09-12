---
id: 60
title: nether bury
type: feature
status: unmarked
milestone: rites
depends_on:
- 22
- 49
- 128
created: 2026-09-10
updated: 2026-09-12
priority: p0
effort: m
area: crates/nether-cli
stratum: '4'
proof: Reports depth, hole count and node count on every burial, and writes exactly one trace
---

The primary verb. Everything a burial learns goes into the ledger; the
terminal output is a summary of what was written, never the thing itself.

## 2026-09-12

Blocked on 0128 rather than on anything here. Every part of the pipeline exists — lex, parse, lower, check, bury, and a store — and what is missing is what a trace's roots are. Section 7.3 says a Trace holds what was demanded, and a demand that did not reduce has no node to point at, because section 6.5 calls the residue nodes and section 7.3 has no node that can hold one. Section 6.6's own summary line counts 903 nodes for a program with one hole, which is the residue being counted, so the two readings cannot both be right and the number in the specification assumes the one section 7.3 cannot express. Inventing a trace identity here would be implementing something the specification does not describe.
