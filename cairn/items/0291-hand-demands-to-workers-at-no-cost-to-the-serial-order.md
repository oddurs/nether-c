---
id: 291
uid: 6040e8da-cae0-4027-a11d-15a3cef63e6b
title: Hand demands to workers at no cost to the serial order
type: bug
status: buried
milestone: assay
assignee: Oddur Sigurdsson
created: 2026-11-08
updated: 2026-11-08
closed_at: 2026-11-08
priority: p2
effort: m
stratum: '0'
area: crates/nether-bury
proof: Burying demands that reach the world on every core costs within 0.2 µs per demand of one thread, and burying heavy pure demands on every core is no slower than before, measured in a release build at the same load
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

## 2026-11-08

Corrected. Measured again with more runs, the cost per demand grows from 2,000 to 32,000 by about the same factor on one thread as on fourteen, which is cache, not the handoff. What holds is that fourteen threads were 1.5 to 2 times slower than one for demands that reach the world. A profile showed no thread waiting: thirteen workers were busy forming holes (SipHash, blake3, allocation) for demands that shares() had already stopped, competing with the serial order. Three changes: shares() stops a worker where it is, a run of 2 x workers empty early results stops the workers, and one Mutex<Vec<Slot>> with a Condvar replaces a channel and a mutex per demand. At load 12, main against this: world demands on 14 threads 1.48-1.68 against 1.07-1.37 us (one thread 0.99-1.30 either way); 64 heavy pure demands on 14 threads 366 against 308 ms (one thread 1.42 and 1.38 s). A first try at the slot gave each slot its own Mutex with one shared Condvar, which std refuses at runtime; seeded_programs_match_serial_objects_spans_and_cairn then hung rather than failing, because its jitter threads were only stopped on success, so it now stops them on drop.
