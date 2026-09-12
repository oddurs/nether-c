---
id: 61
title: nether exhume
type: feature
status: starved
milestone: rites
depends_on:
- 22
- 60
- 67
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
