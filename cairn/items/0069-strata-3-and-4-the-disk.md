---
id: 69
title: 'Strata 3 and 4: the disk'
type: feature
status: buried
milestone: world
assignee: Oddur Sigurdsson
depends_on:
- 67
created: 2026-09-10
updated: 2026-09-12
priority: p0
effort: l
area: crates/nether-world
stratum: '4'
proof: A build replayed from the ledger produces byte-identical output with the source files deleted
---

Reads become holes and every byte read is sealed. Writes are the first
genuinely irreversible thing the language can do, and the spec should sound
like it.

## 2026-09-12

Five functions, the refusals section 9.5 names for each, and two things that are facts about this implementation rather than the language: a root nothing may reach out of, resolved textually so a symbolic link cannot climb out of it, and a directory listing that is sorted because a filesystem's order is a fact about one machine and would not replay.

## 2026-09-12

holds() is by the lattice and not by the name: section 1.1 makes the strata a total order and DESCEND raises the ambient to what was granted, so descend disk! may read. A world granted disk! holds disk.

## 2026-09-12

The stated proof -- a build replayed from the ledger with the source files deleted -- needs a replay provider to serve the witnesses back. That is 0073, which is next, and it is where this proof runs.

## 2026-09-12

The proof runs here after all: a read is sealed before the program is told, so the answer outlives the file. The test reads, deletes the file, finds the answer unchanged in the ledger, and finds the world refusing it -- which is the half that makes the first half worth anything. Serving it back without touching the world is 0073.
