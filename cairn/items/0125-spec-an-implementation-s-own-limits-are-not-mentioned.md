---
id: 125
title: 'Spec: an implementation''s own limits are not mentioned'
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
proof: A program that recurses deeper than the implementation can hold stops with a diagnostic and not a crash
---

## The hole

`spec/06-evaluation.md` §6.4 says:

> Exhausting fuel is a diagnostic, not a crash.

and fuel bounds steps. It does not bound depth. A program that recurses
without stopping spends one step per node and a whole stack frame per call, so
on any implementation that evaluates by recursion the host stack goes first:

```c
I64 down(I64 n) { return down(n + 1); }
demand down(0);
```

With a million steps of fuel this is a stack overflow, which is a crash, which
§6.4 forbids and which no diagnostic in the document covers.

Found by the burial in 0050, where it is reproducible in a line.

## Why fuel cannot cover it

Fuel is a bound on work and depth is a bound on space, and no budget expresses
both: a program can spend a million steps a thousand frames deep or a million
steps two frames deep. Charging more fuel per frame changes the constant and
not the shape.

## What this must decide

That an implementation has limits of its own, and what it owes when one is
reached. The narrow version:

> Burial has one other bound: the implementation's own. Reaching it MUST be
> reported the way exhausted fuel is — naming the limit, its value, and where
> it was reached — and MUST NOT be a crash. An implementation MUST state its
> limits. Two implementations with different limits may disagree about whether
> a program buries at all; they MUST NOT disagree about the result when both
> finish.

The last sentence is the one that matters. It keeps every reproducibility
claim in the document intact while admitting the thing that is true anyway.

## Acceptance criteria

- [x] §6.4 says an implementation has limits and what it owes when one is hit
- [x] It says what that costs the determinism requirement above it, exactly
- [x] `spec/90-rationale.md` records what was rejected
- [x] The site is rebaked

## 2026-09-12

The sentence that matters is the last one: two implementations with different limits may disagree about whether a program buries at all, and may not disagree about the result when both finish. Every reproducibility claim in the document is of the second kind, so admitting the first costs nothing that was ever promised — and pretending otherwise cost a guarantee no recursive evaluator can keep.
