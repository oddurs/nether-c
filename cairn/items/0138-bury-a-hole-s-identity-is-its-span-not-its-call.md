---
id: 138
title: 'bury: a hole''s identity is its span, not its call'
type: bug
status: unmarked
milestone: rites
created: 2026-09-12
updated: 2026-09-12
priority: p0
effort: s
area: crates/nether-bury
stratum: '0'
proof: Two identical calls at two source positions are one hole
---

## The divergence

[§6.3](../spec/06-evaluation.md) ends:

> Two holes with identical `call` fields in the same trace MUST be the same
> hole. This is what makes exhumation cheap: reading the same file twice is one
> question, asked once.

`dig` interns on the hole node's cairn, and `Node::Hole` encodes its span
(`spec/07-ledger.md` §7.3.1). So identity is the call *and the position*, and
two textually distinct `read("main.nc")` calls are two holes:

```
holes: 2
  Hole { call: { function: "read", args: [42e9832c] }, span: { start: 10, end: 20 } }
  Hole { call: { function: "read", args: [42e9832c] }, span: { start: 90, end: 100 } }
```

Exhumation then asks the world the same question twice, which is the one thing
§6.3 exists to prevent.

## Why the proofs did not catch it

Both of them exercise one call site.

- `holes.rs` `the_same_question_twice_is_one_hole` builds three copies through
  the `reading()` helper, which hardcodes `Span::default()`. Three identical
  spans, so three identical cairns.
- `stage_one.rs` `reading_the_same_file_twice_is_still_one_hole` reaches one
  `read` twice through a global. That proves a unit-level binding is evaluated
  once, which is §6.2, not §6.3.

## What to do

Intern on the `call`, not on the cairn, and keep the span of the first place
that asked — which is what §6.3's field table already describes, since it gives
a hole one `span` and calls it "the source location that asked".

## Acceptance criteria

- [ ] Two identical calls at two source positions are one hole
- [ ] A test that would have failed before this
- [ ] §6.3 says which span a shared hole keeps
