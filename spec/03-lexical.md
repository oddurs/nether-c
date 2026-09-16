---
section: "03"
title: Lexical structure
status: draft
---

# Lexical structure

## 3.1 Source encoding

A Nether C source file is a sequence of Unicode scalar values encoded as
UTF-8. An implementation MUST reject a file that is not well-formed UTF-8,
rather than substituting replacement characters.

> A ledger must be able to record names, not just the ones that fit in an
> eight-bit character set.

Line terminators are `U+000A`. A `U+000D` immediately preceding `U+000A` is
discarded; a `U+000D` anywhere else is an error. Source files SHOULD end with
a line terminator.

## 3.2 Comments

```c
// to the end of the line
/* to the closing delimiter; these do not nest */
```

Comments are whitespace. They are not preserved in the AST and do not appear
in the ledger.

## 3.3 Identifiers

```
identifier := XID_Start XID_Continue*
```

Identifiers are compared by exact code-point sequence after NFC
normalisation, and an implementation MUST normalise an identifier before
comparing or recording it, so that two spellings of one name cannot resolve to
two different bindings.

Normalisation happens **here**, in the lexer, and nowhere below it. The ledger
stores the bytes it is handed and does not alter them
([§7.1](07-ledger.md#71-canonical-encoding) rule 6): a `Str` that a program
constructs from arbitrary input is that program's business, and a store that
quietly rewrites it would be addressing something the caller never wrote.

## 3.4 Keywords

Reserved, and never usable as identifiers:

```
descend   seal    shade   look    opaque   demand
if        else    while   for     return   break    continue
struct    typedef sizeof  true    false
```

`sizeof` is reserved and has no meaning. [§4.5](04-grammar.md#45-expressions)
has no production for it and nothing in this specification says what it would
evaluate to: there is no pointer type, no allocation and no memory layout, so
there is nothing for it to measure. It stays reserved so that a program cannot
bind the word to something that is not what a reader would assume, and writing
one is an error that names `len`. [§90.2](90-rationale.md#902-rejected-alternatives).

Type names in the prelude (`U0`, `I64`, `Bytes`, `Str`, `Cairn`, `Shade`,
`Answer`, `Refusal`, `Bool`) are ordinary identifiers bound in the prelude
scope, not keywords. A program MAY shadow them, and SHOULD NOT.

## 3.5 Depth annotations

```
depth_annotation := "@" ( "0".."8" )
```

A depth annotation may appear:

- after a type, binding tighter than anything else: `Bytes@3`, `I64@0`;
- after a function signature, giving its latent depth: `Bytes read(Str p) @3`.

It MUST NOT appear anywhere else. In particular there is no depth annotation
on an expression — depth is inferred for expressions and only ever *asserted*
on declarations. An annotation that disagrees with inference is an error, not
a coercion.

Whitespace is permitted between `@` and the digit in a signature annotation
and forbidden in a type annotation, so that `Bytes@3` is unambiguously one
token sequence.

## 3.6 Literals

```
digit          := "0".."9"
hexdigit       := digit | "a".."f" | "A".."F"

int_literal    := dec | hex | bin
dec            := digit ( digit | "_" )*
hex            := "0x" hexdigit ( hexdigit | "_" )*
bin            := "0b" ( "0" | "1" ) ( "0" | "1" | "_" )*

bool_literal   := "true" | "false"

cairn_literal  := "#" lowerhex{64}
lowerhex       := digit | "a".."f"

str_literal    := '"' ( str_char | escape )* '"'
bytes_literal  := "b" '"' ( str_char | escape | byte_escape )* '"'

escape         := "\\" ( "n" | "t" | "r" | "0" | "\\" | '"' | "u{" hexdigit+ "}" )
byte_escape    := "\\x" hexdigit hexdigit
```

There are no floating-point literals. Floating-point arithmetic is not in the
language, because IEEE 754 has platform-observable behaviour that would make
the canonical encoding of [section 07](07-ledger.md) unsound, and a
reproducibility guarantee with an asterisk on it is not a guarantee. A future
version MAY add a rational or fixed-point type; it will not add binary floats
without a stated evaluation semantics.

An integer literal is `I64`. There is no unsigned type and no integer
promotion; see [section 05](05-types.md).

A cairn literal is `Cairn`, and is exactly sixty-four lowercase hexadecimal
digits after the `#`. Not sixty-three, not sixty-five, no `_` separators, and
no uppercase: a cairn is a name, two spellings of one name are two names, and
[§7.1](07-ledger.md#71-canonical-encoding) refuses the same thing one level
down for the same reason.

It is the spelling the rites print. A name copied out of `nether lamp` or
`nether cairn` pastes into a program with a `#` in front of it and means the
same thing there. There is no short form — a prefix is something a person types
at a terminal, where a ledger is present to resolve it, and a program has no
ledger while it is being read.

The literal exists because [§6.5](06-evaluation.md#65-residue) requires a
residue to be a program. A burial that folds `seal` has a cairn to write down,
and a value the language cannot spell is a residue that does not parse.

`\x` is in `bytes_literal` for the same reason and not in `str_literal` for a
different one. A source file is UTF-8 ([§3.1](#31-source-encoding)), so the byte `0xe2`
cannot be written on its own, and `\u{e2}` is the character U+00E2 — two bytes
when it is encoded, which is not what a `Bytes` value holding one byte wants
said about it. `\xe2` is that byte. It takes exactly two hexadecimal digits,
because a variable-length one makes `\xef` followed by the letter `f` two
different literals depending on how far the reader is willing to look.

A `Str` is UTF-8 ([§5.1](05-types.md#51-base-types)), so a byte escape there
would spell values the type cannot hold, and the two escapes would then mean
different things in the two literals. `\u{…}` means a scalar value in both.

The three spellings do not hold the same range, because they are not spelling
the same thing.

- A **decimal** literal spells a magnitude. It is an error above `2⁶³ − 1`.
- A **hexadecimal** or **binary** literal spells a bit pattern. It is
  sixty-four bits wide and is read as two's complement, so
  `0x8000_0000_0000_0000` is `−2⁶³` and `0xFFFF_FFFF_FFFF_FFFF` is `−1`. It is
  an error above sixty-four bits.

That split is what makes every `I64` writable. `-` is an operator applied to a
literal, so `−2⁶³` has no decimal spelling at all — the literal it would need
is one past the top — and a type with a value nobody can write is a type with
a hole in it.

## 3.7 Punctuation

```
( ) { } [ ] ; , . -> @
+ - * / % ! ~ & | ^ << >>
= == != < <= > >= && ||
+= -= *= /= %= &= |= ^= <<= >>=
```

## 3.8 Tokens the language does not have

For the avoidance of doubt, and because a C programmer will look for them:

- No preprocessor. No `#include`, no `#define`, no `#if`, and in particular no
  `#exe`. Everything `#exe` did, burial does — see
  [section 06](06-evaluation.md) — so a second mechanism for it would be a
  second, worse burial. The `#` is not free for one either way: §3.6 spends it
  on the cairn literal, which is the nearest thing this language has to the
  thing `#include` was reaching for and arrives at it from the other end.
- No `goto`.
- No pointer syntax. See [section 05](05-types.md).
