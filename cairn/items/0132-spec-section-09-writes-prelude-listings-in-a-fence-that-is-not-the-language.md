---
id: 132
title: 'Spec: section 09 writes prelude listings in a fence that is not the language'
type: spec
status: buried
milestone: after
assignee: Oddur Sigurdsson
depends_on:
- 23
created: 2026-09-12
updated: 2026-09-12
priority: p2
effort: s
area: spec/09-prelude.md
proof: Every c fence in the specification is something section 04 parses
---

## The mismatch

`spec/09-prelude.md` lists what the prelude provides like this:

```
Answer<Bytes> read(Str path)           @3;   // absent, denied
```

A signature, then a semicolon. `spec/04-grammar.md` §4.2 has

```ebnf
func_decl := type identifier "(" [ params ] ")" [ latent ] block ;
```

with a block and no alternative. So seven of the `c` fences in §09 are not
Nether C, and the fence says they are.

Nothing is wrong with the notation — it is a header file, and a header file is
a useful thing to read. What is wrong is that it is labelled as the language,
which makes a reader believe a declaration without a body is something they
could write.

Found by the parser's proof in 0055, which had to sort every sample in the
specification into three kinds and found that one of the kinds is a notation
nobody documented.

## What this must decide

Either the fence stops saying `c`, or §04 gains a production for a declaration
with no body and §09's listings become ordinary Nether C that happens to be
provided rather than written.

The first is one character seven times. The second is a language feature, and
the language has no use for one: a program cannot supply a body for `read`.

## Acceptance criteria

- [x] Every fenced `c` block in the specification is something §04 parses
- [x] §09 says what its listings are, if they are not that

## 2026-09-12

Settled the cheap way, which is also the right way: the fence stops claiming to be the language. Seven blocks in section 09 are now tagged signatures, section 09 says in a sentence that a signature and a semicolon is a notation and not a program, and site/bake paints them the same and labels them differently — because they read like the language and are not it.

## 2026-09-12

The alternative was a production for a declaration with no body, which is a language feature the language has no use for: nothing a program can write would supply a body for read. A header file is a useful thing to read and not a thing to be able to write.

## 2026-09-12

The parser's proof got shorter for it. Every sample in the specification was sorted into three kinds and one of them was this notation; now there are two, and the classification says something about the grammar rather than about the markdown.
