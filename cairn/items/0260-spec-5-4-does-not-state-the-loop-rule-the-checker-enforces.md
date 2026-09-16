---
id: 260
title: 'Spec: §5.4 does not state the loop rule the checker enforces'
type: spec
status: unmarked
milestone: codex
depends_on:
- 157
created: 2026-09-16
updated: 2026-09-16
priority: p1
effort: s
stratum: '0'
area: spec/05-types.md
proof: §5.4 rejects the loop 0157 rejects
---

## What is missing

`spec/05-types.md` §5.4:

> If any path through a block names a local, it is named for everything after
> that block.

A loop body has a back edge, so "after" is not enough:

```c
while (c) { h = b"y"; Cairn k = seal h; }
```

The assignment is before the seal on the page and after it on the second turn.
That is the thing §5.4 exists to stop.

0157 found it and fixed it in `crates/nether-core`: the body is walked once for
what it names, with the complaints thrown away, then walked for real.

§5.4 was not changed. Its only sentence about loops is about a `for` counter,
which is the case that stays legal. So 0157's third acceptance criterion — "§5.4
describes what the checker does" — is discharged for the arms and not for the
back edge, and the specification currently accepts a program the implementation
refuses.

## What to write

One sentence: a local named anywhere in a loop body is named for the whole
body, because the body runs again. Same reasoning as the arms, no new concept.

## Acceptance criteria

- [ ] §5.4 states the back-edge rule
- [ ] The loop above is a rejection in `tests/programs/`
- [ ] `for (I64 i = 0; i < n; i += 1)` is still accepted, and §5.4 still says why
