---
id: 120
title: 'Spec: APP''s premise forbids the specification''s own examples'
type: spec
status: buried
milestone: calculus
depends_on:
- 13
created: 2026-09-12
updated: 2026-09-12
priority: p0
effort: s
area: spec/02-calculus.md
proof: Every sample program in the specification satisfies the premise of APP
---

## The contradiction

[APP] in `spec/02-calculus.md` §2.2 has the premise

```
max(dƒ, d_f, d_a) ≤ δ
```

so applying any function to a deep argument requires holding that depth, even
when the function is pure and reaches nothing. Three of the specification's
own examples do exactly that and are described as legal.

`spec/06-evaluation.md` §6.2, at the top level of a file, where δ is 0:

```c
Bytes@3 src = must(descend disk { read("main.nc") });
Bytes   obj = compile(src);      // pure, but starves: src is a hole
```

`must` is applied to a depth-3 answer and `compile` to a depth-3 value, and
the comment says the second is pure and merely starves — that is, legal.

`spec/09-prelude.md` §9.2 does it again with `given(a)` and `must(a)` on a
depth-3 answer, also at δ 0.

And §2.5 says it in prose:

> [PRIM] and [APP] already take the maximum, so a shallow value combines with
> a deep one without any coercion.

[PRIM] has no ambient premise at all. [APP] does, and that is the only reason
the two disagree.

## What the premise is for

A capability is required to *reach* a stratum. The only term in [APP] that
reaches anything is `dƒ`, the latent depth of the arrow: that is the stratum
the function touches when it runs. `d_a` is a fact about where the argument
has already been, and whoever put it there held the capability at the time.
Passing it on reaches nothing new.

`d_f` is the same kind of fact, and §2.3 is right that it belongs in the
maximum — a function fetched over the network is a deep value — but it is not
something applying reaches either.

## Acceptance criteria

- [x] [APP]'s premise is the one the specification's examples satisfy
- [x] §2.3 says why only one of the three terms is a premise
- [x] The result is still the maximum of all three; §2.3's soundness-hole paragraph is untouched
- [x] The rejected alternative is in §90.2
- [x] The site is rebaked

## 2026-09-12

The premise narrowed to df <= delta rather than widening the examples. A capability is what it takes to reach a stratum; d_a and d_f are facts about where values have already been, and whoever took them there held the capability then. All three terms stay in the maximum, so section 2.3's soundness-hole paragraph is untouched and only gains a sentence saying why one of the three is a premise and two are not.
