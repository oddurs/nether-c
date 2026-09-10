---
id: 18
title: 'Spec: the grammar'
type: spec
status: descending
milestone: codex
created: 2026-09-10
updated: 2026-09-10
priority: p1
effort: l
area: spec/04-grammar.md
proof: The grammar is unambiguous and a recursive descent parser can be written from it directly
---

## What this section must answer

Complete EBNF. C-shaped: declarations, structs, functions, statements,
expressions — plus `descend`, `seal`, `shade`, `look`, `opaque`
and `demand`.

## Acceptance criteria

- [ ] EBNF for the whole language, no prose-only constructs
- [ ] Precedence and associativity table
- [ ] Every sample program in the spec parses under it

## 2026-09-10

Draft landed: spec/04-grammar.md. Full EBNF and a precedence table. descend is an expression with a tail value.
