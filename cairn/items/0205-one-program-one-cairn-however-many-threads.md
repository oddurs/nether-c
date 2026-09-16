---
id: 205
title: One program, one cairn, however many threads
type: chore
status: unmarked
milestone: cortege
depends_on:
- 204
created: 2026-09-13
updated: 2026-09-15
priority: p0
effort: m
area: crates/nether-bury
stratum: '0'
proof: A thousand parallel burials of one program produce exactly one distinct cairn
---

6.2 requires two burials of the same input to produce the same trace including
the order holes were found in. Parallelism is where that stops being free.

Run it a thousand times and count the distinct cairns. One is the only
acceptable answer, and a generative test is worth more than an example here:
the failure is a race and races do not reproduce on request.

## Delivery plan — 2026-09-15

### Starting point and scope

This is 0204's stress proof, not another scheduler. The serial evaluator is the oracle.

### Steps

1. Generate seeded independent/shared demands, duplicate holes, deposits and fuel boundaries.
2. Run at least 1,000 burials of one pinned program with varied worker counts and scheduling delays.
3. Compare objects, first-hole spans and cairns; retain failing seeds and add bounded CI plus a longer scripts/task proof invocation.

### Acceptance and evidence

- [ ] Exactly one distinct cairn across the qualifying runs, equal to serial. Record counts and scheduling parameters.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
