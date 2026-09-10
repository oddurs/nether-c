---
id: 18
title: 'Spec: the grammar'
type: spec
status: buried
milestone: codex
created: 2026-09-10
updated: 2026-09-10
priority: p1
effort: l
area: spec/04-grammar.md
proof: Every construct the specification uses has a production, and every operator it defines has a precedence entry
---

## What this section must answer

Complete EBNF. C-shaped: declarations, structs, functions, statements,
expressions — plus `descend`, `seal`, `shade`, `look`, `opaque`
and `demand`.

## Acceptance criteria

- [x] EBNF for the whole language, no prose-only constructs
- [x] Precedence and associativity table
- [x] Every sample program in the spec parses under it

## 2026-09-10

Draft landed: spec/04-grammar.md. Full EBNF and a precedence table. descend is an expression with a tail value.

## 2026-09-10

Proof restated. 'A recursive descent parser can be written from it directly' is not checkable until somebody writes one; that is 0055's proof. What is checkable now is completeness of the grammar against the rest of the document.
