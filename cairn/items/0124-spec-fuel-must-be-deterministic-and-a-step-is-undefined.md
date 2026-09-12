---
id: 124
title: 'Spec: fuel must be deterministic and a step is undefined'
type: spec
status: buried
milestone: calculus
depends_on:
- 15
created: 2026-09-12
updated: 2026-09-12
priority: p0
effort: s
area: spec/06-evaluation.md
proof: Two implementations reading section 6.4 spend the same fuel on the same program
---

## The hole

`spec/06-evaluation.md` §6.4 says:

> Fuel accounting MUST be deterministic — the same source and capabilities
> must exhaust at exactly the same point on every implementation and every
> machine.

and never says what a step is. One implementation can be deterministic with
itself and still disagree with another about every program, which is the half
of the requirement that matters: the sentence asks for agreement between
implementations and specifies only agreement with oneself.

Three decisions are hidden in it, and each changes the count:

- whether a step is an expression, a reduction, or a call;
- whether a unit-level binding named twice costs its value twice;
- whether `opaque` costs what it contains.

Found while building burial, which had to pick all three to run at all.

## What it looks like settled

A step is one evaluation of one expression node, charged when evaluation of
that node begins and never again. A unit-level binding is evaluated at most
once however many times it is named, and the steps it costs are charged to the
first demand that reaches it. `opaque` costs one step and its contents cost
nothing, because its contents are not evaluated.

That is implementable without reference to any representation, it is countable
by hand from the source, and it makes §6.5's staging law cost what it should:
burying a residue does not re-charge for what was already reduced.

## Acceptance criteria

- [x] §6.4 says what a step is, in a sentence
- [x] It says what a binding named twice costs
- [x] It says what `opaque` costs
- [x] The site is rebaked

## 2026-09-12

A step is one evaluation of one expression node, charged when evaluation begins and charged once. The three places two implementations could otherwise disagree are spelled out rather than left to be discovered: a binding named twice, opaque, and a residue buried again. The third is what makes the staging law in 6.5 affordable as well as true — burying a residue does not re-charge for what was already reduced.
