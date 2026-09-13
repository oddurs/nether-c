---
id: 173
title: A cairn literal, in the lexer, the parser and the printer
type: feature
status: buried
milestone: world
assignee: Oddur Sigurdsson
depends_on:
- 172
created: 2026-09-13
updated: 2026-09-13
priority: p0
effort: m
area: crates/nether-syntax
stratum: '0'
proof: A residue that seals something parses, and round-trips
---

§3.6 now gives a cairn a literal form. Nothing reads or writes it.

```console
$ nether lamp <residue of a program that seals something>
demand «a cairn has no syntax»;
```

`crates/nether-syntax/src/print.rs` emits that placeholder because there was
nothing to emit. There is now: `#` and sixty-four lowercase hex digits.

## Where

- `lexer.rs` — a token, refusing the near misses by hand. Sixty-three digits,
  sixty-five, an underscore and an uppercase letter each need a diagnostic
  that says which, because "unexpected character" over a sixty-five character
  token is a message nobody can act on.
- `parser.rs` and the AST — a primary expression, like any other literal.
- `lower.rs` — a `Literal::Cairn`, which already exists.
- `print.rs` — the placeholder goes.

## Acceptance criteria

- [x] A residue that seals something parses, and round-trips
- [x] Every near miss in §3.6 is refused by name
- [x] A cairn printed by `nether lamp` pastes into a program with a `#`

## 2026-09-13

The literal is #<64 lowercase hex>, the spelling the rites print. The lexer refuses each near miss by name -- too short, too long, a capital, a separator, a letter past f -- because a cairn is sixty-five characters and a message that makes the reader count them is no message.

## 2026-09-13

#include and #exe now answer from the cairn's own diagnostic rather than 'this starts nothing', which is what §3.8 says that paragraph is for.
