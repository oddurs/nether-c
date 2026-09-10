---
id: 13
title: 'Spec: the typing rules for depth'
type: spec
status: descending
milestone: codex
created: 2026-09-10
updated: 2026-09-10
priority: p0
effort: l
area: spec/02-calculus.md
proof: The rules fit on one page; if they do not, the design is wrong
---

## What this section must answer

The formal core: judgement forms, the depth lattice, and the rules for
`descend`, `seal`, `shade` and `look`.

## Constraints it inherits

At most twelve rules on one page. This is the load-bearing constraint of the
whole language — depth that cannot be explained on a page cannot be taught,
and a lattice nobody can hold in their head will be worked around.

## Acceptance criteria

- [ ] Judgement form for depth-annotated typing
- [ ] Rule for application taking max of its parts
- [ ] Rules for seal (always @0) and shade (opaque lift)
- [ ] The Orpheus rule for look, whichever form we settle on

## 2026-09-10

Draft landed: spec/02-calculus.md. Eleven rules on one page, as required. APP takes the max of three depths, not two — the depth of the function value itself is the classic soundness hole and it is closed here.
