---
id: 273
title: Grafting an answer keeps the deposits it replaced
type: bug
status: unmarked
created: 2026-09-19
updated: 2026-09-19
priority: p1
effort: m
area: crates/nether-cli
stratum: '2'
proof: Grafting an answer a trace was already given produces a trace whose deposits are the substituted answer, and `lamp` of it differs from `lamp` of the trace it replaced
---

## What happens

## What should happen

## Reproduction

1.

## Cairn of the offending trace

## Fix boundary and regression proof

Name the smallest affected path, the failing test before the fix, and the
verification after it. Preserve unrelated behavior.

`graft` names two things it can replace. Replacing a hole works and is what
every test does. Replacing an answer the trace was already given is accepted,
counted and reported, and does nothing.

A sealed trace's residue no longer contains the work that consumed the answer —
that work happened during `exhume`, and its deposits are already in the trace.
`take` re-buries the residue, which now produces nothing, and then carries the
old deposits forward:

```rust
deposits: crate::closing::deposits(&deposits, &residue.deposits),
```

So the grafted trace names the substituted witness and the *unsubstituted*
deposits. It is internally inconsistent, and `graft` reports `1 recomputed`
while nothing was recomputed.

To see it, with `post.nc` reading `env("WHO")` and depositing it:

```console
$ nether bury post.nc
buried   post.nc → b720a9c3   depth 2   holes 1   6 nodes
$ nether exhume b720a9c3 --grant env --declare WHO=world --clock 0 --target x86_64-unknown-linux-gnu
sealed   b720a9c3 + 10960a1b → 18ee3b82   depth 2   holes 0
$ nether exhume b720a9c3 --grant env --declare WHO=moon --clock 0 --target x86_64-unknown-linux-gnu
sealed   b720a9c3 + c26253b8 → 2d77ad6d   depth 2   holes 0
$ nether graft 18ee3b82 --replace 10960a1b --with c26253b8
grafted  18ee3b82 → 528854fe   1 reused   1 recomputed
$ nether lamp 528854fe
Hello, world
```

`Hello, moon` is what `2d77ad6d` says, and `528854fe` was supposed to be it.

Found while writing the manual (0104). The manual does not document `graft`
until this is settled, because a worked example on this path would teach the
bug.

Either fix it — re-bury from the trace that still had the hole, which is the
only thing that can produce the right deposits — or refuse it, and say in §8.7
that a graft replaces a hole. Refusing is defensible and is one `if`; the
rejected alternative goes in `spec/90-rationale.md` either way.
