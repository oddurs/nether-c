---
id: 206
title: Answer many holes at once
type: feature
status: unmarked
milestone: cortege
depends_on:
- 209
created: 2026-09-13
updated: 2026-09-15
priority: p1
effort: l
area: crates/nether-world
stratum: '5'
proof: A trace with N independent holes answers them concurrently, and the witnesses are recorded in discovery order
---

Exhumation is where the wall clock actually hurts: N network fetches in
sequence is N round trips.

Section 6.3 guarantees finished arguments, not independent world effects.
Only operations whose overlap is explicitly permitted may be asked at once.
The answers must still be recorded in discovery order, because that is in the
trace.

Stratum 6 is different and should be excluded until somebody argues otherwise:
sending is not idempotent and a concurrent send is a different act from a
sequential one.

## Delivery plan — 2026-09-15

### Starting point and scope

Fully evaluated arguments do not imply independent world effects. Mutable reads need care; exclude sending, entropy and unrecorded effects initially.

### Steps

1. Use 0209 to specify eligible provider operations, bounded concurrency, cancellation and failure behavior.
2. Dispatch only eligible holes; record witnesses in discovery order before exposing answers to resumed evaluation.
3. Force out-of-order completion with delayed deterministic providers; cover refusals, duplicates, cancellation and replay.

### Acceptance and evidence

- [ ] Eligible requests overlap while witness order stays fixed. Real-network timing does not establish deterministic equality; no world effect is assumed pure.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
