---
id: 80
title: The first projection
type: feature
status: unmarked
milestone: futamura
depends_on:
- 48
- 79
created: 2026-09-10
updated: 2026-09-15
priority: p1
effort: l
area: lib/
stratum: '0'
proof: Burying the interpreter with a fixed program yields something that runs that program directly
---

Specialise the interpreter to a program: a compiled program falls out. Not a
metaphor — this is what burial already does, pointed at itself.

## Delivery plan — 2026-09-15

### Starting point and scope

0079 is a sample-program bootstrap, not a self-interpreter. Read lib/README.md's supported grammar and crates/nether-bury/tests/interpreter.rs before making a projection claim.

### Steps

1. Specify the fixed guest input and remaining dynamic inputs, and define how to distinguish specialization from simply evaluating a closed program.
2. Use a supported guest with an unanswered world-question; bury interpreter plus guest, inspect the residual program and resume it with distinct answers.
3. Compare result, holes, deposits and failures with direct interpretation; measure residual size and host steps.

### Acceptance and evidence

- [ ] The residual program demonstrably specializes the fixed guest and handles remaining inputs correctly. Pin the guest, answers and measurements; do not call constant output or a wrapper around interpret a projection.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
