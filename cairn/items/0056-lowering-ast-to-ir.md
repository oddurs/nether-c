---
id: 56
title: 'Lowering: AST to IR'
type: feature
status: buried
milestone: surface
assignee: Oddur Sigurdsson
depends_on:
- 46
- 55
created: 2026-09-10
updated: 2026-09-12
priority: p0
effort: m
area: crates/nether-syntax
stratum: '0'
proof: A round trip from source to IR and back to source is semantically identical
---

Desugaring, name resolution, and the point at which `@n` annotations become
checkable constraints rather than syntax.

## 2026-09-12

The round trip needed a printer, so there is one: the IR written back out as Nether C, with the folds undone. A branch becomes an if again, a select whose arm is a constant becomes the && or || it came from, a loop becomes a for. It is not a formatter — every expression is parenthesised, because the round trip is about meaning and a printer that leans on section 4.6 to rebuild the shape is proving section 4.6 rather than lowering.

## 2026-09-12

Semantically identical is not textually identical and should not be. What has to survive is the IR: source that lowers to an IR that prints to source that lowers to the same IR is source nothing was lost from, and the two are compared node for node with only the spans forgotten.

## 2026-09-12

Every unit is also handed to the checker, which derives the same depths by its own route. That is two implementations of section 2.2 agreeing rather than one agreeing with itself, and it is the same argument as the generator in 0052.

## 2026-09-12

Two things the round trip caught. The printer was writing an @0 on every signature, which invents an assertion the source did not make — only what was written is written back. And the loop guard lowering synthesises had a bare break where source produces a block, so the same program written two ways lowered two ways.

## 2026-09-12

Filed 0133: a Shade parameter's origin is inferred from nothing, and section 4.3 says it is always inferred. Lowering gives it stratum 8, so looking at one is legal nowhere — useless and sound, which beats useful and wrong.
