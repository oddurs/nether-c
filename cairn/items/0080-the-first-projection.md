---
id: 80
title: The first projection
type: feature
status: descending
milestone: futamura
assignee: Oddur Sigurdsson
claimed: 2026-09-16
depends_on:
- 48
- 79
created: 2026-09-10
updated: 2026-09-16
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

## 2026-09-16

Burial does not specialise, and 0080 cannot be proved until it does. Buried lib/interpreter.nc + lib/value.nc + lib/evaluate.nc against tests/programs/build.nc with read("main.nc") unanswered, at 3b0195e: the residue is 1,163 lines and its only demand is interpret(b"...the whole of build.nc..."), so the 4,993,540 steps that read, indexed and checked the guest are in the trace and not in the residue. That is a wrapper around interpret, which this item's acceptance criteria name and reject. The cause is crates/nether-bury/src/bury.rs: apply returns None as soon as a body's result is stuck, and block returns at the first statement that starves without keeping what came before it. A prototype of the opposite rule specialised the same burial into 21 functions, with world_request reduced to read("main.nc") and the guest's path already resolved, for 4,993,540 steps — the same fuel — and a residue of 1,478 lines. It also showed the limit: an if whose condition waits on the world residualises with both arms unreduced, so the interpreter goes on interpreting past the hole. spec/06-evaluation.md §6.5 now states the rule and names that limit; 0252 implements it; 0251 is the open question about reducing an arm that may not be taken. 0080 stays descending and its proof is unchanged.
