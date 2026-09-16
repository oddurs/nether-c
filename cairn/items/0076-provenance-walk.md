---
id: 76
title: Provenance walk
type: feature
status: unmarked
milestone: necropolis
depends_on:
- 75
created: 2026-09-10
updated: 2026-09-15
priority: p0
effort: l
area: web/necropolis
stratum: '0'
proof: Any value walks back to the literal or the hole that produced it, one interaction per step
---

The thing that makes people understand the language. Not a debugger view — a
history.

## Delivery plan — 2026-09-15

### Starting point and scope

The graph currently exposes recorded references, not the history of every reduction. Start with web/necropolis/README.md, the graph decoder and spec/07-ledger.md §7.4; an incoming edge is not automatically a producing edge.

### Steps

1. Inventory which literal, call, answer, deposit and shade origins can actually be recovered from current objects.
2. Specify missing provenance before adding fields or changing canonical bytes; split that specification into its own PR if needed.
3. Add a labelled, keyboard-accessible one-step walk, explicit missing-origin states and browser fixtures for shared values and holes.

### Acceptance and evidence

- [ ] Each supported value reaches its recorded origin one interaction at a time, without fabricated causal edges or unwrapping shades. Record uncovered cases; the universal proof stays open until they are resolved.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
