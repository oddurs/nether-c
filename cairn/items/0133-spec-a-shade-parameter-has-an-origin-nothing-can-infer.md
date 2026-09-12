---
id: 133
title: 'Spec: a shade parameter has an origin nothing can infer'
type: spec
status: buried
milestone: surface
depends_on:
- 19
created: 2026-09-12
updated: 2026-09-12
priority: p1
effort: m
area: spec/04-grammar.md
proof: Every Shade a program can write has an origin the specification says where to get
---

## The hole

`spec/04-grammar.md` §4.3:

> The origin depth of a shade is part of its type but is not written in source
> syntax; it is always inferred, because writing it would let a programmer
> claim an origin the value does not have.

Inferred from what it is bound to, where there is something:

```c
Shade<Bytes> reply = descend net { shade must(get(url)) };   // origin 5
```

A parameter has nothing to be bound to:

```c
U0 keep(Shade<Bytes> s) { … }
```

so its origin is inferred from nothing. §1.6's own worked example passes a
shade to somewhere, and there is no way to declare the somewhere.

Found by lowering in 0056, which has to put a number there and has none.

## What it looks like settled

The honest answers are a polymorphic origin or a written one, and they pull
opposite ways.

- **An origin variable.** `keep` works for a shade from any stratum, and
  `look` inside it is legal only where the caller's ambient depth allows,
  which means the variable has to reach the call site. That is a second kind
  of polymorphism in a language that has none.
- **A written origin, for parameters only.** `Shade<Bytes>@5 s` — but §4.3's
  reason for not writing one is that a programmer could claim an origin the
  value does not have, and on a parameter the claim is checked at every call
  site rather than believed. That reason does not apply here, which is why
  this is probably the answer.

What lowering does meanwhile is the conservative thing: a shade parameter's
origin is stratum 8, so looking at one is legal nowhere. Useless and sound,
which beats useful and wrong.

## Acceptance criteria

- [x] A shade parameter has an origin, and §4.3 says where it comes from
- [x] §1.6's example passes a shade to something that can be declared
- [x] `spec/90-rationale.md` records the alternative
- [x] nether-syntax follows

## 2026-09-12

Settled with the written origin, and with no new syntax — which is the part worth recording.

A shade's origin IS the depth of the value it holds, and the depth annotation on a type already means 'this came from there'. A shade's argument is a type. So Shade<Bytes@5> is a shade of bytes from stratum 5, and Shade<Bytes@5>@0 is that shade held at depth 0, which is the ordinary case and the whole reason a shade is worth having. It already parsed; nothing in 4.3's EBNF changed.

The rule splits by where the shade appears rather than by whether it is written. Where a shade is CONSTRUCTED, the origin is inferred and may not be written — 4.3's original reason stands there, since a programmer could claim an origin the value does not have. Where a shade is RECEIVED — a parameter, a field, a return type — it must be written, because there is nothing to infer it from and the claim is not believed: it is checked at every call site.

The origin variable is rejected implicitly by this: it would be a second kind of polymorphism in a language with none, and it would have to reach the call site to make look checkable.

Two shades of the same type with different origins are different types, because look on them is legal in different places. 5.5 says so now.
