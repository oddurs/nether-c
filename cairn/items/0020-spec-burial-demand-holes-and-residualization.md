---
id: 20
title: 'Spec: burial, demand, holes and residualization'
type: spec
status: descending
milestone: codex
created: 2026-09-10
updated: 2026-09-10
priority: p0
effort: l
area: spec/06-evaluation.md
proof: The evaluation order is deterministic and stated well enough to reimplement
---

## What this section must answer

The heart of it. What burial does, what a hole is, how `demand` drives
evaluation, and exactly what the residual program contains.

## Acceptance criteria

- [ ] Demand-driven evaluation order, stated deterministically
- [ ] What a hole records: the call, the stratum, the dependency graph, the span
- [ ] What exhumation does to a trace, and why the result is a new cairn
- [ ] Replay semantics: what "identical" means, formally

## 2026-09-10

Draft landed: spec/06-evaluation.md. Burial, demand, holes, fuel, opaque, residue, the staging law, exhumation and replay. The staging law is what makes graft sound.
