---
id: 54
title: 'nether-syntax: the lexer'
type: feature
status: buried
milestone: surface
assignee: Oddur Sigurdsson
depends_on:
- 17
created: 2026-09-10
updated: 2026-09-12
priority: p0
effort: m
area: crates/nether-syntax
stratum: '0'
proof: Every token in spec section 03 is produced, with correct spans
---

UTF-8 in, tokens with byte spans out. Spans are load-bearing: every
diagnostic and every hole in the language points back at source.

## 2026-09-12

The keyword and punctuation tables are read back out of sections 3.4 and 3.7 and compared, so a token added to the specification fails the proof until the lexer has one and one removed fails it until the lexer does not. The same trick as the prelude table in 0046 and the eleven rules in 0047.

## 2026-09-12

Section 3.5's rule that whitespace is permitted between @ and a digit in a signature and forbidden in a type is a lexer rule, not a parser one. @3 with nothing between is one Depth token; @ 3 is punctuation and a number. The parser accepts either after a signature and only the first after a type, which is what makes Bytes@3 unambiguously one token sequence.

## 2026-09-12

Filed 0131. Section 3.6 says an integer literal is I64 and nothing about the edges, and one of them is that -2^63 has no spelling if a decimal tops out at the maximum. The lexer reads decimal as a magnitude and hex and binary as bit patterns, which is what C does and the only reading found that leaves every I64 writable.
