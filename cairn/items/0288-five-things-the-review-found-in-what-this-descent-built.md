---
id: 288
uid: 2e828eda-403a-4cac-8aaf-adaec7736922
title: Five things the review found in what this descent built
type: chore
status: buried
milestone: assay
assignee: Oddur Sigurdsson
created: 2026-11-08
updated: 2026-11-08
closed_at: 2026-11-08
priority: p1
effort: m
stratum: '8'
area: nether-foreign, nether-world, nether-ledger, nether-bury
proof: 'Each of the five has a test that fails or does not finish on the old code: calls into one object take turns, a base witness is named as itself, export order matches the old rounds, a twenty-thousand-link chain exports, and a worker stops when the serial order halts'
---

## What happens

The review of `1d2edf4..main` found five things beyond 0284 and 0285:

1. `unsafe impl Sync for Object` in `nether-foreign` rested on "stratum 8 calls
   are never dispatched concurrently", which only `World::ask_all` followed.
   Nothing stopped two threads calling into one object at once.
2. `Replay::merge` scanned every earlier slot for each delivery and for each
   chosen slot: O(n²) in the holes.
3. `merge` named a bad base witness as slot 0, which it has nothing to do with.
4. `Store::export` scanned every pending object once per level of the DAG:
   O(n²) for a chain. Its fallback for a cycle silently broke the order.
5. A worker that took a demand ahead kept its whole fuel budget after the
   serial order halted, and the halt waited for it.

The review also said a truncated `import` leaves a partial import that breaks
`audit`. §7.5.4 specifies that on purpose: entries arrive leaves first, so any
prefix is dependency-closed.

## Fix boundary and regression proof

1. `Object` holds its handle in a `Mutex`, held for every use of it. The
   `unsafe impl Sync` is gone. The handle is `Send`, which POSIX says it is.
   `calls_into_one_object_take_turns_across_threads` fails on the old code.
2. `merge` keeps the lowest chosen slot per read-stratum call, and a count of
   earlier slots per call: O(n). Same errors as before.
3. `MergeError::BaseWitness(Cairn)`.
4. `export` sorts by height, then cairn, which is the order the rounds gave.
   A cycle is an error. `export_order_is_the_order_rounds_of_ready_objects_gave`
   compares with the old algorithm, and the chain proof did not finish in two
   minutes on the old code.
5. Workers carry a stop flag that `burn` checks, raised when the serial order
   returns. `a_worker_stops_when_the_serial_order_halts` did not finish in a
   minute on the old code and takes under half a second now.
