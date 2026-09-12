---
id: 150
title: 'Spec: a budget cannot change a trace, so 8.2 asks for the impossible'
type: spec
status: buried
milestone: rites
created: 2026-09-12
updated: 2026-09-12
priority: p1
effort: s
area: spec/08-rites.md
proof: Either Node::Trace records the budget, or 8.2 stops claiming a different budget is a different trace
---

## The hole

§8.2 says:

> `--fuel` sets the budget of §6.4. Its default MUST be finite and MUST be
> reported in the trace, because a trace buried under a different budget is a
> different trace.

`Node::Trace` records `fuel_spent`, which is what burial used, not the budget
it was allowed. Two burials of one program under different budgets that both
finish spend the same fuel and produce byte-identical traces.

## Why the reasoning behind it does not hold

Running out of fuel produces a `Halt`, not a trace (§6.4, and the CLI gives it
exit 75 per §8.8). So a budget has exactly two effects: the burial finishes, or
there is no trace at all. It can never change *which* trace you get.

That makes §8.2's justification false as written. The budget is an input that
provably cannot reach the output.

## What this must decide

Either:

1. `Node::Trace` gains the budget, and §8.2 is right for a reason it does not
   currently give — perhaps that a trace should record the conditions it was
   made under even where they did not bind. That is another frozen-section
   change and another domain bump.
2. §8.2 drops the claim and says what is true: the budget is reported by the
   rite, not recorded in the trace, because it cannot affect one.

Two looks right and one is arguable. The reason to think twice is exhumation:
if a later burial of the same residue under a smaller budget halts where the
first did not, somebody will want to know what the first one was allowed.

## How it was found

Writing the test for 0060. `a_different_budget_is_a_different_trace` asserted
§8.2 directly and failed, because the two traces were identical down to the
cairn.

## Acceptance criteria

- [x] §8.2 and `Node::Trace` agree
- [x] The rejected option is recorded in spec/90-rationale.md
- [x] The test in `crates/nether-cli/tests/rites.rs` asserts whichever it is

## 2026-09-12

Settled the second way: 8.2 drops the claim.

A budget has exactly two outcomes — the burial finished, or there is no trace. It can never change WHICH trace you get, so requiring it to be recorded in one was asking for something that carries no information. 8.2 now says the rite reports it when asked, and that what a trace records is fuel_spent: a fact about the burial that happened rather than about the room it was given.

The rejected option is in 90.2 with its real argument, which is diagnostic: if a later burial of the same residue under a smaller budget halts where the first did not, somebody will want to know what the first was allowed. It lost because adding a field to Node::Trace changes what an existing tag means, which bumps the domain and renames every value ever stored — too much to pay for a number that provably did not affect the result. If the diagnostic case matters it belongs in the rite's output, not the artifact.
