---
id: 206
uid: 29439edb-464f-444e-9da5-6ce8d1019f6a
title: Answer many holes at once
type: feature
status: buried
milestone: cortege
depends_on:
- 209
created: 2026-09-13
updated: 2026-11-07
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

- [x] Eligible requests overlap while witness order stays fixed. Real-network timing does not establish deterministic equality; no world effect is assumed pure.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

## Evidence — 2026-11-07

- `spec/06-evaluation.md` §6.6 (`Answering in parallel`) and `spec/90-rationale.md` §90.2 (`Answering every hole in a trace at once`) specify that contiguous runs of read-stratum holes (strata 1, 2, 3 and 5) may be answered concurrently under a fixed in-flight bound while act-stratum holes (strata 4, 6, 7 and 8) are answered alone in sequence, witnesses are committed in discovery order, and fatal failures stop at the first failing hole in discovery order while cancelling unstarted later holes in the run.
- `World::ask_all` in `crates/nether-world/src/provider.rs` dispatches contiguous read-stratum runs across up to `MOST_IN_FLIGHT = 8` scoped worker threads (`Provider: Sync`), commits results in discovery order, and uses an atomic `cutoff` index to cancel unstarted holes past a fatal error while still finishing earlier indices so the first failure in discovery order is reported.
- `crates/nether-cli/src/exhume.rs` (`dig_up`) answers a trace's holes through `World::ask_all` before recording witnesses, `said`, and `Answers` in discovery order.
- Verified by `many_holes_answer_concurrently_and_keep_discovery_order` and `concurrency_is_bounded_and_collapsing_cancels_later_holes` in `crates/nether-world/tests/discipline.rs`, and `many_network_holes_are_answered_concurrently_in_discovery_order` in `crates/nether-cli/tests/rites.rs`.
