---
id: 79
title: The interpreter, in Nether C
type: feature
status: descending
milestone: futamura
assignee: Oddur Sigurdsson
claimed: 2026-09-15
depends_on:
- 23
- 59
created: 2026-09-10
updated: 2026-09-15
priority: p1
effort: xl
area: lib/
stratum: '0'
proof: The Nether C interpreter, written in Nether C, runs the spec's sample programs
---

Written against the prelude, with the source bytes as a hole. The
precondition for everything below it.

## 2026-09-15

Started the self-hosted interpreter in lib/interpreter.nc. The first layer is a recursive Bytes cursor (whitespace, line comments, search, and literal escape decoding), compiled as Nether C. A burial proof now gives it the exact hello.nc source bytes and recovers Hello from the nether\\n without a host parser examining those bytes. While proving recursion, fixed burial so return values and control-flow branches carrying a return are not incorrectly recorded as deposits. 079 remains descending: environments and calls, block comments and diagnostics, residual prelude questions for build.nc, and Orpheus checking for stamp.nc are still required before the item can be buried.

## 2026-09-15

Review correction: the earlier literal_demand helper only searched for a keyword and the first string; it did not interpret hello.nc. It has been removed. The replacement parses U0 functions without parameters, resolves named calls and demands, validates syntax before evaluation, and deposits Str values. Tests cover exact hello.nc, renamed/reordered functions, ignored functions, misleading comments, malformed/unsupported syntax, and source bytes supplied through a read hole. Added utf8 folding and nested-return deposit regressions to burial. The trusted-core ceiling rises by seven lines for these required semantics. This is a tested subset, not completion: build.nc requires values, environments, function arguments and residual prelude dispatch; stamp.nc requires static type/depth/Orpheus validation. General grammar, Unicode identifiers and structured diagnostics also remain. Keep 0079 descending and keep its proof unchanged.

## 2026-09-15

Verification: Rust tests, lint, build, site/graphics/font checks and the WASM budget pass. The final formatted core is 8017 lines: an eight-line increase over 8009 for utf8 folding and correct return/deposit handling, reflected in .decay-ceiling. The executable subset remains explicitly incomplete for the item proof.
