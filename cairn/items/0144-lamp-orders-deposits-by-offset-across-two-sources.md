---
id: 144
title: lamp orders deposits by offset across two sources
type: bug
status: unmarked
milestone: rites
created: 2026-09-12
updated: 2026-09-12
priority: p2
effort: s
area: crates/nether-cli
stratum: '1'
proof: Deposits from two sources come out grouped by source, in offset order within each
---

## The ordering

`lamp::deposits` sorts by `span.start` alone and ignores `span.source`.
[§8.4](../spec/08-rites.md) says deposits are rendered "in source order, which
is how a program's output is read", and *source order* is undefined once a
trace spans more than one source: two files interleave by byte offset, which
corresponds to nothing anybody wrote.

One source today, so it is right today. A `graft` makes it wrong.

## What to do

Sort by `(source, start)` so the output is grouped rather than interleaved, and
say in §8.4 that this is what "source order" means when there is more than one.

## Acceptance criteria

- [ ] Deposits from two sources are grouped by source
- [ ] §8.4 says what order means across sources
