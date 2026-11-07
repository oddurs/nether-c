---
id: 281
title: Reduce past a statement that starved
type: spec
status: buried
milestone: futamura
assignee: Oddur Sigurdsson
claimed: 2026-11-07
created: 2026-10-30
updated: 2026-11-07
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

- [x] Are the statements after a starved one reduced speculatively, with the
      starved binding as an unknown, or not at all?
- [x] A `return` after the starved statement: does the residue keep the
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

With reduction past a starved statement (`Burial::after_starved`), the
bootstrap residue drops from 1,433 lines to 1,355 lines (21 minted functions,
4,995,707 steps, one hole). `evaluate_program__` specialises `s` past
`Bytes r = expression__91();`, `expression__` folds `cursor(args)` to `415`
and drops `if (len(lookup(env, t)) > 0)`, and `block_value__` drops
`if (!checking)` and folds `problem(b"unsupported deposit type", p)` inside
the undecided branch after `r`.

## Acceptance criteria

- [x] Every normative claim is stated once, in one place
- [x] Every code sample in it is in `tests/transcripts/`
- [x] A reader who has not read the rest of the spec can follow it

## 2026-11-07

Buried. §6.5 now specifies that once a statement in a block starves, that statement stays in place with any binding left as an unknown local, and the statements and tail after it are reduced speculatively in place under §6.5's three clauses while keeping statement order intact. §90.2 records the rejected alternatives (stopping the block at the first starved statement, or evaluating past it without speculation). Implemented in `crates/nether-bury/src/bury.rs` via `Burial::after_starved`, `Burial::try_expr`, and `Burial::ret_depth`. Tested in `crates/nether-bury/tests/projection.rs` (`the_residue_still_interprets_past_the_unanswered_question`) and `crates/nether-cli/tests/rites.rs` (`a_block_reduces_past_a_statement_that_starved`).
