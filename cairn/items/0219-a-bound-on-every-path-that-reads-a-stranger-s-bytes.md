---
id: 219
title: A bound on every path that reads a stranger's bytes
type: chore
status: unmarked
milestone: warden
depends_on:
- 216
created: 2026-09-13
updated: 2026-09-15
priority: p1
effort: l
area: crates/
stratum: '0'
proof: No input under one megabyte causes more than ten megabytes of allocation, on any entry point, asserted
---

Two allocation bugs have been found by hand: a cairn count that reserved 396 MB
from 8 MB of input, and an integer overflow in the error path of the guard that
was supposed to stop it.

Both were in the decoder because that is where somebody looked. The rule should
hold everywhere and be checked rather than reasoned about.

## Delivery plan — 2026-09-15

### Starting point and scope

The sub-1 MiB input / 10 MiB allocation claim is a proposed universal bound, not an established property.

### Steps

1. Inventory all untrusted entrypoints with 0216, including aggregate expansion and native/WASM paths.
2. Define peak-live versus cumulative allocation, baseline exclusions, decoded expansion and recursion/time limits.
3. Instrument each path with adversarial inputs; propose an explicit proof/spec amendment if the target is impossible.

### Acceptance and evidence

- [ ] Every inventoried path meets the stated unambiguous bound. Do not silently narrow coverage to decoder buffers or ignore repeated low-live-memory allocation.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
