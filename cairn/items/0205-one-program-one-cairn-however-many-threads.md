---
id: 205
title: One program, one cairn, however many threads
type: chore
status: buried
milestone: cortege
assignee: Oddur Sigurdsson
claimed: 2026-11-07
depends_on:
- 204
created: 2026-09-13
updated: 2026-11-07
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

- [x] Exactly one distinct cairn across the qualifying runs, equal to serial. Record counts and scheduling parameters.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

## 2026-11-07

Buried on top of 206cfae. Added two gates: in crates/nether-bury/tests/parallel.rs, seeded_programs_match_serial_objects_spans_and_cairn generates 32 seeded programs mixing independent fib demands, depositing functions, shared pure and starved globals, duplicate read holes from different spans and unmerged write holes, and checks objects, first-hole spans, trace cairn and mid-budget exhaustion across 2, 3, 4, 8 and 16 threads under two contender yield threads; repeated_parallel_burials_produce_one_cairn runs 64 scheduled burials in CI. In crates/nether-bury/tests/proofs.rs, a_thousand_parallel_burials_produce_one_cairn runs 1,000 burials of MIXED across worker counts [2, 3, 4, 6, 8, 12, 16] with 0..4 pre-launch yields and four contender threads: 1 distinct cairn (00c5fa5a, equal to serial), 27,096 steps, 10 objects and 2 holes in 593.59 ms (`cargo test --release --package nether-bury --test proofs a_thousand_parallel_burials_produce_one_cairn -- --ignored --nocapture`). Limits: exercises in-process demand parallelism in nether-bury; multi-process store contention is 0207.
