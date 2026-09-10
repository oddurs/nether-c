---
id: 94
title: 'Decide: what a prelude function does when it cannot answer'
type: spec
status: marked
milestone: codex
depends_on:
- 23
created: 2026-09-10
updated: 2026-09-10
priority: p0
effort: m
area: spec/09-prelude.md
proof: A program that reads a file which may not exist can be written, and the specification says what happens
---

## The question

Section 9.9 says a prelude function that cannot answer — `read` on a missing
file, `utf8` on invalid bytes — **starves**, and that the burial reports it.

That is not a design, it is a description of the failure mode. Starvation as
specified has no recovery: there is no way to write "try this, and if it fails
do that", and a language aimed at build systems needs one on day one.

The specification admits this in 9.9 rather than hiding it. This item is the
admission turned into work.

## Constraints it inherits

- No exceptions. Unwinding has no meaning in a language where evaluation is
  demand-driven and partially staged, and a stack that can be unwound is a
  stack that can be observed.
- Whatever is added has to have a depth. A recovery path that reaches the world
  is deeper than one that does not.
- Whatever is added has to be *recorded*. A failure that happened and was
  recovered from is still a thing that happened, and the trace has to say so —
  otherwise replay is not replay.

## Candidates

1. **A result type.** `Result<T>` in the prelude, `read : Str -> Result<Bytes>
   @3`. Ordinary, teachable, and it makes every call site noisier.
2. **Starvation as a value.** A starved node is already a first-class thing in
   the trace; expose it. `rescue e else f` evaluates `f` if `e` starved. Novel,
   fits the existing machinery, and risks making starvation catchable in places
   where it should not be.
3. **Nothing, and say so.** Declare that failure is a burial-level diagnostic
   and that recovery belongs outside the language. Honest, and probably fatal
   for the build-system use case that motivates the whole design.

## Acceptance criteria

- [ ] A decision, written into spec/09-prelude.md, replacing the current 9.9
- [ ] The rejected candidates recorded in spec/90-rationale.md 90.2
- [ ] A worked example: read a file that may not exist, handle both outcomes
- [ ] Stated interaction with replay: what the trace records about a recovery
