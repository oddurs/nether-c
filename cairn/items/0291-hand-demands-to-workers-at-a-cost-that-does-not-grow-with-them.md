---
id: 291
uid: 6040e8da-cae0-4027-a11d-15a3cef63e6b
title: Hand demands to workers at a cost that does not grow with them
type: bug
status: unmarked
milestone: assay
created: 2026-11-08
updated: 2026-11-08
priority: p2
effort: m
stratum: '0'
area: crates/nether-bury
proof: Burying N independent demands on every core costs the same per demand at N = 2,000 and N = 32,000, measured in a release build
---

## What happens

Burying many independent demands costs more per demand as their number grows,
but only on more than one thread. Release build, one read per demand, best of
three:

| threads | 2,000 | 8,000 | 32,000 |
|---|---|---|---|
| 1 | 4.01 µs | 1.84 µs | 4.04 µs |
| 14 | 3.33 µs | 3.06 µs | 9.04 µs |

On one thread it is flat. On fourteen it triples between 8,000 and 32,000. A
sample of the fourteen-thread run is mostly `semaphore_timedwait_trap` and
`__ulock_wait`: threads waiting, not working.

The handoff is #236 (`Burial::burrow`, `Burial::demands`). Each demand has a
channel and a mutex, every worker takes the next index, and a demand that
reaches the world (`shares`) stops at once, so for demands like these a worker
does almost nothing between handoffs and the cost is the handoff.

## What should happen

The handoff costs the same per demand whatever the number of demands, and a
burial on every core is never slower than on one.

## Fix boundary and regression proof

`crates/nether-bury/src/bury.rs`, `burrow` and `demands`. A release benchmark
over the sizes above, recorded here before and after. A wall-clock test in the
suite is not the proof: 0286 is why.
