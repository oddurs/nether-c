---
id: 211
title: 'Secondary burial: move a thing without renaming it'
type: feature
status: unmarked
milestone: ossuary
depends_on:
- 210
created: 2026-09-13
updated: 2026-09-15
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

## Delivery plan — 2026-09-15

### Starting point and scope

Scope the first cold tier to local packed storage; remote storage belongs to 0215. Wait for 0210's workload evidence.

### Steps

1. Specify archive/index format, verification and interrupted-move behavior before implementation.
2. Verify the cold copy before retiring the hot copy; preserve witnesses and restart safely.
3. Test tier lookup, corrupt archives, absent indexes and crash points against original bytes/cairns.

### Acceptance and evidence

- [ ] Every moved object resolves unchanged above Store. Packing is not pruning; never delete the last verified copy.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
