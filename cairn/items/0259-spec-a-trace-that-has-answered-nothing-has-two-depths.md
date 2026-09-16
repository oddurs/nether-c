---
id: 259
title: 'Spec: a trace that has answered nothing has two depths'
type: spec
status: unmarked
milestone: codex
created: 2026-09-16
updated: 2026-09-16
priority: p1
effort: s
stratum: '0'
area: spec/08-rites.md
proof: §8.6 prose and its own transcript agree with §7.3.2
---

## The two readings, four lines apart

`spec/08-rites.md` §8.6:

```console
$ nether strata 76c4b655
depth 3   disk
```

and, below that transcript:

> A trace that has answered nothing has depth 0 and says what exhuming it will
> cost.

The transcript is of exactly such a trace — §8.2's, with one unanswered `read`
— and it prints 3.

`spec/07-ledger.md` §7.3.2 settles it, and names this case:

> A stage-one burial has answered nothing, so its witnesses say 0 and its
> residue says how deep the program still reaches — which is why §8.2's
> transcript reads `depth 3` over a program with one unanswered `read`.

So §8.6's sentence is the one that is wrong. It is reaching for *reached*
depth — the line under it, "of that, 0 (pure) has happened", is the claim it
meant to make — and it spends the word `depth`, which §7.3.2 owns.

This is the failure [§0.5](../spec/00-overview.md) exists to prevent, in a
document that states the rule: one claim, two places, two shapes.

## Acceptance criteria

- [ ] §8.6 does not restate what a trace's depth is; it links §7.3.2
- [ ] §8.6's prose and its own transcript agree
- [ ] A transcript covers `strata` on a trace with a hole and no witnesses
