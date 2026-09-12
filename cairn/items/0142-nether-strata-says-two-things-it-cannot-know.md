---
id: 142
title: nether strata says two things it cannot know
type: bug
status: buried
milestone: rites
assignee: Oddur Sigurdsson
created: 2026-09-12
updated: 2026-09-12
priority: p1
effort: s
area: crates/nether-cli
stratum: '1'
proof: The report claims a lower bound only when a node is missing, and never collapses two different answers
---

## Two things it cannot know

**It claims a lower bound it has not established.** `survey` counts every
cairn `store.get` misses, but `Node::references()` yields values as well as
nodes — call arguments, witness answers, and every `span.source`. A source
blob that was never sealed into the ledger is the ordinary state of things, and
it makes the report say

```
  3 cairn(s) under this trace are not in this ledger, so this is a lower bound.
```

A missing source bounds nothing about which strata were reached. Only a missing
*node* does, and the only edges that carry nodes are `Trace.roots` and
`Hole.depends`, so the walk can be exact rather than apologetic.

`a_span_whose_source_is_gone_still_gives_the_offset` asserts the wrong
behaviour and has to change with it.

**Its repeat count hides the thing worth seeing.** The `×N` collapse keys on
(stratum, answered, span, call), which for a witness leaves out the *answer*.
Two witnesses of the same call at the same place that came back with different
answers collapse into one line marked `×2` — which is exactly the
non-determinism a blame tool exists to surface. Everywhere else the branch is
unreachable, because nodes are already deduplicated by cairn.

## What to do

Walk node edges only, and count `missing` over those. Delete the collapse.

## Acceptance criteria

- [x] A trace whose source is not in the ledger reports no lower bound
- [x] A trace missing a node under it does
- [x] Two witnesses with different answers are two lines

## 2026-09-12

The walk goes along Node::nodes now, which is new on the ledger and says which of the cairns a node holds are themselves nodes: a trace's roots and a hole's depends, and nothing else. Section 7.3 already implied it; this states it once where the node model is.

## 2026-09-12

The collapse is gone rather than fixed. A node is named by its content and the walk is keyed on that, so two entries were always two nodes -- the branch was unreachable except in the one case where it was harmful.
