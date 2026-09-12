---
id: 131
title: 'Spec: section 3.6 does not say what an integer literal may hold'
type: spec
status: buried
milestone: surface
assignee: Oddur Sigurdsson
depends_on:
- 17
created: 2026-09-12
updated: 2026-09-12
priority: p1
effort: s
area: spec/03-lexical.md
proof: Every integer an I64 holds has a literal, and every literal section 3.6 admits is one an I64 holds
---

## The hole

`spec/03-lexical.md` §3.6 gives three spellings for an integer and one
sentence about what one means:

> An integer literal is `I64`.

It does not say what happens at the edges, and there are three of them:

- `9223372036854775808` — one past `I64`'s maximum. An error, presumably.
- `0xFFFF_FFFF_FFFF_FFFF` — sixteen digits, which is a perfectly ordinary
  thing to write for a mask and is not a number an `I64` holds as a magnitude.
- `−2⁶³` itself, which cannot be written at all if a decimal literal tops out
  at `I64`'s maximum: `-` is an operator applied to a literal, and the literal
  it would need is one past the top. `spec/05-types.md` §5.1 says the type
  holds it.

Two implementations reading §3.6 can differ on all three, and one of them can
be unable to write the smallest number the type has.

## What the lexer does, pending a decision

A decimal literal spells a magnitude, so it is an error above `2⁶³ − 1`. A
hexadecimal or binary literal spells a bit pattern, so it runs the full
sixty-four bits and is read as two's complement, which makes
`0x8000_0000_0000_0000` the way to write `−2⁶³`.

That is what C does and it is the only reading found that leaves every `I64`
writable. It is in the lexer because a lexer has to pick; it is filed because
picking is not the lexer's job.

## Acceptance criteria

- [x] §3.6 says what each of the three spellings may hold
- [x] Every value of `I64` has a literal that denotes it
- [x] `spec/90-rationale.md` records the alternative, if the answer is not the obvious one

## 2026-09-12

Settled the way the lexer already had it, because it is the only reading found that leaves every I64 writable. A decimal spells a magnitude and tops out at 2^63-1; a hexadecimal or binary spells a bit pattern and runs the whole width, which is what makes 0x8000000000000000 the way to write -2^63 in a language whose minus is an operator.

## 2026-09-12

Section 90.2 records the alternative: one range for all three, which leaves -2^63 unwritable, and the variant that lets a decimal reach 2^63 exactly, which makes a bare 9223372036854775808 in range as a literal and out of range as a value. Nothing else in the language makes that distinction.
