---
id: 307
uid: c006bdf5-e92f-44f7-ba53-ab427f619711
title: Close Pass-4 prim, control-flow, DAG, and syntax gaps
type: bug
status: buried
milestone: calculus
created: 2026-11-24
updated: 2026-11-24
closed_at: 2026-11-24
priority: p0
area: nether-core,nether-syntax
effort: m
stratum: '0'
proof: cargo test --workspace and scripts/task check pass with Pass-4 core and syntax regressions
---

## What happens

## What should happen

## Reproduction

1.

## Cairn of the offending trace

## Fix boundary and regression proof

Name the smallest affected path, the failing test before the fix, and the
verification after it. Preserve unrelated behavior.

## 2026-11-24

Enforced signature checks on all 32 Prim forms and U0 typing/scoping on Loop/Break/Continue/Return in nether-core, fixed block_always_returns with may_jump_loop, removed exponential backtracking in carries_depth_inner, and checked struct Field::asserted on Assign. Bounded flat binary/postfix/array chains in nether-syntax parser, propagated typedef asserted depths to returns and Shade<Typedef> while rejecting conflicts, rejected omitted Shade origins in refused(absent), and restricted print::short_circuit to expression-printable Bool arms.
