---
id: 133
title: 'Spec: a shade parameter has an origin nothing can infer'
type: spec
status: unmarked
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

- [ ] A shade parameter has an origin, and §4.3 says where it comes from
- [ ] §1.6's example passes a shade to something that can be declared
- [ ] `spec/90-rationale.md` records the alternative
- [ ] nether-syntax follows
