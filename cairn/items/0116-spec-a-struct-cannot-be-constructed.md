---
id: 116
title: 'Spec: a struct cannot be constructed'
type: spec
status: buried
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

- [x] A production that constructs an aggregate, or an argued removal of §5.4's example
- [x] Array values have a form too, or a stated reason they do not need one
- [x] The rejected alternatives recorded in §90.2

## 2026-09-12

A second unreachable construct on the same seam: section 4.4's for production has a step expression and section 4.5 has the compound assignments, and section 5.4 says a binding may not be reassigned. So for (I64 i = 0; i < n; i += 1) cannot advance its own counter. Whatever settles the construction form has to settle this too, or the for loop goes.

## 2026-09-12

Settled by drawing the line at the ledger rather than at the function body.

A local may be assigned, and its fields and elements may be assigned, until it is NAMED — sealed, shaded, deposited, returned or passed as an argument, which is exactly the moment its cairn exists. 5.4's own sentence was already right: 'once a value has been read, its cairn exists, and nothing can change what a cairn names'. The grammar just never allowed the construct.

let_decl's initialiser is now optional at block level and required at unit level, since there is no statement above a global to assign one. Reading an unassigned field collapses: there is no value there and there never was one, and a zero would be the implementation deciding what the program meant.

That makes all three unreachable constructs reachable for ONE reason rather than three: assign, the for step, and aggregate construction.

Both alternatives are in 90.2. A struct literal fixes construction and leaves for unable to step. Forbidding assignment outright leaves a language with while and recursion and nothing else, and the inversion does not require it — what every TempleOS task could write to was shared, addressable, permanent memory, and a counter in a block is none of those.

The cost: 'there is no mutation' stops being true flat and becomes true of the ledger. A worse sentence and a better rule.

The parser does not implement it yet — filed, p0, in this milestone.
