---
id: 17
title: 'Spec: lexical structure'
type: spec
status: descending
milestone: codex
created: 2026-09-10
updated: 2026-09-10
priority: p1
effort: m
area: spec/03-lexical.md
proof: The lexer in nether-syntax is generated from, or checked against, this section
---

## What this section must answer

Source encoding, tokens, literals, comments, identifiers, and the depth
annotation syntax (`@n`). Terry's 8-bit ASCII is a deliberate non-inversion:
Nether C is UTF-8, because a ledger that cannot record a name is not a ledger.

## Acceptance criteria

- [ ] Full token table
- [ ] Literal grammar for integers, bytes and strings
- [ ] Stated rule for where `@n` may appear

## 2026-09-10

Draft landed: spec/03-lexical.md. UTF-8 rather than HolyC's 8-bit ASCII, stated as a deliberate non-inversion. No floats, and the reason is canonical encoding.
