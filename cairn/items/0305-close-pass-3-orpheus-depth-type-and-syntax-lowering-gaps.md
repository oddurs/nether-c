---
id: 305
uid: c1af476c-1fb6-471c-b73b-95148776481d
title: Close Pass-3 Orpheus, depth-type, and syntax lowering gaps
type: bug
status: buried
milestone: calculus
created: 2026-11-23
updated: 2026-11-23
closed_at: 2026-11-23
priority: p1
area: nether-core,nether-syntax
effort: m
stratum: '0'
proof: cargo test --workspace and scripts/task check pass with Pass-3 core and syntax regressions
---

## What happens

## What should happen

## Reproduction

1.

## Cairn of the offending trace

## Fix boundary and regression proof

Name the smallest affected path, the failing test before the fix, and the
verification after it. Preserve unrelated behavior.

## 2026-11-23

Replaced shades_agree with recursive depth_types_agree and carries_depth_type in nether-core to enforce nominal Struct, Fn latent/result_depth/arity, container Shade invariants, non-returning FuncDef fall-off, Call arity, and exact Prim::Answered/Must shapes; fixed Call argument freezing order and Loop fixpoint inner-local reset. Hardened nether-syntax lowering with non-lexical compound-assign index temps, user alias/struct shadowing before prelude types, required @d on received Shade<T@d>, typedef asserted-depth propagation, direct-only fill_shades, bare polymorphic prelude rejection, and Fn call arity enforcement.
