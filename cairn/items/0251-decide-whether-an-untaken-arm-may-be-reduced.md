---
id: 251
title: Decide whether an untaken arm may be reduced
type: spec
status: buried
milestone: futamura
assignee: Oddur Sigurdsson
created: 2026-09-16
updated: 2026-09-18
priority: p1
area: spec/06-evaluation.md
proof: §6.5 states one rule for an undecided arm and names what it forbids
part_of:
- 80
stratum: '0'
---

## What this section must answer

`spec/06-evaluation.md` §6.5 now specialises a starved call to its reduced
body, and §6.2 forbids evaluating anything no `demand` transitively requires.
An arm of an `if` whose condition waits on the world is exactly that, so
reduction stops at the branch and an interpreter goes on interpreting from
there. A first projection therefore stops at the first unanswered question on
its path.

The question is whether an arm that may not be taken may be *reduced* — folded,
inlined, specialised — on the condition that it **asks the world nothing**: it
records no hole and no deposit, and anything that would ask stops where it is.

## Constraints it inherits

- §6.2 is a semantic guarantee, not an optimisation. Its stated reason is that
  evaluating an undemanded expression could reach the world and leave a witness
  for something the program never asked for. A reduction that cannot ask cannot
  do that, which is why the condition above is the one worth arguing about.
- §6.4 requires fuel accounting to be deterministic across implementations. Any
  rule here has to say what reducing an untaken arm costs.
- §6.5's staging law. Reducing an arm that the next burial then does not take
  must leave the composite trace unchanged.

## Open questions

- [x] Recorded. A call inside an undecided arm stops and residualises, even
      where the other arm asks the same question and a hole already exists —
      §6.3's merging does not reach across into an arm nothing has committed to.
- [x] Nothing but the budget, and that is stated as a cost rather than
      solved. A separate speculative budget was rejected: the residue would
      then depend on the invocation, which §8.2 forbids.
- [x] Yes, and no new construct. Without it the projection stops at the first
      branch on an unknown value, which for an interpreter is the first one it
      reaches; `opaque` remains the only control, in the only direction worth
      asking for.

## Delivery steps and dependencies

1. Settle the condition, in one sentence, against the three constraints above.
2. Write it into §6.5 beside the block quote that names this item, and remove
   that block quote.
3. Record the rejected reading in `spec/90-rationale.md`.
4. Name the implementation item that discharges it.

## Acceptance criteria

- [x] Every normative claim is stated once, in one place
- [x] Every code sample in it is in `tests/transcripts/`
- [x] A reader who has not read the rest of the spec can follow it

## 2026-09-18

Settled yes, with three refusals: a speculative reduction may not form a hole, may not deposit, and may not collapse. The first two are §6.2's own reason turned into a rule — it forbids undemanded evaluation because such an expression could reach the world, and a reduction that cannot reach it leaves nothing for the guarantee to protect.

## 2026-09-18

The third refusal is the one that would have been got wrong. An out-of-range slice in an arm the world never takes would otherwise cave in the burial, which makes whether a program buries depend on how far a specialiser got. That is the objection that killed rescue, arriving from the other side.

## 2026-09-18

A separate speculative budget was rejected even though it removes the divergence regression. The residue would depend on how much fuel was left, two budgets would give two residues, and §8.2 says a budget is not in a trace precisely because it cannot have affected one. 0270 implements it.
