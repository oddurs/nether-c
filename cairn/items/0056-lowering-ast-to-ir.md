---
id: 56
title: 'Lowering: AST to IR'
type: feature
status: unmarked
milestone: surface
depends_on:
- 46
- 55
created: 2026-09-10
updated: 2026-09-10
priority: p0
effort: m
area: crates/nether-syntax
stratum: '0'
proof: A round trip from source to IR and back to source is semantically identical
---

Desugaring, name resolution, and the point at which `@n` annotations become
checkable constraints rather than syntax.
