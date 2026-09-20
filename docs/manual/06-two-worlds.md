---
section: "06"
title: Two worlds
status: draft
---

# Two worlds

> One idea: **one burial, as many worlds as you like.**

Everything so far has been one program and one answer. Now the same trace
against two different worlds. Write `post.nc`:

```c
// post.nc — one burial, and as many worlds as you care to try.
//
// `env` is stratum 2 and `--declare` is how a rite answers it, so the world
// this program runs against fits on the command line. Two exhumings of one
// trace reach two sealed traces, and both keep their names.
//
// One deposit, not three. A trace whose deposits come from two sources reads
// back in an order §8.4 decides by sorting cairns, and 0274 is about that.

Str@2 who = must(descend env { env("WHO") });

U0 greet()
{
  concat("Hello, ", who);
}

demand greet();
```

`env` is stratum 2 — shallower than the disk, because reading a frozen
environment costs nothing and changes nothing. Bury it once:

```console
$ nether bury post.nc
buried   post.nc → 50893e26   depth 2   holes 1   4 nodes
  hole ①  env("WHO")                     stratum 2  env
```

Now try to answer it, and get told off:

```console
$ nether exhume 50893e26 --grant env
nether: granting `env` means pinning it: `--clock` and `--target`.

  §8.3.1: there is no default, because a default would be this
  machine's, and a rite whose answer depends on which machine ran
  it is what §6.7 exists to prevent.
```

That message is worth stopping on. Granting the environment means the rite has
to say what the environment *is* — including the time and the target triple —
because a default would quietly be this machine's, and then the trace would
depend on which machine happened to run it. There is no convenience here on
purpose. Somewhere there is a reproducible build that is only reproducible on
the laptop it was invented on, and this is that bug, refused at the door.

So say what the world is:

```console
$ nether exhume 50893e26 --grant env --declare WHO=world --clock 0 --target x86_64-unknown-linux-gnu
  ①  env("WHO")  →  world  10960a1b
sealed   50893e26 + 10960a1b → 2d9de1c3   depth 2   holes 0
```

And then say it differently:

```console
$ nether exhume 50893e26 --grant env --declare WHO=moon --clock 0 --target x86_64-unknown-linux-gnu
  ①  env("WHO")  →  moon  c26253b8
sealed   50893e26 + c26253b8 → 45b71fe3   depth 2   holes 0
```

Two rites, one burial, two worlds. Both of them are still there:

```console
$ nether lamp 2d9de1c3
Hello, world
$ nether lamp 45b71fe3
Hello, moon
```

## What did not happen

`post.nc` was parsed, checked and evaluated once. The second exhuming did not
re-read the file, did not re-typecheck it, and did not redo the part of the
work that does not depend on `WHO`. It answered a hole in a trace it already
had.

That is the ordinary shape of work here. Bury the expensive, deterministic part
once and keep its name. Answer the part that depends on the world as many times
as there are worlds you care about. Nothing is invalidated, because nothing was
ever overwritten — `50893e26` still has its hole, and will have it forever.

## The names are the record

`2d9de1c3` and `45b71fe3` are different names because they are different
things, and they are different things because they were told different things.
The environment that produced each is not a note somebody kept; it is reachable
from the trace, through the witness, by anyone holding the name.

If you built something for two customers and one of them says the wrong thing
shipped, the question "which world did this come out of" has an answer, and the
answer is not "let me check the CI logs".

[Next](07-the-replay.md): asking a trace whether the world has moved.
