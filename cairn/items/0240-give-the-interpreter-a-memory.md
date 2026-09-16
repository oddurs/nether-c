---
id: 240
title: Give the interpreter a memory
type: feature
status: buried
milestone: futamura
assignee: Oddur Sigurdsson
depends_on:
- 79
created: 2026-09-15
updated: 2026-09-15
priority: p1
effort: l
area: lib/
stratum: '5'
proof: Repeated and diamond-shaped global dependencies deposit once, cyclic globals report their name, all sample and staged-hole proofs pass, and fixed-input burial spends fewer steps than the bootstrap.
---

Index declarations once. Thread an explicit immutable evaluation memory through
expressions, calls and demands; cache globals on first use, marking those still
being evaluated so cycles have a diagnostic. Keep checking memory separate from
evaluation memory. Separate the record representation from language semantics,
and consolidate repeated result/depth operations behind small named functions.

This is a refinement of the sample bootstrap, not the later self-burial
projections. Preserve the original sample proof and the existing external
result encoding.

## 2026-09-15

Replaced repeated declaration scans with a unit index and explicit immutable evaluation memory. A global moves absent -> Pending -> value; repeated and diamond dependencies now deposit once, and cycles report the binding name. Kept checking, runtime, local scopes and separate interpretations isolated. Split the record codec into value.nc, separated rites/prelude dispatch from source traversal, unified return-tail checking, bounded token comparisons to the bytes needed, and preserved the external demand-result encoding. Added 9 tests (27 total), including resumed shared globals, signed codec extremes, binary payloads and invalid UTF-8; strengthened duplicate-parameter, refusal and malformed-depth regressions. Fixed-input host steps fall from 1300946/20340206/7759045 to 1064424/4654746/1885775 for hello/build/stamp, with regression budgets. scripts/task check passes; trusted core stays 8017 lines and compressed WASM 147645 bytes. No new dependency or world capability. Stratum 5 remains recorded read/get dispatch with supplied answers in tests.
