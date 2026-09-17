---
id: 255
title: 'Spec: two identical sends are one hole'
type: spec
status: descending
milestone: codex
assignee: Oddur Sigurdsson
claimed: 2026-09-16
created: 2026-09-16
updated: 2026-09-16
priority: p0
effort: m
stratum: '0'
area: spec/06-evaluation.md
proof: Two identical post calls leave two holes; two identical read calls leave one
---

## The rule

`spec/06-evaluation.md` §6.3:

> Two holes with identical `call` fields in the same trace MUST be the same
> hole. This is what makes exhumation cheap: reading the same file twice is one
> question, asked once.

The justification is a read. The rule ranges over every prelude call.

## What it does to the other half of the lattice

```c
demand post("http://h/pay", b"{}");
demand post("http://h/pay", b"{}");
```

Two sends, one hole, one witness, one `post`. §1.1 says stratum 6 costs *the
world remembers what was said*, and §1.8 spends a section arguing a write is
deeper than a read because it cannot be taken back. §6.3 is the rule that
ignores that split.

Entropy is worse, because the merge is silent and the value is wrong:

```c
Bytes a = descend entropy { draw(8) };
Bytes b = descend entropy { draw(8) };
```

`a` and `b` are the same eight bytes. §9.7 calls `draw` the only source of
nondeterminism in the language; under §6.3 it is a pure function of its
argument within one trace.

## Where the line is

A read is idempotent and its answer is a fact about the world, so asking twice
is waste. A send and a draw are *acts*, and two acts are two.

`write` and `remove` are the arguable middle. Writing the same bytes to the
same path twice is idempotent in effect; `remove` is not — the second answers
`absent` where the first answered given. Merging either makes the trace
describe fewer acts than the program performed.

The narrowest rule that keeps §6.3's payoff: **holes at a read stratum — 1, 2,
3, 5 — with identical calls are one hole. Holes at 4, 6, 7 and 8 are never
merged.**

## What the implementation does today

0138 moved the interning from the hole's cairn to the `call`, which is §6.3 as
written. So `nether-bury` merges sends and draws now, and the proof that closed
0138 is a `read`.

## Acceptance criteria

- [ ] §6.3 says which strata merge and which do not, and rests the split on §1.8
- [ ] Two identical `read` calls leave one hole
- [ ] Two identical `post` calls leave two holes, with two spans
- [ ] Two `draw(8)` calls can produce different bytes
- [ ] 0138's proof is restated over a stratum that still merges
- [ ] §90.2 records the version that merged everything

## 2026-09-16

Specification half: §6.3 now merges only at a read stratum -- 1, 2, 3 and 5 -- and never at 4, 6, 7 or 8, resting the split on §1.8's own argument that those strata are ordered by what they disturb rather than what they learn. It also settles the question the split creates: the answers to one call are served in the order §6.2 fixes, the nth hole taking the nth answer, and an implementation must not address an answer by the span that asked -- a span is a fact about one text and §6.5 requires a residue to be printed and lowered again. §90.2 records the version that merged everything and why it survived review: the rule was written with a read in front of it and proved with a read. The burier still merges everything; that is the second half of this item.
