---
id: 270
title: 'bury: reduce under an undecided branch'
type: feature
status: unmarked
milestone: futamura
depends_on:
- 251
created: 2026-09-18
updated: 2026-09-18
priority: p1
effort: l
stratum: '0'
area: crates/nether-bury
proof: An interpreter buried against a guest it branches on specialises past the branch, and a slice that would collapse in an untaken arm does not stop the burial
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

- [ ] An `if` on an unanswered condition residualises with both arms reduced
- [ ] A world-question inside an arm forms no hole, and the trace's hole count
      is what it was before the arm was reduced
- [ ] A deposit inside an arm is not in the trace
- [ ] An out-of-range slice inside an arm does not stop the burial
- [ ] The same program buries to the same trace on two runs, fuel included
- [ ] The bootstrap's residue is measured again and both figures recorded

## Evidence to close

Set `proof` to the observable gate. Record the tested commit, exact checks and
remaining limits here. Human or elapsed-time evidence cannot be replaced by CI.
