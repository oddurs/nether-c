---
id: 123
title: 'Spec: sizeof has no answer'
type: spec
status: unmarked
milestone: surface
depends_on:
- 19
created: 2026-09-12
updated: 2026-09-12
priority: p2
effort: s
area: spec/05-types.md
proof: sizeof of every type in section 5.1 has a value the specification fixes
---

## The hole

`spec/04-grammar.md` §4.5 has

```ebnf
primary := … | "sizeof" "(" type ")" | …
```

and nothing anywhere says what it evaluates to. `spec/05-types.md` §5.1 gives
each type a canonical *encoding*, which is a different question: `Bytes` is a
length prefix and then the bytes, so its encoding has no fixed size at all.

There is no pointer type, no array decay and no allocation, so the C reason
for `sizeof` — how much memory to ask for — does not exist here either.

Found while building burial, which has to do something with every form in the
IR and had nothing to do with this one. It residualises, which is the only
honest thing available.

## What this must decide

One of:

- `sizeof` is the length of the canonical encoding, which makes it a fact
  about the ledger and undefined for anything variable-length unless it means
  *of this value* rather than *of this type*;
- `sizeof` is a fixed size per type, which means inventing a memory layout the
  language otherwise does not have;
- there is no `sizeof`, and §4.5 loses a production.

The third is the most likely to be right, and `len` already answers the
question people would reach for it to ask.

## Acceptance criteria

- [ ] `sizeof` has a stated value, or it is gone from §4.5
- [ ] If it stays, `spec/90-rationale.md` says why a language with no pointers has one
- [ ] nether-core follows
