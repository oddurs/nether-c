---
id: 15
title: 'Decide: the fuel budget and the opaque barrier'
type: spec
status: buried
milestone: codex
created: 2026-09-10
updated: 2026-09-10
priority: p0
effort: m
area: spec/06-evaluation.md
proof: A program that would unroll forever fails with a message naming the line
---

## The question

Aggressive partial evaluation will happily unroll a loop into a gigabyte, and
a pure infinite loop at depth 0 hangs the compiler rather than the program.
Both need a stated answer in the spec, not a flag discovered later.

## Proposal

A fuel budget on burial, and an `opaque` barrier that a programmer uses to
say *do not evaluate through this, even though you could*.

## Acceptance criteria

- [x] Fuel accounting is deterministic — same source, same exhaustion point
- [x] Running out of fuel is a diagnostic, not a crash, and names a source span
- [x] `opaque` has a typing rule, not just an implementation behaviour

## 2026-09-10

PROPOSED, in spec/06-evaluation.md 6.4: a deterministic fuel budget reported in the trace, plus `opaque` as a typing rule rather than a flag — because whether burial evaluates through an expression changes the artifact, and anything that changes the artifact belongs in the language.

## 2026-09-10

SETTLED. Deterministic fuel budget, reported in the trace; `opaque` is a typing rule, not a flag. Rejected alternative (both as command-line concerns) recorded in 90.2 — a trace buried under a different budget is a different trace.
