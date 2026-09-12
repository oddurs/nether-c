---
id: 47
title: The bidirectional depth checker
type: feature
status: buried
milestone: calculus
assignee: Oddur Sigurdsson
depends_on:
- 13
- 46
created: 2026-09-10
updated: 2026-09-12
priority: p0
effort: l
area: crates/nether-core
stratum: '0'
proof: Every example in spec section 02 checks or fails exactly as the spec says
---

Depth as a lattice, composing by max. Inference good enough that annotations
are optional in ordinary code and read as documentation when present.

## 2026-09-12

The checker derives a depth for every expression and compares it against the depth the IR states, so the IR is self-validating rather than merely annotated. Two premises are enforced because two are written down: APP's df <= delta and LOOK's d <= delta. The ambient bound is otherwise a theorem, not a check — a descent concludes at the depth it reached. Forms section 02 gives no rule for compose by PRIM, which is the only rule available and the conservative one.

## 2026-09-12

It checks depth and deliberately not types. An earlier draft checked arity, what a call produces and what each rite produces, none of which any depth rule depends on; that was drift and it came back out. What stays is the three places a depth rule reads a type: a shade's origin, an arrow's latent depth, and a prelude reference's stratum.

## 2026-09-12

Found two contradictions in section 02 on the way, both fixed first in their own pull requests. 0119: ambient soundness was stated as d <= delta, which DESCEND refutes and which forbids section 1.3's opening example. 0120: APP's premise was on all three terms, which forbids section 6.2's compile(src) and section 9.2's given(a). Also found that APP's middle term is unreachable today — nothing in the language produces a function value from the world — which is not a reason to drop it.
