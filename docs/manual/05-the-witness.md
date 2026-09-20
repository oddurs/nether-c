---
section: "05"
title: The witness
status: draft
---

# The witness

> One idea: **what the world said is kept beside what it made.**

Pick up where [chapter 4](04-the-grant.md) left off:

```console
$ nether exhume 4fc5e660 --grant disk
  ①  read("who.txt")  →  6 bytes  4395a792
sealed   4fc5e660 + 4395a792 → f03489d0   depth 3   holes 0
```

That `4395a792` in the middle of the seal line was not decoration. It is the
name of the answer, and the answer is in the ledger:

```console
$ nether lamp 4395a792
given world
```

*Given*, as opposed to refused. Six bytes, named, stored, and attached to the
trace they went into. The disk said this, at this moment, to this rite, and
nothing else in the program could have said it instead.

The node that records that is a **witness**, and a rite cannot skip making one.
The provider that answers a question in this implementation hands back a value
that only the recorder can construct, and the recorder writes to the ledger
before it returns. There is no code path where the world answers and nothing
is written down, because there is no way to produce the answer without writing
it. That is what [§1.4](../../spec/01-strata.md) means, expressed as a type
rather than as a rule to remember.

## Reading it back

```console
$ nether lamp f03489d0 --provenance
f03489d0  trace    depth 3   residue d6374b77   0 hole(s)   1 witness(es)   1 deposit(s)
```

One witness, one deposit, no holes left. `--provenance` walks backwards — from
a value to the nodes that made it, to their inputs, down to the literals and
the answers at the bottom. It is the question "where did this come from",
asked of the ledger rather than of a person.

```console
$ nether strata f03489d0
depth 3   disk

  3  read("who.txt")           cc948e97:7:36
  0  everything else

  replayable: yes
```

The same account you saw before granting anything, with `pending` gone from the
one line that has now happened. Before and after are the same shape, which is
the point: you can compare them.

## Why this is the interesting one

Most of this language can be explained as a restriction. This chapter is the
thing the restrictions are for.

A trace is an argument about what happened that you do not have to take on
faith. It names every value it was given, every place it was given one, and
how deep each of them reached. A build that produced a wrong binary can be
asked which bytes it was handed. A program that touched the network can be
asked what came back. Not from a log the program chose to write — from the
structure of what it left.

Logs are a program's account of itself. A trace is not written by the program.

## What a refusal looks like

The world is allowed to say no, and no is an answer like any other. It is
recorded the same way, with the same kind of node, and a program can branch on
it — [§5.1.1](../../spec/05-types.md) fixes the closed set of six refusals, and
[§9.9](../../spec/09-prelude.md) is what `must` was skipping past back in
[chapter 3](03-the-hole.md).

The distinction is worth holding on to. A refusal is the world answering. A
hole is the world not having been asked yet. They are different nodes, and
conflating them is how a system ends up unable to tell "there is no such file"
from "nobody looked".

[Next](06-two-worlds.md): the same burial, answered twice.
