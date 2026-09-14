---
id: 212
title: What a prune may remove, and what it may never
type: spec
status: unmarked
milestone: ossuary
depends_on:
- 210
created: 2026-09-13
updated: 2026-09-13
priority: p0
effort: m
area: spec/07-ledger.md
stratum: '4'
proof: Section 7.6 names exactly what an operator-invoked prune may remove, and a witness is provably not in that set
---

7.6 permits an explicit operator-invoked prune with a stated reachability root,
and forbids pruning witnesses at all — because a witness is the only copy of
something the world said once and may never say again.

What it does not do is say how an operator states a root, what reachability
means across traces that reference each other, or what a store is allowed to
claim afterwards. A prune that silently breaks a provenance chain is worse than
a store that grew.
