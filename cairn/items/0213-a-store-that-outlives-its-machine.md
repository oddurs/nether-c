---
id: 213
title: A store that outlives its machine
type: feature
status: unmarked
milestone: ossuary
created: 2026-09-13
updated: 2026-09-15
priority: p1
effort: m
area: crates/nether-ledger
stratum: '4'
proof: A store exported, moved to another machine and imported answers every cairn the original did
---

The store is a directory today, which means moving it is `tar`. That works
and it is not a plan: it copies the reverse index, which is derived, and it has
no integrity check at the far end.

An export that carries only what cannot be re-derived, and an import that
rebuilds the rest and verifies as it goes.

## Delivery plan — 2026-09-15

### Starting point and scope

A directory copy is not a verified export/import contract. Transfer canonical objects and rebuild derived indexes.

### Steps

1. Specify manifest/archive, versions, snapshot consistency and interrupted import before code.
2. Implement bounded streaming verification, duplicate handling and index rebuild.
3. Move a store between machines with holes, witnesses and deposits; test corrupt/truncated imports.

### Acceptance and evidence

- [ ] Every original cairn resolves identically and replay works after import. Destination collisions cannot overwrite valid objects; same-machine round trips are preliminary.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
