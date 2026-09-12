---
id: 118
title: 'Spec: Json is bound in the prelude scope and never defined'
type: spec
status: unmarked
milestone: surface
depends_on:
- 19
created: 2026-09-12
updated: 2026-09-12
priority: p2
effort: s
area: spec/03-lexical.md
proof: Every type name section 3.4 claims the prelude binds is defined in section 05 or section 09
---

## The hole

`spec/03-lexical.md` §3.4 says:

> Type names in the prelude (`U0`, `I64`, `Bytes`, `Str`, `Cairn`, `Shade`,
> `Json`, `Bool`) are ordinary identifiers bound in the prelude scope.

`Json` is not in `spec/05-types.md` §5.1, not in `spec/09-prelude.md`, and no
prelude function produces or consumes one. It appears once more, in
`spec/01-strata.md` §1.6, as `Shade<Json>` in the example that introduces the
Orpheus rule — so the example names a type the language does not have.

## What this must decide

Either the prelude has a `Json` type and §09 says what it is and what parses
into it, or it does not and two sentences change: §3.4's list loses a name and
§1.6's example uses `Bytes`.

The second is much more likely to be right. §09 says the prelude should be
readable in full in ten minutes, and a JSON model is not a ten-minute type.

## Acceptance criteria

- [ ] §3.4's list and §09 name the same set
- [ ] §1.6's example uses a type that exists
