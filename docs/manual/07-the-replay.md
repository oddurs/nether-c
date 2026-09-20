---
section: "07"
title: The replay
status: draft
---

# The replay

> One idea: **a replay is a question about the past, not a second run.**

`f03489d0` from [chapter 4](04-the-grant.md) was made by asking the disk what
was in `who.txt`. Ask whether that is still true:

```console
$ nether exhume 4fc5e660 --grant disk
  ①  read("who.txt")  →  6 bytes  4395a792
sealed   4fc5e660 + 4395a792 → f03489d0   depth 3   holes 0
$ nether exhume f03489d0 --replay
identical.
```

One word, and it is the right one. Not "success", not "0 differences" — the
world was asked the same questions and gave the same answers, so the trace a
replay would reach is the trace you already have, and it has the same name.

Change `who.txt` and ask again and it will not be identical, and the difference
will be itemised by question: this call, this answer then, this answer now.

## What replay is not

It is not running the program again. The program was never run. There is
nothing to run and no second execution to compare against a first.

It is not checking a signature either. Nothing was signed. The trace names its
answers by content, so asking the world the same questions and hashing what
comes back is the whole of the check. Agreement is the names matching.

And it is not a cache. A cache tells you a previous answer so you can skip the
work. A replay does the work — it actually asks the disk — and tells you
whether the world changed. Those look similar and are opposite: one is for
going faster and one is for finding out you were wrong.

## Why `--replay` and `--grant` are the same rite

Both are `exhume`. Both take a trace and a world and produce the trace that
world implies. The only difference is that one starts from holes and the other
starts from answers already given.

That is not a coincidence in the implementation. It is
[§6.7](../../spec/06-evaluation.md), which says burial is a staged evaluation
and each stage is the same operation applied to what is left. Exhuming a hole
and replaying a witness are one thing asked twice.

## What this is for

Three questions this answers that are normally guesswork:

- **Is this artifact still the artifact?** Replay it. If the world has not
  moved, you get its own name back.
- **Which input changed?** The difference is per-question, not per-file.
- **Can I reproduce this at all?** `strata` says `replayable: yes` or it does
  not, and it says so before you try. A trace that reached stratum 8 — foreign
  code — is honest about being unrepeatable, because
  [§1.7](../../spec/01-strata.md) marks it forever.

[Next](08-the-orpheus-rule.md): the rule with the best name.
