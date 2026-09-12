---
id: 157
title: Freezing a local depends on which arm you write first
type: bug
status: unmarked
milestone: surface
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

## What this must decide

Either the arms are parallel — an implementation cannot know which one runs, so
a local named on any arm is named on all of them and both programs are refused
— or naming is a source-order fact and §5.4 says that instead of saying "after
that block".

The first is what §5.4's own reasoning argues for, and it is the conservative
one: it refuses a safe program rather than accepting an unsafe one. The second
is cheaper and has to admit the rule is about text.

## Acceptance criteria

- [ ] The two programs above are treated alike
- [ ] §5.4 describes what the checker does
