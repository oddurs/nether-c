---
id: 263
title: 'Spec: a program can inspect an Answer and cannot build one'
type: spec
status: unmarked
milestone: codex
created: 2026-09-16
updated: 2026-09-16
priority: p2
effort: m
stratum: '0'
area: spec/09-prelude.md
proof: A program can return an Answer of its own, or §5.1.1 says why it may not
---

## Two holes with one shape

**No program can construct a refusal.** `spec/05-types.md` §5.1.1 gives the
type:

```
Answer⟨T⟩  ::=  Given T  |  Refused Refusal
```

[§9.2](../spec/09-prelude.md) binds the six codes and three inspectors —
`given`, `refusal`, `must` — and no constructor. So every `Answer` that exists
came out of the prelude, and a program that parses something cannot hand its
caller the shape the prelude hands it. It has `must`, which collapses, or a
`Bool` and a separate value, which is the thing `Answer` replaced.

§9.2 already shows this is not a rule anybody chose. `utf8` is at depth 0 and
returns `Answer<Str>` refused `malformed` — a pure prelude function refuses,
while §9.9 frames the type as what the *world* said. A pure program function
cannot do what a pure prelude function does.

**No program can write a function over two types.** §9.2's signatures are
generic in `T`. `func_decl` ([§4.2](../spec/04-grammar.md)) has no type
parameters, §3.8 removes the preprocessor and §5.2 removes pointers. No macro,
no `void *`, no generic: a program that wants `Answer<Header>` has to be the
prelude.

## What to decide

For answers, the cheap fix is two prelude constructors at depth 0 — a `given`
of a `T` and a `refused` of a `Refusal`. It adds nothing to the calculus: an
`Answer` is already a value with an encoding in
[§7.1](../spec/07-ledger.md#71-canonical-encoding), and these are ordinary
depth-0 functions over it. The naming needs care, since `given` is taken by the
predicate.

For generics the honest answer is probably *not in this draft* — and then §90.3
has to say so beside the other exclusions, because a C programmer reaches for a
container on day two and this language has no way to write one.

## Acceptance criteria

- [ ] §9.2 binds constructors for `Answer`, or §5.1.1 says only the prelude builds one and why
- [ ] §9.9 is consistent with `utf8` refusing at depth 0
- [ ] §90.3 records that a program cannot abstract over a type
- [ ] If constructors are added, a program returning its own `Answer<T>` is in `tests/programs/`
