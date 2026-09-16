---
id: 251
title: Decide whether an untaken arm may be reduced
type: spec
status: unmarked
milestone: futamura
created: 2026-09-16
updated: 2026-09-16
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

- [ ] Does "asks nothing" mean no hole *recorded*, or no prelude call deeper
      than the surface *attempted*? They differ when two arms would ask the
      same question.
- [ ] What stops the reduction from diverging where the two arms are a
      recursion that only the answer terminates?
- [ ] Is the rule worth its size, given that `opaque` already exists to make
      burial do less and nothing yet exists to make it do more?

## Delivery steps and dependencies

1. Settle the condition, in one sentence, against the three constraints above.
2. Write it into §6.5 beside the block quote that names this item, and remove
   that block quote.
3. Record the rejected reading in `spec/90-rationale.md`.
4. Name the implementation item that discharges it.

## Acceptance criteria

- [ ] Every normative claim is stated once, in one place
- [ ] Every code sample in it is in `tests/transcripts/`
- [ ] A reader who has not read the rest of the spec can follow it
