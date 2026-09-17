---
id: 255
title: 'Spec: two identical sends are one hole'
type: spec
status: buried
milestone: codex
assignee: Oddur Sigurdsson
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

- [x] §6.3 says which strata merge and which do not, and rests the split on §1.8
- [x] Two identical `read` calls leave one hole
- [x] Two identical `post` calls leave two holes, with two spans
- [x] Two `draw(8)` calls leave two holes, which is what lets them answer differently; the answering is `exhume`'s and is not built
- [x] 0138's proof is over `read`, stratum 3, which still merges
- [x] §90.2 records the version that merged everything

## 2026-09-16

Merging is by stratum: 1, 2, 3 and 5 read and merge; 4, 6, 7 and 8 do not. The split is §1.8's, which is the section that argues a write is deeper than a read because it cannot be taken back — this was the rule that ignored it one section later. Depth::answers_alike is the predicate and the list is checked against the lattice rather than repeated in prose.

## 2026-09-16

Two acts at one span are still one hole, because a node is named by its content and a Hole holds only call, stratum and span. A post inside a loop that unrolls is the case. That needs a field on Hole and a domain bump, so it is 0264 rather than a line here.
