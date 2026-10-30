---
id: 281
title: Reduce past a statement that starved
type: spec
status: unmarked
milestone: futamura
created: 2026-10-30
updated: 2026-10-30
priority: p1
effort: l
stratum: 0
area: spec/06-evaluation.md
proof: An interpreter buried against a guest with an unanswered question specialises past the statement that binds the answer, and the_residue_still_interprets_past_the_unanswered_question has to change
---

## What this section must answer

§6.5: "A block reduces its statements in order and stops at the first one that
starves." Everything after it is in the residue unreduced. Whether the
statements after a starved one may be reduced, and under what rules.

## Constraints it inherits

- §6.5's speculative reduction (0251, built in 0270) already defines a mode that
  may produce a value or stop and nothing else: no hole, no deposit, no
  collapse. The statements after a starved one are not undecided — they run if
  it finishes — but they may read what it binds, which is not here.
- §6.2 forbids evaluating what no demand requires. The statements after a
  starved one are required, once the world answers.
- The staging law: burying the residue with the answer must reach the trace a
  single burial with the answer would have.

## Open questions

- [ ] Are the statements after a starved one reduced speculatively, with the
      starved binding as an unknown, or not at all?
- [ ] A `return` after the starved statement: does the residue keep the
      statement order exactly, so nothing it did moves before the starved one?

## Delivery steps and dependencies

1. Decide, in §6.5, with the rejected alternative in §90.2.
2. Implement in `crates/nether-bury`'s `block`, reusing 0270's speculation.
3. Measure the bootstrap again (below) and change the projection test.

## Evidence

Found implementing 0270. With speculation under an undecided `if` built, the
bootstrap — `lib/interpreter.nc`, `value.nc` and `evaluate.nc` buried against
`tests/programs/build.nc` with `read("main.nc")` unanswered — is unchanged:
1,433 lines, 21 minted functions, 4,993,540 steps, one hole, four calls to the
general `evaluate_program`. The interpreter's branch on the answer is
`if (failed(found(r)))`, and the statement before it, `Bytes r =
expression__91();`, is the one that starves. The block stops there and the
branch is never reached.

## Acceptance criteria

- [ ] Every normative claim is stated once, in one place
- [ ] Every code sample in it is in `tests/transcripts/`
- [ ] A reader who has not read the rest of the spec can follow it
