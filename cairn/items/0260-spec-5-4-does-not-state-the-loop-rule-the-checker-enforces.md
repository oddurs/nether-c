---
id: 260
title: 'Spec: §5.4 does not state the loop rule the checker enforces'
type: spec
status: buried
milestone: codex
assignee: Oddur Sigurdsson
depends_on:
- 157
created: 2026-09-16
updated: 2026-09-17
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

- [x] §5.4 states the back-edge rule
- [x] The loop above is a rejection in `tests/programs/`
- [x] `for (I64 i = 0; i < n; i += 1)` is still accepted, and §5.4 still says why

## 2026-09-17

Written. §5.4 now says a loop body comes after itself, so a local named anywhere in one is named for the whole of it including the lines above the naming -- the second turn's assignment is after the first turn's naming, which is the arms' reasoning and no new concept. The paragraph ends by keeping the counter explicitly: i += 1 assigns a local that arithmetic never named, so the new rule has nothing to freeze, which is the same distinction three paragraphs up and the reason the rule is about naming rather than reading. tests/programs/turn.nc is the sample, refused by check with 'this was named, and a name cannot change what it names' at the assignment; crates/nether-syntax/tests/frozen.rs includes it so the sentence and the checker cannot drift. The sample is registered as 05-types.md § 5.4 Mutation #1, Shape::Illegal, and recorded in the transcript manifest. 0157's third criterion is now discharged for the back edge as well as the arms. crates/nether-core was already right and did not change: the checker walks a loop body once for what it names, throws the complaints away, then walks it for real. scripts/task check passes in full; the ceiling did not move.
