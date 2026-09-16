---
id: 215
title: A second store, and what they owe each other
type: spec
status: unmarked
milestone: ossuary
created: 2026-09-13
updated: 2026-09-15
priority: p2
effort: l
area: spec/07-ledger.md
stratum: '5'
proof: Two stores can be compared and reconciled by cairn alone, specified before it is built
---

Two people with two stores have a set-difference problem and nothing else: a
cairn is either there or it is not, and if it is, it is the same bytes.

This is the substrate 0101's package format needs and the thing a public store
serves. It should be specified once, here, rather than three times by whoever
needs it first.

## Delivery plan — 2026-09-15

### Starting point and scope

Cairns establish identity, not availability, authorization or witness agreement. Specify exchange rather than deploy a service.

### Steps

1. Define root closure, bounded comparison, missing/corrupt responses and version compatibility.
2. Specify receiver verification, interruption/resume and discovery privacy; distinguish read/fetch from active-send strata.
3. Write transcripts for equal, partial, divergent and malicious stores and record rejected formats.

### Acceptance and evidence

- [ ] Verified objects reconcile under a precise bounded contract. Possessing a cairn does not imply authentication, mutation authority or agreement about world answers.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
