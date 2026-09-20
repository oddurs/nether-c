---
section: "03"
title: The hole
status: draft
---

# The hole

> One idea: **what the world has not said is written down as a question.**

Every program so far could be worked out completely, because nothing in it
needed anything from outside. Most programs are not like that. Write
`errand.nc`:

```c
// errand.nc — the manual's worked example. docs/manual/03-the-hole.md.
//
// One read of one file, and a greeting built from what comes back. It is the
// smallest program that has a hole in it: `read` is stratum 3, burial has no
// disk, so the call cannot happen and the trace says so instead.

Bytes@3 name = must(descend disk { read("who.txt") });

U0 greet()
{
  concat(b"Hello, ", name);
}

demand greet();
```

Three new things, and they are all on the first line.

`@3` is a **stratum**. It is part of the type, and it says this value comes
from a depth of 3 — the disk. A type without one is at stratum 0 and is pure.
The strata are a fixed list of nine and [§1](../../spec/01-strata.md) is the
whole of it.

`descend disk { … }` is how a capability is acquired. It is lexical: the disk
is held for the length of that block and nowhere else. There is no ambient
permission in this language, no global that is already open, and nothing to
forget to close.

`must` turns an answer that might be a refusal into the value, or collapses. It
is the short way to say "I have not written the failure path yet", which is
honest and is checkable — [§9.9](../../spec/09-prelude.md) has the long way.

Also make the file it wants:

```console
$ echo world > who.txt
```

Now bury it.

```console
$ nether bury errand.nc
buried   errand.nc → 4fc5e660   depth 3   holes 1   4 nodes
  hole ①  read("who.txt")                stratum 3  disk
```

The disk is right there. Burial did not read it.

## Burial has no world

This is the part that takes a minute. `nether bury` is not refusing to read the
file, and it is not failing. It has no disk at all, in the same way it has no
terminal. Burial evaluates what can be evaluated with nothing, and where it
reaches something it cannot know, it writes down the question instead of
guessing.

That written-down question is a **hole**. It is a node in the trace like any
other, it has a cairn like any other, and it records what was asked, where it
was asked, and which stratum answering it would reach.

```console
$ nether strata 4fc5e660
depth 3   disk

  3  read("who.txt")           cc948e97:7:36   pending
  0  everything else

  replayable: yes
  of that, 0 (pure) has happened; the rest is what exhuming will cost.
```

Read the last line again. Before anything touches your disk, you have an
itemised account of what it is going to cost: one call, at stratum 3, from line
7 column 36 of a source whose name is `cc948e97`. Not a summary somebody wrote
by hand — the trace, asked a question.

And the greeting is not there yet, because it cannot be:

```console
$ nether lamp 4fc5e660
nothing was deposited
```

`concat(b"Hello, ", name)` needs `name`, `name` is a hole, so the concat is
**starved** and the deposit never happened. Starvation is not an error. It is
the ordinary condition of a program that has not been given what it asked for,
and the trace is complete and correct as it stands.

## What you have

A file that is worth keeping. `4fc5e660` is a full account of everything
`errand.nc` means, minus exactly one fact about the world, with that one fact
named. It will still mean that in ten years. You can hand it to somebody who
does not have your disk.

[Next](04-the-grant.md): giving it the disk.
