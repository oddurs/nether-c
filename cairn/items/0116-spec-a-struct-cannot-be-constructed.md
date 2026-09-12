---
id: 116
title: 'Spec: a struct cannot be constructed'
type: spec
status: unmarked
milestone: surface
depends_on:
- 18
created: 2026-09-12
updated: 2026-09-12
priority: p0
effort: s
area: spec/04-grammar.md
proof: Every sample program in the specification, section 5.4 included, parses under the grammar in section 04
---

## The hole

`spec/05-types.md` §5.4 opens with

```c
Header h;
h.len = 3;
```

and `spec/04-grammar.md` §4.4 has no production for it. `let_decl` is
`type identifier "=" expr ";"` — an initialiser is required — and `primary`
has no struct literal and no array literal.

So there is no way to write a struct value, and there is no way to write an
array value either. Two consequences:

- §5.4's own sample does not parse under §04.
- The `assign` production is unreachable. §5.4 permits a write only to a local
  aggregate that has not been read yet, and the only aggregates a program can
  obtain are parameters, which have been.

Found while proving `nether-core: the IR` against §04: the IR has a form for
every production, and two of them have nothing that can reach them.

## What this must decide

One of:

- a struct literal in `primary`, which makes aggregates ordinary values and
  leaves §5.4's build-then-seal discipline with nothing to describe;
- an uninitialised declaration in `stmt`, which keeps §5.4 as written and owes
  a definition of what it means to read a half-built aggregate;
- neither, and §5.4's example goes.

Whichever it is, `spec/90-rationale.md` gets the two that were rejected.

## Acceptance criteria

- [ ] A production that constructs an aggregate, or an argued removal of §5.4's example
- [ ] Array values have a form too, or a stated reason they do not need one
- [ ] The rejected alternatives recorded in §90.2
