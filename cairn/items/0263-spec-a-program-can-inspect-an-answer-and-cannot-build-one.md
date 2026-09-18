---
id: 263
title: 'Spec: a program can inspect an Answer and cannot build one'
type: spec
status: buried
milestone: codex
assignee: Oddur Sigurdsson
created: 2026-09-16
updated: 2026-09-17
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

- [x] §9.2 binds constructors for `Answer`, or §5.1.1 says only the prelude builds one and why
- [x] §9.9 is consistent with `utf8` refusing at depth 0
- [x] §90.3 records that a program cannot abstract over a type
- [ ] If constructors are added, a program returning its own `Answer<T>` is in `tests/programs/`

## 2026-09-17

Two holes, and they came apart. The generics one is settled: §90.3 now says a program cannot abstract over a type, beside the other exclusions -- no type parameters on func_decl, no preprocessor, no pointers, so the prelude's own signatures are generic in T and nothing a program writes can be, and a container is one type per element type by hand in this draft. The answers one is settled as far as it can be settled here, and not by adding the constructors. §9.9's 'a refusal is an answer' listed 'the bytes are not UTF-8' among the things the world said and grounded the whole type in §1.4; it no longer does. Bytes that are not UTF-8 are a fact about bytes the program already had, nothing was asked of anybody and no witness is owed, so what Answer<T> says is that this can be refused and which no it was, and where the no came from is what the stratum says. §5.1.1 and §9.2 say plainly that there is no constructor and that utf8 shows this is not a rule anybody chose. The constructors themselves are 0266, in the block-quote form §0.6 prescribes, because they cannot land here: crates/nether-core's the_prelude_is_the_one_in_section_09 reads the signature fences out of §09 and compares them with Prim::ALL, so the specification cannot gain a prelude function ahead of the implementation -- and refused(absent) does not say what T is, while lower.rs's three polymorphic prims all take their T from an argument the IR has no variables for. Criterion 4 goes with 0266; there is no program to put in tests/programs until the shape is decided. scripts/task check passes in full.
