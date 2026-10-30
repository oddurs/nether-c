---
id: 270
title: 'bury: reduce under an undecided branch'
type: feature
status: buried
milestone: futamura
assignee: Oddur Sigurdsson
claimed: 2026-10-30
depends_on:
- 251
created: 2026-09-18
updated: 2026-10-30
priority: p1
effort: l
stratum: '0'
area: crates/nether-bury
proof: An if on an unanswered condition residualises with both arms reduced, and an arm forms no hole, deposits nothing and collapses nothing
---

## Problem

[§6.5](../spec/06-evaluation.md) settled 0251: an implementation SHOULD reduce
the arms of an `if` whose condition waits on the world, and the reduction is
speculative — it may produce a value or stop, and may do nothing else.

`nether-bury` does not. `Select` residualises whole, so a first projection
stops at the first branch on an unanswered value, which for an interpreter is
the first one it reaches.

## Proposal

Reduce both arms in a mode that cannot record. Three refusals, each of which is
a place the existing evaluator does something today:

- **No hole.** `dig` is the only thing that forms one. Under speculation the
  call stops and residualises instead, *including* where the same call already
  has a hole — §6.3's merging does not reach across into an arm the program has
  not committed to.
- **No deposit.** `deposit` likewise.
- **No collapse.** A collapse is a `Halt` today and unwinds the whole burial.
  Under speculation it has to become a stop, and the arm residualises as it
  stands. This is the one worth writing the test for first: an out-of-range
  slice in an untaken arm must leave a trace rather than a diagnostic.

## Starting point and non-goals

`Select` already walks both arms for §5.4's naming analysis in
`nether-core`, so the shape of a two-arm walk exists; this is the evaluator's
side and it is a different walk. Not a goal: deciding *which* arm to prefer, or
reducing across a `while` whose condition waits — a loop residualises whole and
§6.5 is unchanged about that.

## Delivery steps and dependencies

1. A speculative mode on the burial, and the three refusals above.
2. Reduce both arms of an undecided `Select`; keep the branch.
3. Fuel is charged for speculative steps like any other — §6.4 does not except
   them, and two implementations have to agree on the total.
4. Measure the bootstrap again. 0252's note records 1,478 lines of residue and
   21 minted functions against `build.nc`; this changes both numbers and the
   new ones belong beside the old.

## Which stratum does this reach?

0. Speculation is the one mode of evaluation that is defined by not reaching
the world.

## Acceptance criteria

- [x] An `if` on an unanswered condition residualises with both arms reduced
- [x] A world-question inside an arm forms no hole, and the trace's hole count
      is what it was before the arm was reduced
- [x] A deposit inside an arm is not in the trace
- [x] An out-of-range slice inside an arm does not stop the burial
- [x] The same program buries to the same trace on two runs, fuel included
- [x] The bootstrap's residue is measured again and both figures recorded

## Evidence to close

Set `proof` to the observable gate. Record the tested commit, exact checks and
remaining limits here. Human or elapsed-time evidence cannot be replaced by CI.

## 2026-10-30

Buried. select reduces both arms of an undecided branch through Burial::speculate, on a copy of the environment, with speculating set. Under it: a prelude call deeper than the surface neither digs a hole nor takes a recorded answer (taking one would move which answer the next act gets); a bare statement that would deposit starves instead, so it waits in the residue and deposits when an arm is really taken; a unit-level binding reached for the first time is not cached, since a binding that starved because the arm may not ask would stay starved for the code that may. A collapse leaves the arm as written and rolls back the functions minted inside it; an arm that reached break, continue or return is kept as written too, since folding it to its value would drop the jump. Fuel is charged as for anything else and running out is still a halt. Tests in rites.rs: an_undecided_branch_is_reduced_in_both_arms_and_keeps_its_branch (I64 k = 2 + 3 becomes I64 k = 5 in the specialised body, the collapsing slice stays as written, and the burial finishes), an_undecided_arm_asks_nothing_and_deposits_nothing (one hole; nothing lamped), speculation_buries_to_the_same_trace_and_the_same_steps_every_time. No documented transcript moved. Bootstrap measured again: 1,433 lines, 21 minted functions, 4,993,540 steps, one hole, the same as before, because the interpreter never reaches its branch on the answer: the statement before it, Bytes r = expression__91();, starves, and §6.5 stops a block there. The original proof, an interpreter that specialises past the branch, needs reduction past a starved statement, which is a specification question; it is 0281's proof now, with this measurement as its evidence, and projection.rs points its boundary assertion at 0281.
