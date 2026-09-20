---
section: "04"
title: The grant
status: draft
---

# The grant

> One idea: **a capability is given, never ambient.**

`errand.nc` asked the disk a question. Answering it is a separate rite, run by
a person who decided to answer it, and that rite is `exhume`.

Try it with nothing:

```console
$ nether exhume 4fc5e660
unchanged 4fc5e660   depth 0   holes 1

nothing was granted, so nothing was answered. §9.1 names the capability each hole's stratum asks for.
```

*Unchanged.* Not an error — you asked for the trace to be taken further and it
could not be taken anywhere, so it is still itself, and the ledger did not grow.

Now grant the disk:

```console
$ nether exhume 4fc5e660 --grant disk
  ①  read("who.txt")  →  6 bytes  4395a792
sealed   4fc5e660 + 4395a792 → f03489d0   depth 3   holes 0
```

Read that second line as arithmetic, because that is what it is. The trace you
had, plus the answer the world gave, equals a new trace. All three have names.
None of them replaced any other — `4fc5e660` is still in the ledger, still has
its hole, and still means exactly what it meant.

And now:

```console
$ nether lamp f03489d0
Hello, world
```

## Nothing is open by default

`--grant disk` is the only reason that read happened. There is no configuration
file that could have turned it on, no environment variable, no inherited
handle. A rite with no grants can do nothing observable, and that is not a
hardened mode — it is the ordinary case, and every other case is something a
person typed.

Compare this with how it usually goes. A process starts with the filesystem,
the network, the clock and the environment already in hand, and the question
"what can this program reach" has no answer short of reading all of it. Here
the question is answered by the command line, and `strata` will tell you what
the program *wanted* before you decide what to give it.

That ordering matters: you saw the itemised cost in
[chapter 3](03-the-hole.md) *before* granting anything. The account comes
first, the permission second.

## The grant is not the capability

`--grant disk` does not open your whole disk to the program. It says this rite
may answer the disk questions this trace already wrote down. The trace asked
for `who.txt` at burial time, in a file you can read, and it cannot decide
during exhuming to ask for something else — a hole is a question that was
already settled, not an open line.

## `descend` and `--grant` are two halves

The program's `descend disk { … }` says *this code may ask*. Your `--grant
disk` says *and I will answer*. Neither is enough alone, they are written by
different people at different times, and both are visible in plain text.

[Next](05-the-witness.md): what the answer left behind.
