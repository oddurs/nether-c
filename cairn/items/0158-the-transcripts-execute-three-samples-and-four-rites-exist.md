---
id: 158
title: The transcripts execute three samples and four rites exist
type: chore
status: unmarked
milestone: rites
created: 2026-09-12
updated: 2026-09-12
priority: p1
effort: m
area: tests/transcripts
stratum: '0'
proof: Every transcript whose rite exists is executed rather than hashed
---

## The promise

`tests/transcripts/run` says of its pinned samples:

> 63 describe behaviour that does not exist yet; each becomes executable the
> moment its rite does.

Four rites exist now — `bury`, `cairn`, `lamp`, `strata` — and the gate is one
regex that has not moved:

```python
RUNNABLE = re.compile(r"^nether\s+(run|--version|-V|--help|-h)\b")
```

So three samples are executed and the rest are hashes. §8.2's transcript is
exact — it reproduces down to `0380c8ae` — because somebody ran it and pasted
it. Nothing checks that it stays exact.

## What makes it harder than the regex

A transcript that names a cairn depends on a store. `nether bury build.nc`
prints a cairn that exists only once something has been buried, and §6.6's
`exhume` transcript depends on the trace from the one above it. So executing
these means giving a sample a scratch store and running its commands in order,
which is what a transcript already is.

## Acceptance criteria

- [ ] A transcript of a rite that exists is executed
- [ ] §8.2's cairn is checked rather than pinned
- [ ] A rite that does not exist yet is still pinned, and the count says so
