---
id: 126
title: 'Spec: starvation is the name of two different things'
type: spec
status: buried
milestone: calculus
assignee: Oddur Sigurdsson
depends_on:
- 20
created: 2026-09-12
updated: 2026-09-12
priority: p1
effort: s
area: spec/06-evaluation.md
proof: 'One word, one meaning: a reader of any one section can tell which of the two is meant without reading the other'
---

## The collision

`spec/06-evaluation.md` §6.4 opens with:

> An expression **starves** when it cannot be evaluated and cannot become a
> hole — because it depends, transitively, on a hole. Starvation is normal; a
> starved expression is residualised and waits.

`spec/09-prelude.md` §9.9 says:

> ### Starvation is a bug
>
> An expression **starves** when it cannot produce a value and never will …
> Starvation is not catchable. There is no `rescue`, no `try`, no recovery
> form, and there will not be one.

One is ordinary and resolves by exhumation. The other stops the burial and
never resolves. §9.9 notices, and handles it with a note calling the first one
"a third and different thing" — which is an admission rather than a fix.

`CLAUDE.md` settles which way the project's own vocabulary runs: **starved**
is the word for *blocked*, and `cairn` has a status of that name meaning work
that is waiting on something. So it is §9.9's usage that has no word of its
own, not §6.4's.

Found while building 0050, where the two are different variants of the same
enum and had to be told apart to be reported at all.

## What this must decide

A word for the fatal one. Not a decision to take quietly: the vocabulary is
load-bearing here, and one of the two sections has to change a term that
readers will already have learned.

The candidate that fits the register: an expression that can never be fed has
not starved, it has **collapsed** — and a burial that hits one caves in rather
than waiting. That leaves `starved` meaning *blocked*, which is what it means
everywhere else in the project.

## Acceptance criteria

- [x] One word, one meaning, in §6.4 and §9.9
- [x] `spec/10-glossary.md` has both
- [x] `CLAUDE.md`'s table has the new word if there is one
- [x] `nether-bury`'s `HaltKind` follows

## 2026-09-12

Settled as the item proposed: the fatal one is a collapse. What decided it was not taste but section 10, which defended the merged word on the ground that both stop a burial -- and section 6.4 says a starved expression is residualised and waits. The stated reason for the merge was false of the case it was covering for. Both rejected ways out are in section 90.2.

## 2026-09-12

HaltKind lives in nether-bury, not nether-core. The tests/starvation.rs file is now tests/collapse.rs, because it proves the fuel and collapse diagnostics and neither is starvation.
