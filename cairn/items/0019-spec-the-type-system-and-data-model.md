---
id: 19
title: 'Spec: the type system and data model'
type: spec
status: descending
milestone: codex
created: 2026-09-10
updated: 2026-09-10
priority: p1
effort: l
area: spec/05-types.md
proof: Every type in the prelude has a stated canonical encoding
---

## What this section must answer

Base types, structs, arrays, `Bytes`, `Cairn`, `Shade<T>`, and how a
type relates to its canonical encoding in the ledger.

## Constraints it inherits

Terry made everything an I64 and let it coerce. We invert: nothing coerces,
and a value's type carries its depth. Watch that this does not make ordinary
arithmetic unwritable.

## Acceptance criteria

- [ ] The relationship between a type and its canonical encoding is total
- [ ] Structural vs nominal equality is decided and stated

## 2026-09-10

Draft landed: spec/05-types.md. No pointers at all — content addressing requires it. Mutation is permitted only before a value is first read.
