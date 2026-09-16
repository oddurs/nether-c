---
id: 253
title: A Bytes literal cannot spell every byte
type: bug
status: descending
assignee: Oddur Sigurdsson
claimed: 2026-09-16
created: 2026-09-16
updated: 2026-09-16
priority: p1
effort: s
stratum: 0
area: spec/03-lexical.md
proof: A residue holding any byte prints, parses and lowers back to the same program
---

## What happens

`demand b"é";` buries to the two bytes `c3 a9`. The printer has no spelling for
a byte above `0x7e`, so it writes `\u{c3}\u{a9}`, and `\u{…}` names a scalar
value — §3.6 says so. Lexing it back gives `Ã©`, which is four bytes, and
burying that residue gives a different program again.

```
  left: "demand b\"\\u{c3}\\u{a9}\";\n"
 right: "demand b\"\\u{c3}\\u{83}\\u{c2}\\u{a9}\";\n"
```

That is `spec/06-evaluation.md` §6.5's MUST broken by a two-line program:

> An implementation MUST be able to print any residue it can produce, and
> lowering what it printed MUST give back the same program.

## What should happen

The residue is the same program the second time, and the staging law with an
empty `B` holds for a `Bytes` value whatever bytes are in it.

§3.6 already makes this argument for the cairn literal — *a value the language
cannot spell is a residue that does not parse* — and then leaves `Bytes` with
the same hole. The escape production needs a byte escape, and a source file is
UTF-8 (§3.1), so a raw `0xe2` cannot be the answer.

## Reproduction

1. `residue_survives("demand b\"\u{e9}\";\n")` in
   `crates/nether-bury/tests/residue_is_source.rs`.

## Cairn of the offending trace

Not a trace: the residue is well-formed and it is the printing that is wrong.

## Fix boundary and regression proof

Two PRs. The specification first, because §3.6 has no production for this and
`CLAUDE.md` says the section comes before the compiler:

1. `spec/03-lexical.md` §3.6 gains a byte escape, admissible in a
   `bytes_literal` and not in a `str_literal` — a `Str` is UTF-8 by §5.1 and a
   byte escape there would spell values it cannot hold. The rejected
   alternatives go in `spec/90-rationale.md`.
2. `crates/nether-syntax`: the lexer reads it, the printer writes it for every
   byte it cannot write literally, and `residue_is_source.rs` gets the case
   above. Nothing else changes: `\u{…}` keeps meaning a scalar value in both
   kinds of literal.
