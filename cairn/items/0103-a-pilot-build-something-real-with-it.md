---
id: 103
title: 'A pilot: build something real with it'
type: chore
status: unmarked
milestone: after
depends_on:
- 82
created: 2026-09-11
updated: 2026-09-11
priority: p0
effort: xl
area: lib/
stratum: '4'
proof: Nether C builds this repository's own site, end to end, and the result is byte-identical to site/bake's
---

Every language demo is a fibonacci function and every language demo is a lie.

The honest test is to take something that already works, rebuild it in Nether
C, and see what breaks. `site/bake` is the right target: it reads a directory,
transforms text, writes files, and its output is already checked byte-for-byte
in CI. If Nether C cannot express it, that is worth knowing before anyone else
finds out.

Expect this to produce more bug reports than any other item on the roadmap.
That is the point of it.
