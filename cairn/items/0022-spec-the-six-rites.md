---
id: 22
title: 'Spec: the six rites'
type: spec
status: buried
milestone: codex
created: 2026-09-10
updated: 2026-09-10
priority: p1
effort: m
area: spec/08-rites.md
proof: Every documented invocation exists as a transcript test
---

## What this section must answer

The exact contract of `bury`, `exhume`, `lamp`, `cairn`, `strata`
and `graft`: arguments, output shape, exit codes, and what each writes to
the ledger.

Also: why there is no `run`, stated once, in the spec, so it stops being a
joke and becomes a rule.

## Acceptance criteria

- [x] Exit code table
- [x] Machine-readable output (--json) specified for every rite
- [x] The output of a burial is specified: depth, holes, node count

## 2026-09-10

Draft landed: spec/08-rites.md. All six rites, --json for each, and an exit code table. 8.0 makes the absence of run the only frozen requirement in the draft.
