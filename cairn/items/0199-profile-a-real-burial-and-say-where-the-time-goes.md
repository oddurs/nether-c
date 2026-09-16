---
id: 199
title: Profile a real burial and say where the time goes
type: chore
status: unmarked
milestone: quickening
depends_on:
- 103
created: 2026-09-13
updated: 2026-09-15
priority: p0
effort: s
area: crates/nether-bury
stratum: '0'
proof: The three largest costs in a burial are named, with numbers, in spec/90-rationale.md or an item note
---

Nobody has looked. Guessing which part is slow is how a week gets spent on the
part that was not.

The pilot in 0103 is the honest workload: a real program, not a microbenchmark.
Until that exists, `site/bake` rebuilt in Nether C is the closest thing.

## Delivery plan — 2026-09-15

### Starting point and scope

lib/README.md reports interpreter steps, not a profile of the 0103 site pilot. The pilot gate remains explicit.

### Steps

1. Prepare instrumentation with existing samples, labelling those measurements preliminary.
2. Once 0103 exists, profile parsing, checking, reduction, encoding and store I/O separately, warm and cold.
3. Record commit, corpus, toolchain, machine, repetitions and the three dominant costs.

### Acceptance and evidence

- [ ] Measured pilot burial costs justify the ranking. No optimization is included; findings scope 0198 and 0203.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
