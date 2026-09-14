---
id: 213
title: A store that outlives its machine
type: feature
status: unmarked
milestone: ossuary
created: 2026-09-13
updated: 2026-09-13
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
