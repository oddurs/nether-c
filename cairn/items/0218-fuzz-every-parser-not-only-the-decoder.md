---
id: 218
title: Fuzz every parser, not only the decoder
type: chore
status: unmarked
milestone: warden
depends_on:
- 216
created: 2026-09-13
updated: 2026-09-13
priority: p0
effort: m
area: crates/nether-syntax
stratum: '0'
proof: The lexer, the parser and the cairn parser each soak with no panic and no unbounded allocation
---

The decoder is fuzzed and `Cairn::from_str` is fuzzed, and both of those were
added because a bug was found in them.

The lexer and the parser read a stranger's source. They have a nesting limit,
which is the right instinct, and nothing has ever thrown ten million bytes of
malformed UTF-8 at them.
