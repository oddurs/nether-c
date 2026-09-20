---
section: "01"
title: The first burial
status: draft
---

# The first burial

> One idea: **a deposit is not a print.**

Put this in a file called `hello.nc`.

```c
// hello.nc — there is no main; there is a demand.

U0 greet()
{
  "Hello from the nether\n";   // deposited, not printed
}

demand greet();
```

Two things in it are not C.

There is no `main`. A Nether C file does not have an entry point, because
there is no entry: nothing is entered. What it has is a **demand**, and a
demand is the request that something be worked out. Nothing that is not
demanded, directly or through something that is, is evaluated at all.

And the string on its own line is a statement whose value is not `U0`. Such a
statement **deposits** its value into the trace. It is not written to your
terminal. A program has no terminal.

Bury it.

```console
$ nether bury hello.nc
buried   hello.nc → efed9b2a   depth 0   holes 0   4 nodes
```

Read that line across. `hello.nc` was evaluated and what came out was named
`efed9b2a`. Its **depth** is 0, which means nothing in it touched the world.
It has no **holes**, which means nothing in it is still waiting on an answer.
It is four nodes in the ledger.

Nothing was printed, and nothing is going to be. Burial does not have a
terminal either. To read what the program left, go and get it:

```console
$ nether lamp efed9b2a
Hello from the nether
```

That is the whole of the idea. The greeting exists in the ledger because the
program put it there. It reached your screen because *you* decided to look —
a second act, by a person, after the fact.

## Why this is worth the trouble

A program that can print can also print at three in the morning into a log
nobody reads, or into a log everybody reads. It can print a password. It can
print different things depending on whether it thinks it is being watched.

A Nether C program cannot do any of that, and not because it is discouraged
from it. There is no prelude function that reaches a terminal. `lamp` is a tool
an operator carries; it is not a capability a program holds. The separation is
structural, and it is why [§8.4](../../spec/08-rites.md#84-lamp) spends a
paragraph on it.

## What `depth 0` bought you

```console
$ nether strata efed9b2a
depth 0   pure

  nothing reached the world

  replayable: yes
```

`strata` answers one question: how far down did this go, and what took it
there. This program went nowhere. Everything it says it did, it did with
arithmetic and literals, and anybody can check that by burying the same file
and comparing names.

That is the floor. [Next](02-a-name-that-holds.md) is what a name is.
