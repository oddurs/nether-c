---
section: "05"
title: Types and the data model
status: draft
---

# Types and the data model

## 5.1 Base types

| Type | Values | Canonical encoding |
| --- | --- | --- |
| `U0` | exactly one, written `()` | the empty sequence |
| `Bool` | `true`, `false` | one byte, `0x00` or `0x01` |
| `I64` | −2⁶³ … 2⁶³−1 | fixed 8 bytes, two's complement, big-endian |
| `Bytes` | any finite byte string | length prefix, then the bytes |
| `Str` | any well-formed UTF-8 string | as `Bytes`, over the bytes as given |
| `Cairn` | a content address | 32 bytes |
| `Shadeᵈ⟨T⟩` | an opaque `T` from depth `d` | the cairn of the underlying value |
| `Answer⟨T⟩` | what the world said: a `T`, or a refusal | tag byte, then the `T` or the `Refusal` |
| `Refusal` | one of six codes, and nothing else | one byte |

`I64` is the only integer type. There is no unsigned type, no `char`, no
integer promotion and no implicit narrowing. Arithmetic wraps; an
implementation MUST NOT make overflow undefined, because a canonical encoding
cannot be built on top of behaviour that varies by compiler.

> HolyC made everything an `I64` and let everything coerce into everything.
> Nether C keeps the one integer type and removes every coercion. The
> inversion is not the width; it is the silence.

## 5.1.1 Answers and refusals

Nether C has no exceptions and no error type. It has `Answer⟨T⟩`, which is what
a function returns when the world is entitled to say no.

```
Answer⟨T⟩  ::=  Given T  |  Refused Refusal

Refusal    ::=  absent       the thing is not there
             |  denied       it is there and you may not have it
             |  malformed    it is there and it is not what it claims to be
             |  unreachable  nothing answered
             |  exhausted    a limit was reached: space, quota, size
             |  conflict      something else changed it first
```

The set of refusal codes is **closed** and fixed by this specification. An
implementation MUST NOT add to it. The six names are bound in the prelude scope
and compare by equality:

```c
if (refusal(a) == absent) { /* ... */ }
```

A `Refusal` carries a code and nothing else — no message, no platform error
number, no path. Those things vary between operating systems, and a value whose
encoding varies between operating systems cannot have a stable cairn, which
would put a hole in [section 07](07-ledger.md) large enough to sink every
reproducibility claim in this document.

The detail is not lost. It is recorded in the **witness**, alongside everything
else the world said ([§1.4](01-strata.md#14-what-the-trace-records)), where
`nether lamp` can show it to a person and no program can branch on it.

> A program may act on *what kind* of no it received. It may not act on how a
> particular kernel chose to phrase it.

## 5.2 Aggregates

```c
struct Header {
  I64   len;
  Bytes tag;
};

typedef I64[16] Row;
```

Structs are nominal: two structs with identical fields are different types.
Arrays are fixed-length when the length is written and slices when it is not.

There is no union type, and no pointer type at all. A value is reached by
name, index or field; there is no way to name a location. This is not a safety
feature bolted on — [section 07](07-ledger.md) requires every value to have a
content address, and an address that depends on where a value happens to live
is not a content address.

## 5.3 Equality

Equality is **structural** and total: two values of the same type are equal
exactly when their canonical encodings are equal, which is exactly when their
cairns are equal.

An implementation MUST compare by cairn. This makes deep equality a 32-byte
comparison, which is the main practical payoff of content addressing and is
worth stating as a requirement rather than leaving as an optimisation.

`Shade` is comparable: two shades are equal when their underlying values are.
Comparing shades does not count as looking at them, because it reveals only
what `seal` already revealed.

## 5.4 Mutation

There is none, observably.

A binding may not be reassigned. A local aggregate may be built up
field-by-field before it is first used as a value, and an implementation
SHOULD compile that to in-place writes, but no program can observe the
difference: once a value has been read, its cairn exists, and nothing can
change what a cairn names.

```c
U0 build()
{
  Header h;
  h.len = 3;          // legal: h has not been read yet
  h.tag = b"nc";
  Cairn c = seal h;
  h.len = 4;          // error: h was sealed at line 6
}
```

> In TempleOS every task could write to all of memory at all times. Nether C
> keeps the total sharing and removes the writing. Below, nothing changes;
> that is what makes it the nether.

## 5.5 Depth is not a type constructor

`τ@d` is a type paired with a depth, not a type of its own. In particular
`Bytes@3` and `Bytes@0` are the same type at different depths: they unify,
and unification takes the maximum.

`Shadeᵈ⟨T⟩` *is* a distinct type, because a shade's whole purpose is to be
something you cannot use as a `T`.

## 5.6 Type inference

Types are inferred within a function body and MUST be written on:

- struct fields,
- function parameters and return types,
- unit-level `let` declarations.

Depth is inferred everywhere and MAY be written anywhere a type is written. A
written depth is a checked assertion: if inference produces a different depth,
that is an error naming both depths and the expression responsible.

> Depth annotations must remain optional in ordinary code. The roadmap item
> *Depth inference* is proven by deleting every `@n` from every sample program
> in this specification and requiring them all still to compile.
