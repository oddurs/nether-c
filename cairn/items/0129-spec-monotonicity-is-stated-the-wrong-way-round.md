---
id: 129
title: 'Spec: monotonicity is stated the wrong way round'
type: spec
status: buried
milestone: calculus
depends_on:
- 13
created: 2026-09-12
updated: 2026-09-12
priority: p0
effort: m
area: spec/02-calculus.md
proof: Ten million generated programs, no counterexample to the inequality as section 2.4 states it
---

## The counterexample

`spec/02-calculus.md` §2.4:

> **Monotonicity.** If `Γ ; δ ⊢ e : τ@d` and `e ⟶ e′`, then
> `Γ ; δ ⊢ e′ : τ@d′` with `d′ ≥ d` — never less.

Found by the generative test in 0052, at round 47 of the first seed, in the
shape of:

```c
if (true) { 1 } else { must(descend disk { read("k") }) }
```

The conditional's depth is the maximum of its parts, which is 3. Burial
reduces it to `1`, which is at depth 0. So `d′ < d`, and the law says never.

## Which of the two is wrong

The law. The depth of that expression was never a fact about a value — it was
an upper bound over an arm that was not taken. Burial did not lower anything;
it found out which arm it was.

And the inequality is the wrong way round for what the lattice is for. A type
claiming depth 3 for a value that turns out to be pure is conservative and
harmless. A type claiming depth 0 for a value that reached the disk is the one
thing the whole design exists to prevent, and that is what `d′ ≤ d` forbids.

`spec/01-strata.md` §1.2 — *no evaluation step lowers the depth of a value* —
is a different statement and is not affected. It says there is no `ascend`: no
operation takes a deep value and hands back a shallow one. Picking an arm does
not take a deep value anywhere, because the deep value was never produced.

## The rule that is missing underneath it

§2.2 has eleven rules and none of them is a conditional, while
`spec/04-grammar.md` has four constructs that branch: `if`, `while`, `&&` and
`||`. An implementation has to give them a depth and the specification does
not say which.

[PRIM] already covers it — a conditional is a primitive operation over its
condition and its arms, and its depth is their maximum — but that has to be
written down rather than inferred, because it is exactly the rule whose depth
is an upper bound rather than a fact, and it is what makes the law above false
as stated.

Eleven rules is a hard constraint and this adds none: it is a sentence about
what [PRIM] ranges over.

## Acceptance criteria

- [x] §2.2 says a conditional is [PRIM], in a sentence, and stays at eleven rules
- [x] §2.4's inequality is one no program refutes
- [x] §2.4 says what a depth in a type is a bound on, and how that differs from the depth in a trace
- [x] §1.2 is left alone, and §2.4 says why they are different claims
- [x] `spec/90-rationale.md` records the version that was wrong and what it cost

## 2026-09-12

The inequality flips to d' <= d. A depth in a type is an upper bound on how far into the world the value's history reaches, and evaluation can only ever find out that something was shallower than it looked. The direction that carries the guarantee is the one that forbids a value escaping at a depth its type did not admit — which is what the lattice is for — and that is <=, not >=.

## 2026-09-12

Three numbers were being read as one and section 2.4 now separates them: the bound a type offers before anything runs, the fact section 1.2 states about operations (there is no ascend), and the exact depth a trace records with a witness beside it. The first was carrying the prose of the third.

## 2026-09-12

No twelfth rule. A conditional is PRIM over its condition and its arms, which is a sentence about what PRIM ranges over rather than a new rule, and eleven is a hard constraint. The condition is in the maximum because which arm ran is something the condition knew.
