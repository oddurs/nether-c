---
id: 54
title: 'nether-syntax: the lexer'
type: feature
status: unmarked
milestone: surface
depends_on:
- 17
created: 2026-09-10
updated: 2026-09-10
priority: p0
effort: m
area: crates/nether-syntax
stratum: '0'
proof: Every token in spec section 03 is produced, with correct spans
---

UTF-8 in, tokens with byte spans out. Spans are load-bearing: every
diagnostic and every hole in the language points back at source.
