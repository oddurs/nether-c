---
id: 66
title: Transcript tests for every documented invocation
type: chore
status: buried
milestone: rites
assignee: Oddur Sigurdsson
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

## 2026-09-12

The piece this said was available now is done: 0158 gave the harness a scratch store, the specification's example programs, and a ledger already buried, and carries cairns across commands in a sample. Twelve samples execute. What is still waiting is the exhume half.

## 2026-09-12

Closed. Every shell block in the specification and on the site runs and matches character for character: fourteen of fourteen. The other fifty-three pinned samples are not transcripts -- a c fence is a program and an ebnf fence is a grammar -- and the summary now says that rather than claiming they wait on a rite.

## 2026-09-12

The last one was an error display with no command in it, so nothing could run it. Adding the invocation turned up that the front page had been showing stamp.nc:14:11 and look(reply).len for a program that says 14:21 and len(look(reply)).
