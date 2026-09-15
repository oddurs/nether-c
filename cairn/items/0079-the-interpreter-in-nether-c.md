---
id: 79
title: The interpreter, in Nether C
type: feature
status: buried
milestone: futamura
assignee: Oddur Sigurdsson
depends_on:
- 23
- 59
created: 2026-09-10
updated: 2026-09-15
priority: p1
effort: xl
area: lib/
stratum: '5'
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

## 2026-09-15

The unchanged sample-program proof now passes through one Nether C interpreter: hello.nc deposits its greeting as Str; build.nc produces the real read(main.nc) question and resumes to obj: plus the supplied bytes; stamp.nc is rejected at the guest look byte offset with origin 5 and ambient 0, before any network question. Eighteen burial tests cover forward declarations, renamed bindings, typed parameters, unused network calls, Unicode and packet delimiters, refusals, call-depth propagation, repaired and too-shallow looks, source bytes arriving through a hole, and printed-residue reburial after staged source/file answers. Parsing, checking and evaluation of guest bytes are all in lib/interpreter.nc and lib/evaluate.nc. The host only buries that Nether C unit and supplies recorded answers. scripts/task check passes in full; the core remains at 8017 lines. The sample bootstrap boundary and remaining general-language work are explicit in lib/README.md: operators/control flow/aggregates, recursive checking, inferred latent signatures, memoised guest globals, runtime seal and guest fuel accounting are not claimed by this proof. Later self-burial projections remain separate items. Stratum 5 is prelude dispatch; these proofs use supplied ledger answers and perform no network I/O.
