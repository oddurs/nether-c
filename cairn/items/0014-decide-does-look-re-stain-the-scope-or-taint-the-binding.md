---
id: 14
title: 'Decide: does look re-stain the scope, or taint the binding?'
type: spec
status: buried
milestone: codex
created: 2026-09-10
updated: 2026-09-10
priority: p0
effort: m
area: spec/02-calculus.md
proof: A worked example of each, and a written judgement with reasons
---

## The question

The Orpheus rule says opening a shade costs you. Two ways to charge for it:

**Scope re-stain.** `look` raises the depth of the entire enclosing scope
to the shade's origin. Dramatic, mythologically exact, and possibly produces
errors nobody can read — a single `look` deep in a function poisons
everything above it with no obvious culprit.

**Binding taint.** Only the bound value takes the depth. Ordinary, tractable,
and much closer to how effect systems people already use behave. But it makes
`shade` little more than a newtype.

## Constraints it inherits

Whichever we pick has to survive section 02 fitting on a page.

## Acceptance criteria

- [ ] A program that is legal under one and illegal under the other
- [ ] The error message each produces, written out
- [ ] A decision, with the losing option recorded in 90-rationale

## 2026-09-10

PROPOSED, in spec/01-strata.md 1.6: neither re-stain nor taint. `look` is well-typed only where the ambient depth already reaches the shade's origin — you may only look by going back down. It is a local check, the error names the exact fix, and the myth survives intact. Both rejected forms are recorded in spec/90-rationale.md 90.2. Awaiting confirmation before this is closed.

## 2026-09-10

SETTLED. `look` is well-typed only where the ambient depth already reaches the shade's origin. Local check, not a propagation; the error names both depths and the descend that fixes it. Rejected forms recorded in spec/90-rationale.md 90.2.
