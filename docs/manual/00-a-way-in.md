---
section: "00"
title: A way in
status: draft
---

# A way in

The specification says what the rules are. It says each of them exactly once,
in the order a rule needs the ones above it, and it is not a tutorial and will
not become one. This is the other document. It answers *what do I type*.

Read it at a terminal with the files in front of you. Every command on these
pages is executed on every build of this project, against the binary in this
repository, and the output printed underneath is the output that came back. If
a line here is wrong, the build is red. That is the only promise this manual
makes, and it is the one worth making.

## What you need

A `nether` binary. Either take one from a release or build it:

```console
$ cargo build --release --bin nether
```

Then a directory to work in, and nothing else. There is no project file, no
manifest and no daemon. A ledger appears beside your work the first time
something needs one.

## What you will have done by the end

Ten short chapters, each with one idea in it:

| | | |
|---|---|---|
| 01 | [The first burial](01-the-first-burial.md) | a deposit is not a print |
| 02 | [A name that holds](02-a-name-that-holds.md) | the name is the thing |
| 03 | [The hole](03-the-hole.md) | what the world has not said is written down |
| 04 | [The grant](04-the-grant.md) | a capability is given, never ambient |
| 05 | [The witness](05-the-witness.md) | what the world said is kept beside what it made |
| 06 | [Two worlds](06-two-worlds.md) | one burial, as many worlds as you like |
| 07 | [The replay](07-the-replay.md) | a question about the past, not a second run |
| 08 | [The Orpheus rule](08-the-orpheus-rule.md) | carry it up, go back down to look |
| 09 | [Where to go from here](09-where-to-go-from-here.md) | the map |

You will write four programs, bury each of them, and read back what each one
left. None of them will run, because nothing in this language runs.

## The one thing to know first

A Nether C program is not executed. It is **buried**: evaluated as far as the
world currently permits. What it leaves behind is a **trace** — everything that
could be worked out, plus a **hole** for every question the world has not
answered yet.

That is not a limitation with a workaround. It is the whole design. A program
that cannot run cannot lie about what it did, and a program that must write
down every answer it was given can be asked, years later, what it was told.

Start at [the first burial](01-the-first-burial.md).
