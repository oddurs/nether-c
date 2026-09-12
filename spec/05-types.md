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
| `Answer⟨T⟩` | what the world said: a `T`, or a refusal | `0x07`: one discriminant byte, then the `T` or the `Refusal` |
| `Refusal` | one of six codes, and nothing else | `0x08`: one byte, in the order §5.1.1 lists them |

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
implementation MUST NOT add to it. They encode as `0x00` through `0x05`, in
the order written above, and that order is part of the format: see
[§7.1](07-ledger.md#71-canonical-encoding). The six names are bound in the prelude scope
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

`Shade` is comparable, and the rule above is the whole of it. A shade's origin
stratum is part of its type and of its encoding
([§7.1](07-ledger.md#71-canonical-encoding)), so two shades are equal when they
came out of the same stratum holding the same value. Comparing them does not
count as looking at them, because it reveals only what `seal` on each would
reveal anyway.

## 5.4 Mutation

Nothing in the ledger ever changes. A local, before it is in the ledger, is not
in the ledger.

That is the whole of the rule, and the rest of this section is what it means.

A **local** is a binding inside a block. It may be assigned, and its fields and
elements may be assigned, until it is **named** — and after that it may not.
A local is named the moment its value is used as a value: sealed, shaded,
deposited, returned, or passed as an argument. From then on its cairn exists,
and nothing can change what a cairn names.

A **global** — a unit-level `let` — may not be assigned at all. There is no
statement above it to do the assigning, and its initialiser is required
([§4.2](04-grammar.md#42-declarations)).

An implementation SHOULD compile the assignments to in-place writes. No program
can observe the difference, which is what makes it sound.

Reading a field or an element that has not been assigned **collapses**
([§9.9](09-prelude.md#99-failure-and-the-difference-between-two-of-them)). It
is a mistake in the program and not a fact about the world: there is no value
there and there never was one.

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

This is also what makes a `for` loop work. Its step assigns the counter, which
is a local and has not been named, so the loop advances without anything in the
ledger changing.

> In TempleOS every task could write to all of memory at all times. Nether C
> keeps the total sharing and removes the writing.
>
> The line is drawn at the ledger rather than at the function body, and it is
> worth being exact about why. What the inversion is about is *shared,
> addressable, permanent* memory — the thing every task could reach. A counter
> in a loop is none of those: nothing else can see it, it has no name, and it
> stops existing when the block does. Forbidding it would buy nothing and cost
> the language its loops.
>
> Below, nothing changes. A local is not below yet.

## 5.5 Depth is not a type constructor

`τ@d` is a type paired with a depth, not a type of its own. In particular
`Bytes@3` and `Bytes@0` are the same type at different depths: they unify,
and unification takes the maximum.

`Shadeᵈ⟨T⟩` *is* a distinct type, because a shade's whole purpose is to be
something you cannot use as a `T`. Its `d` is written as the depth on `T`
([§4.3](04-grammar.md#43-types)) — `Shade<Bytes@5>` — and two shades of the
same type with different origins are different types, because `look` on them
is legal in different places.

## 5.6 Types are written; depths are inferred

**Every binding carries its type.** Struct fields, function parameters and
return types, unit-level `let` declarations, and block-level ones. There is one
`let_decl` production ([§4.2](04-grammar.md#42-declarations)) and it begins
with a type, everywhere it appears.

**Depth is inferred everywhere**, and MAY be written anywhere a type is
written. A written depth is a checked assertion: if inference produces a
different one, that is an error naming both depths and the expression
responsible.

That is the whole rule, and the two halves are not arbitrary. A type is a fact
about what a value *is*, which the person writing the program knows and the
reader wants told. A depth is a fact about where it *came from*, which is
derived from everything it touched and is exactly the thing a person gets
wrong. Writing what you know and deriving what you do not is the trade the
language is making.

> An earlier draft said types were inferred inside a function body, and there
> was no way to write such a binding: the grammar has one `let_decl` and it
> begins with a type. Adding a second form to get inference would have put a
> keyword in the language to save four characters, and made `n = 0;` mean a
> binding in one reading and an assignment in another. See
> [§90.2](90-rationale.md#902-rejected-alternatives).

> Depth annotations must remain optional in ordinary code. The roadmap item
> *Depth inference* is proven by deleting every `@n` from every sample program
> in this specification and requiring them all still to compile.
