---
id: 108
title: Soak the decoder for twenty-four hours
type: chore
status: unmarked
milestone: after
created: 2026-09-11
updated: 2026-09-15
priority: p2
effort: s
area: crates/nether-ledger
stratum: '0'
proof: Twenty-four hours of continuous fuzzing on a dedicated machine, no panics and no non-canonical acceptance, with the seed and the duration recorded here
---

## Why this is separate

0045 built the harness and restated its own proof, because "twenty-four hours
of fuzzing" is not something a commit can demonstrate and claiming it would
have been a lie.

`tests/fuzz.rs` runs 320,000 inputs on every commit and `scripts/task fuzz`
does twenty million in about two seconds. What neither does is run long enough
to find the thing that only turns up after an hour, which is the actual point
of a soak.

## What this needs

A machine and a day. Not a change to the code.

- [ ] Run `NETHER_FUZZ_ROUNDS` high enough to occupy twenty-four hours
- [ ] Record the seed, the round count and the duration in this item
- [ ] If it finds anything, the fix arrives with the input pinned as a unit test

## Delivery plan — 2026-09-15

### Starting point and scope

The decoder harness exists. This is an elapsed-time evidence task, not a request to add another fuzzer or equate a high round count with a day.

### Steps

1. Verify the current scripts/task fuzz invocation and seed controls; choose a dedicated machine and a monotonic-time log.
2. Run continuously for at least 24 hours, recording commit, toolchain, seed progression, completed cases and actual start/end times.
3. Minimize failures into regression cases, fix them, and restart the clean qualifying soak after changes.

### Acceptance and evidence

- [ ] A recorded uninterrupted 24-hour passing run at the tested commit. An interrupted session or estimated duration remains partial evidence, never a completed proof.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
