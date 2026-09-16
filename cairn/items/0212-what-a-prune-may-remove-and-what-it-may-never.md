---
id: 212
title: What a prune may remove, and what it may never
type: spec
status: unmarked
milestone: ossuary
depends_on:
- 210
created: 2026-09-13
updated: 2026-09-15
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

## Delivery plan — 2026-09-15

### Starting point and scope

§7.6 forbids witness pruning. This item specifies roots/reachability; it does not execute a prune.

### Steps

1. Enumerate edge types and permanent witness roots, including cycles, missing objects and cross-trace references.
2. Specify dry-run output, explicit operator roots, uncertain-reachability refusal and interruption guarantees.
3. Build graph fixtures demonstrating witness/provenance preservation and record rejected weaker policies.

### Acceptance and evidence

- [ ] The removal set is unambiguous and excludes all witnesses. No live deletion is authorized by this specification; implementation needs a separate item.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
