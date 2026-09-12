---
id: 119
title: 'Spec: ambient soundness is stated as something that is not true'
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
proof: Section 02 states one invariant, and the eleven rules all satisfy it
---

## The contradiction

`spec/02-calculus.md` §2.1 says:

> The invariant `d ≤ δ` holds in every derivable judgement. An implementation
> MUST reject any program in which it does not.

and §2.4 says the same thing again as a metatheorem:

> **Ambient soundness.** If `Γ ; δ ⊢ e : τ@d` then `d ≤ δ`.

[DESCEND] breaks both:

```
              Γ ; max(δ, s(κ)) ⊢ e : τ@d
  [DESCEND] ──────────────────────────────
              Γ ; δ ⊢ descend κ {e} : τ@d
```

The conclusion carries `d` out into ambient `δ`, and `d` is up to `s(κ)`,
which is the whole point of the rule. `spec/01-strata.md` §1.3 opens with the
counterexample:

```c
Bytes@3 src = descend disk { read("kernel.nc") };
```

Depth 3, at the top level of a file, where δ is 0. Under §2.1 as written an
implementation MUST reject the specification's own introductory example.

Found while building the checker for 0047, which cannot implement a rule and
its negation at once.

## What is actually true

The prose under §2.4 already says it:

> A value can never be deeper than the capabilities that were held while it
> was made.

Held *while it was made* — which is the ambient depth **or** a descent inside
the expression that granted more. The formula above it says something
narrower and false. The blame property it is there to support survives
unchanged: a value at depth 5 means some `descend net` is responsible, and it
is either an enclosing one or one in the expression itself.

## Acceptance criteria

- [x] §2.1 says where `d ≤ δ` is required, which is the premises of [APP] and [LOOK]
- [x] §2.4's ambient soundness is a statement the eleven rules satisfy
- [x] The rejected ways of keeping the simpler statement are in §90.2
- [x] The site is rebaked

## 2026-09-12

Corrected in place rather than by adding a second statement: section 2.1 now says where d <= delta is required (the premises of APP and LOOK) and section 2.4 states the invariant the eleven rules actually satisfy. The prose under 2.4 was already right — 'the capabilities that were held while it was made' — and only the formula above it was wrong, so the blame property nether strata rests on did not move.
