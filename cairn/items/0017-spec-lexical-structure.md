---
id: 17
title: 'Spec: lexical structure'
type: spec
status: buried
milestone: codex
created: 2026-09-10
updated: 2026-09-10
priority: p1
effort: m
area: spec/03-lexical.md
proof: Every terminal the grammar in section 04 references is defined in section 03
---

## What this section must answer

Source encoding, tokens, literals, comments, identifiers, and the depth
annotation syntax (`@n`). Terry's 8-bit ASCII is a deliberate non-inversion:
Nether C is UTF-8, because a ledger that cannot record a name is not a ledger.

## Acceptance criteria

- [x] Full token table
- [x] Literal grammar for integers, bytes and strings
- [x] Stated rule for where `@n` may appear

## 2026-09-10

Draft landed: spec/03-lexical.md. UTF-8 rather than HolyC's 8-bit ASCII, stated as a deliberate non-inversion. No floats, and the reason is canonical encoding.

## 2026-09-10

Proof restated. The original — 'the lexer in nether-syntax is generated from, or checked against, this section' — cannot be met during the spec phase because there is no lexer, and burying an item on a proof that cannot hold is exactly what the proof field exists to prevent. Implementation-phase verification belongs to 0054. Checking the restated proof found a real gap: the grammar used `digit` and section 03 never defined it. Fixed, along with `hexdigit`.
