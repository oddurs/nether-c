---
id: 211
title: 'Secondary burial: move a thing without renaming it'
type: feature
status: unmarked
milestone: ossuary
depends_on:
- 210
created: 2026-09-13
updated: 2026-09-13
priority: p0
effort: l
area: crates/nether-ledger
stratum: '4'
proof: An object moved to cold storage resolves by the same cairn, and nothing above the store notices
---

## The idea the name comes from

An ossuary is a second burial. When the ground is needed, the bones are moved
somewhere smaller and they are still the same bones.

A cairn names content, not a location, so an object can move between tiers —
hot directory, packed archive, another machine — without anything that holds
the name being affected. That is a property the design has had since 7.2 and
has never used.

## Acceptance criteria

- [ ] A packed archive format for cold objects, specified before it is written
- [ ] `get` finds an object in any tier
- [ ] Moving an object is not observable above `Store`
