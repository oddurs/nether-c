---
id: 134
title: 'Spec: types are inferred in a body and the grammar requires one'
type: spec
status: unmarked
milestone: surface
depends_on:
- 18
created: 2026-09-12
updated: 2026-09-12
priority: p1
effort: s
area: spec/04-grammar.md
proof: Every binding section 5.6 says may omit its type has a production that omits it
---

## The contradiction

`spec/05-types.md` §5.6 opens:

> Types are inferred within a function body and MUST be written on:
>
> - struct fields,
> - function parameters and return types,
> - unit-level `let` declarations.

A block-level binding is not on the list, so its type is inferred. But
`spec/04-grammar.md` §4.2 has one production for both:

```ebnf
let_decl := type identifier "=" expr ";" ;
```

and §4.2 says so in as many words: "A `let_decl` at unit level is a
declaration; the same production inside a block is a statement." So the
grammar requires the type that §5.6 says is inferred, and there is no way to
write the binding §5.6 describes.

Found while proving depth inference in 0057. Depth annotations are optional
everywhere, which is what that item asks for. Type annotations are mandatory
everywhere, which is not what §5.6 says.

## What this must decide

Either §5.6's list is wrong and a type is always written, or §4.4 gains a
statement form for a binding with no type — `n = 0;` or `let n = 0;` or
something else, and *something else* matters here because `n = 0;` is already
an assignment expression statement and the two would be indistinguishable.

That collision is the interesting part, and it is the same one as 0116: a
language with no reassignment does not need `n = 0;` to mean assignment, and
if assignment goes then the spelling is free.

## Acceptance criteria

- [ ] §5.6 and §4.4 agree about which bindings carry a type
- [ ] If a form is added, it is not ambiguous with an assignment
- [ ] nether-syntax follows
