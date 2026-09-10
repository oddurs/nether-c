---
id: 23
title: 'Spec: the prelude'
type: spec
status: descending
milestone: codex
created: 2026-09-10
updated: 2026-09-10
priority: p2
effort: m
area: spec/09-prelude.md
proof: The prelude is small enough to read in full in ten minutes
---

## What this section must answer

The minimal standard library: what is available at depth 0, and the signature
of every world-touching function with the stratum it costs.

## Acceptance criteria

- [ ] Every prelude function has a stratum in its signature
- [ ] Nothing in the prelude reaches deeper than it must

## 2026-09-10

Draft landed: spec/09-prelude.md. Every signature carries its stratum. 9.9 admits failure handling is under-specified and needs an item of its own before The Surface.
