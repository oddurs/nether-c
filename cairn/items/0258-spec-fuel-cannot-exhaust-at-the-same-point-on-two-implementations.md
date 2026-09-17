---
id: 258
title: 'Spec: fuel cannot exhaust at the same point on two implementations'
type: spec
status: buried
milestone: codex
assignee: Oddur Sigurdsson
created: 2026-09-16
updated: 2026-09-16
priority: p0
effort: m
stratum: '0'
area: spec/06-evaluation.md
proof: §6.2 and §6.4 make one claim about evaluation order between them
---

## The two sentences

`spec/06-evaluation.md` §6.2:

> Within a single demand, evaluation order is unspecified **except** that it
> MUST be deterministic for a given source and capability set

§6.4:

> Fuel accounting MUST be deterministic — the same source and capabilities must
> exhaust at exactly the same point on every implementation and every machine.
> That is a requirement about agreement between implementations and not only
> with oneself, which is why the step is defined here rather than left to one.

A step is charged per node, once, when evaluation of that node begins. Which
node is reached first is order. So where a budget runs out is a function of the
order §6.2 declines to specify, and §6.4 requires two implementations to agree
on it.

The *total* is order-independent once the burial finishes — the same nodes are
charged either way — which is why this survived review. It is visible only
under a budget that binds, which is the case §6.4 exists for.

## Which one gives

**Specify the order.** §6.2's guarantee is that two burials produce the same
trace, and a specified order gives that too. The cost lands on 0204: a parallel
burial has to charge fuel as though it were sequential, or stop claiming §6.4.

**Or weaken §6.4** to what §6.2 supports: fuel accounting is deterministic for
one implementation, and a budget that binds is a fact about that
implementation — like the limits three paragraphs below it, which §6.4 already
admits two implementations may differ on:

> Two implementations with different limits may therefore disagree about
> whether a given program buries at all. They MUST NOT disagree about the
> result when both of them finish.

That sentence is the smaller claim, it is already on the page, and fuel is the
same kind of bound.

## Acceptance criteria

- [x] §6.2 and §6.4 make one claim about evaluation order between them
- [x] 0204 is checked against whichever was chosen
- [x] §90.2 records the claim that was withdrawn

## 2026-09-16

Settled by specifying the order, not by weakening §6.4. §6.2 now says evaluation inside a demand is left to right and innermost first, with a bullet per construct: a call takes the expression naming the function and then its arguments in order, an operator its left then its right, a block its statements then its tail, an if its condition then only the arm it chose, a loop its body then its step. The reason is that the order is in the trace three times over, not twice as the item had it -- §7.3 puts the holes in the order they were discovered, §6.3 gives a shared hole the span of the first place that asked and calls which place that is a fact about the program, and §6.4 decides where a budget cuts. Weakening §6.4 to per-implementation determinism was the other option and is rejected in §90.2: the frame and space limits it would have been matched to decide whether a burial finishes, and §6.4 goes on to require two implementations not to disagree about the result when both of them do -- order is not like those limits, it is in the result. §6.4 now says out loud that it rests on §6.2. crates/nether-bury already evaluated in that order; two tests in holes.rs pin it, one on the order two reads are discovered in at the top level and nested, and one on a budget that binds cutting where the order says. Reversing the argument loop in call() fails the first, so it discriminates. 0204 is noted: a parallel burial must now charge fuel as though sequential as well as discovering holes and keeping spans in the serial order. scripts/task check passes; the ceiling did not move.
