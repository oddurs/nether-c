---
id: 157
title: Freezing a local depends on which arm you write first
type: bug
status: buried
milestone: surface
assignee: Oddur Sigurdsson
created: 2026-09-12
updated: 2026-09-12
priority: p1
effort: s
area: crates/nether-core
stratum: '0'
proof: Two programs differing only in the order of two if arms are accepted or rejected alike
---

## The asymmetry

[§5.4](../spec/05-types.md): "If any path through a block names a local, it is
named for everything **after** that block."

The checker applies it in source order *within* the block instead, so two
programs that differ only in which arm holds the `seal` disagree:

```c
if (c) { Cairn k = seal h; } else { h = b"y"; }   // error: this was named
if (c) { h = b"y"; } else { Cairn k = seal h; }   // accepted
```

Neither assignment is after the block. Both programs are safe — the arms are
exclusive, so no value is ever both sealed and then changed. The checker
refuses one of them because of where it appears on the page.

## And the one underneath it

Fixing the arms turned up a worse case in the other direction. A loop body runs
again, so a naming anywhere in it precedes an assignment anywhere in it —
including one written above it. This was accepted:

```c
while (c) { h = b"y"; Cairn k = seal h; }
```

On the second turn the assignment follows the seal, so a sealed value changes.
That is the thing §5.4 exists to stop, and source order cannot see it.

The arm leak was also masking the true positive: in

```c
if (c) { Cairn k = seal h; } else { h = b"y"; }
h = b"z";
```

the error pointed at the else-arm, which is legal, instead of at `h = b"z"`,
which is the line §5.4 forbids.

## What this must decide

Either the arms are parallel — an implementation cannot know which one runs, so
a local named on any arm is named on all of them and both programs are refused
— or naming is a source-order fact and §5.4 says that instead of saying "after
that block".

The first is what §5.4's own reasoning argues for, and it is the conservative
one: it refuses a safe program rather than accepting an unsafe one. The second
is cheaper and has to admit the rule is about text.

## Acceptance criteria

- [x] The two programs above are treated alike
- [x] §5.4 describes what the checker does
- [x] A loop body cannot seal a value on one turn and change it on the next

## 2026-09-12

The arms are exclusive, so neither precedes the other: each is walked from where the block started and what they name is joined afterwards, because it is named for everything after the block. That accepts both programs rather than refusing both -- the conservative reading was wrong, since two exclusive paths cannot both run.

## 2026-09-12

A loop is the opposite case and was a false negative: while (c) { h = b"y"; seal h; } was accepted, and on the second turn the assignment follows the seal. The body is now walked once for what it names with the complaints thrown away, then walked for real.
