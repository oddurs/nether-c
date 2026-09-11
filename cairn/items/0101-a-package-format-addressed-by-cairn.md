---
id: 101
title: A package format, addressed by cairn
type: feature
status: unmarked
milestone: after
depends_on:
- 42
created: 2026-09-11
updated: 2026-09-11
priority: p1
effort: xl
area: crates/nether-ledger
stratum: '1'
proof: Two machines that have never spoken resolve the same dependency to the same cairn, offline
---

## Problem

The roadmap ends where the compiler works, which is not where a language starts
mattering. Nobody adopts a language they cannot share code in.

## Proposal

There is no registry and there does not need to be one. A package is a cairn. A
dependency is a cairn. Resolution is a lookup in a content-addressed store, and
two people who have never spoken resolve the same name to the same bytes
because the name *is* the bytes.

This is the part of the design that was free all along and it should be claimed
deliberately rather than discovered.

## Open questions

- [ ] How does a human name a package they have not seen? Cairns are not memorable
- [ ] Version ranges have no meaning when a dependency is a hash. Is that a feature?
- [ ] Who hosts the store, and what happens when they stop (see 'A public store')
