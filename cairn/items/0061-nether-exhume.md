---
id: 61
title: nether exhume
type: feature
status: buried
milestone: rites
assignee: Oddur Sigurdsson
depends_on:
- 22
- 60
- 67
- 163
created: 2026-09-10
updated: 2026-09-12
priority: p0
effort: l
area: crates/nether-cli
stratum: '5'
proof: Filling a hole produces a new cairn, and --replay reproduces it byte-identically
---

Grants a stratum, answers holes, records every answer, emits a deeper trace.
Never mutates the trace it was given — descent produces a new artifact.

## 2026-09-12

STARVED on 0067, which is in The World and not in this milestone.

8.3 says exhume 'grants capabilities, answers holes, records every answer'. Answering a hole means calling read() or get() for real, and doing it under the recording discipline 1.4 requires: the answer is written to the ledger BEFORE the program is told. That is 0067's provider trait, and there is no nether-world crate yet.

What could be built without it is --replay, which grants nothing and serves only from the ledger. That is the half of exhume with no provider in it, and it is worth noting that it is also the half that carries the language's central claim. It is not split out as its own item because a --replay that has never had anything to replay is not testable: something has to answer a hole first.

Dependency on 0067 recorded rather than left implicit, so the board stops offering this as ready work.

## 2026-09-12

Unstarved: 0067 landed the provider trait and the recording discipline, and 0069 landed the disk, so there is something that can answer a hole under the discipline section 1.4 requires.

## 2026-09-12

The grant half is built and proved: exhume answers a hole through 0067's discipline and 0069's disk, records the witness, re-buries the residue and seals the trace. Section 6.6's shape exactly -- sealed dd1289f4 + adf89b12 to 11356301, depth 3, holes 0.

## 2026-09-12

STARVED on 0163 for the other half of its proof. Replaying a sealed trace produces a trace identical in every field but fuel_spent: thirteen steps against one, because replay buries an already-folded residue. --replay refuses and says so rather than pretending.

## 2026-09-12

An answer is a value burial can fold through and never write down: section 04 has no syntax for one, so must, given and refusal consume it and an answer that is bound stays the question it was. That needed no change to the IR and no change to the language.

## 2026-09-12

Unstarved and closed: 0163 took fuel_spent out of the trace and --replay reproduces. Both halves of the proof hold -- filling a hole produces a new cairn, and replaying the sealed trace prints identical.
