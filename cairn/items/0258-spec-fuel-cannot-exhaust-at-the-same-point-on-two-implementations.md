---
id: 258
title: 'Spec: fuel cannot exhaust at the same point on two implementations'
type: spec
status: unmarked
milestone: codex
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

- [ ] §6.2 and §6.4 make one claim about evaluation order between them
- [ ] 0204 is checked against whichever was chosen
- [ ] §90.2 records the claim that was withdrawn
