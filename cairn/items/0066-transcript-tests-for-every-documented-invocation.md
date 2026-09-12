---
id: 66
title: Transcript tests for every documented invocation
type: chore
status: starved
milestone: rites
depends_on:
- 27
- 60
- 61
- 62
created: 2026-09-10
updated: 2026-09-12
priority: p0
effort: m
area: tests/transcripts/
proof: Every shell block in the spec and on the site runs and matches character for character
---

The stage 3 proof. Snapshot tests over real invocations. Documentation that
lies fails the build.

## 2026-09-12

STARVED behind 0061, which is starved on 0067 in The World.

Most of the transcripts in the specification are exhume and lamp against a trace that exhume produced. Until a hole can be answered they cannot be executed, only pinned — which is what tests/transcripts already does with 63 of its 66 samples.

One piece is available now and is worth doing when this unblocks: 0.7's transcript is bury then lamp, both of which work. Executing it needs the harness to set up a scratch store, write hello.nc, run bury, and carry the cairn from the first command into the second. That machinery is most of what every other transcript will need too.
