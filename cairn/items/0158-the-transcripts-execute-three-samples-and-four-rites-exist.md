---
id: 158
title: The transcripts execute three samples and four rites exist
type: chore
status: buried
milestone: rites
assignee: Oddur Sigurdsson
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

- [x] A transcript of a rite that exists is executed
- [x] §8.2's cairn is checked rather than pinned
- [x] A rite that does not exist yet is still pinned, and the count says so

## 2026-09-12

Twelve executed, up from three, and it found five drifts on the first run: the front page still said build.nc buries to 903 nodes, which is the residue counted as nodes and the reading section 6.5 rejects. Section 00 and the README still said hello.nc was 17 nodes and 8f3a1c0e.

## 2026-09-12

Each checked sample gets a scratch directory holding the specification's example programs and a ledger that has already buried them, because that is what a reader following the document has done by the time they reach one. A sample can still hold a line for a rite that does not exist; those are skipped and the count says how many, so checked does not read as all of it was.
