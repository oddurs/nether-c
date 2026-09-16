---
id: 208
title: Two machines, one trace
type: feature
status: unmarked
milestone: cortege
depends_on:
- 215
created: 2026-09-13
updated: 2026-09-15
priority: p2
effort: xl
area: crates/nether-world
stratum: '6'
proof: Two machines each answer half a trace's holes, and the sealed result is byte-identical to one machine answering all of them
---

## The idea

A trace is a set of questions and a residue. Nothing says one machine has to
answer all of them.

Content addressing verifies the objects exchanged; it does not make merging
answers trivial. Two machines can observe different answers to the same
question. Assignment, conflicting witnesses and deterministic merge order
must be specified before distribution.

## Why it is xl anyway

Moving objects between stores is a protocol, and a protocol is a format, and
this project freezes formats carefully. It should not be invented in a hurry.

## Delivery plan — 2026-09-15

### Starting point and scope

Machines can answer the same question differently. Content addressing names disagreement; it does not resolve it.

### Steps

1. On top of 0215, specify assignment, answer identity, duplicate delivery, conflicts and merge order in a separate PR.
2. Use two isolated workers with deterministic recorded answers and reject conflicting witnesses.
3. Exercise retries, missing objects and interrupted transfer; replay the merge without world capabilities.

### Acceptance and evidence

- [ ] The two-machine result matches serial for the same recorded answers. No promise of equality for independently sampled worlds or exactly-once external effects.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
