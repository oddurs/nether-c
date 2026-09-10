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

> HolyC used 8-bit ASCII throughout its toolchain, deliberately. This is one
> of the few places Nether C does not invert: a ledger that cannot record a
> name is not a ledger, and half the names in the world need more than 256
> code points.

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
normalisation. An implementation MUST normalise before comparison and MUST
store the normalised form in the ledger, so that two spellings of the same
name cannot produce two different cairns.

## 3.4 Keywords

Reserved, and never usable as identifiers:

```
descend   seal    shade   look    opaque   demand
if        else    while   for     return   break    continue
struct    typedef sizeof  true    false
```

Type names in the prelude (`U0`, `I64`, `Bytes`, `Str`, `Cairn`, `Shade`,
`Json`, `Bool`) are ordinary identifiers bound in the prelude scope, not
keywords. A program MAY shadow them, and SHOULD NOT.

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

str_literal    := '"' ( str_char | escape )* '"'
bytes_literal  := "b" str_literal

escape         := "\\" ( "n" | "t" | "r" | "0" | "\\" | '"' | "u{" hexdigit+ "}" )
```

There are no floating-point literals. Floating-point arithmetic is not in the
language, because IEEE 754 has platform-observable behaviour that would make
the canonical encoding of [section 07](07-ledger.md) unsound, and a
reproducibility guarantee with an asterisk on it is not a guarantee. A future
version MAY add a rational or fixed-point type; it will not add binary floats
without a stated evaluation semantics.

An integer literal is `I64`. There is no unsigned type and no integer
promotion; see [section 05](05-types.md).

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
  second, worse burial.
- No `goto`.
- No pointer syntax. See [section 05](05-types.md).
