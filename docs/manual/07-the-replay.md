---
section: "07"
title: The replay
status: draft
---

# The replay

> One idea: **a replay is a question about the past, not a second run.**

`49df3bdc` from [chapter 4](04-the-grant.md) was made by asking the disk what
was in `who.txt`. Replay it with no capabilities at all:

```console
$ nether exhume 4fc5e660 --grant disk --root .
  ①  read("who.txt")  →  6 bytes  4395a792
sealed   4fc5e660 + 4395a792 → 49df3bdc   depth 3   holes 0
$ nether exhume 49df3bdc --replay
identical.
```

One word, and it is the right one. Not "success", not "0 differences" —
re-burying the trace solely from the witnesses already recorded in the ledger
produced the same trace, byte for byte, with the same cairn.

Change or delete `who.txt` and ask again and it is still `identical.`.
Replay holds **no capabilities at all**
([§6.7](../../spec/06-evaluation.md#67-replay),
[§8.3](../../spec/08-rites.md#83-exhume)): it never asks the disk and cannot
reach the world. Every answer comes from the ledger witnesses stored beside the
trace. To check whether the disk has changed since, exhume the earlier trace
`4fc5e660` against `--grant disk --root .` again and compare the new sealed
cairn with `49df3bdc`.

## What replay is not

It is not running the program again. The program was never run. There is
nothing to run and no second execution to compare against a first.

It is not checking a signature either. Nothing was signed. Every witness is
named by its content, and replay re-buries the residue from those recorded
witnesses alone. Agreement is the re-buried trace hashing to the same cairn,
byte for byte; if it diverges, `exhume --replay` reports the first differing
node.

And it is not asking the world again. A replay holds no grants and cannot touch
the disk, the network, or the clock, even on a machine where `who.txt` never
existed. It verifies deterministic reproduction from recorded witnesses, not
whether the outside world stood still.

## Why `--replay` and `--grant` are the same rite

Both are `exhume`. One grants capabilities to ask the world for open holes; the
other grants no capabilities and serves every answer from witnesses already in
the ledger. Both feed answers into burial of the residue and produce a trace.

That is not a coincidence in the implementation. It is
[§6.7](../../spec/06-evaluation.md#67-replay), which says burial is a staged
evaluation and each stage is the same operation applied to what is left.
Exhuming a hole and replaying a witness are one thing asked twice.

## What this is for

Three questions this answers that are normally guesswork:

- **Does this trace still reproduce?** Replay it. Holding no capabilities,
  re-burying from its recorded witnesses gives its own cairn back — even after
  the original file is changed or gone.
- **Where did reproduction diverge?** If a bug or change makes a replay produce
  a different trace, `exhume --replay` names the first differing node rather
  than only a mismatched cairn.
- **Can I reproduce this at all?** `strata` says `replayable: yes` or it does
  not, and it says so before you try. A trace that reached stratum 8 — foreign
  code — is honest about being unrepeatable, because
  [§1.7](../../spec/01-strata.md) marks it forever.

[Next](08-the-orpheus-rule.md): the rule with the best name.
