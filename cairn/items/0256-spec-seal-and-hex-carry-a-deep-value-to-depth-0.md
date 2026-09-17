---
id: 256
title: 'Spec: seal and hex carry a deep value to depth 0'
type: spec
status: buried
milestone: codex
assignee: Oddur Sigurdsson
created: 2026-09-16
updated: 2026-09-16
priority: p1
effort: s
stratum: '0'
area: spec/01-strata.md
proof: Neither §1.5 nor §2.3 claims a deep value is confined
---

## The channel

`spec/01-strata.md` §1.5 gives `seal e : Cairn@0` for any `e`, and
[§9.2](../spec/09-prelude.md) gives `Str hex(Cairn c) @0`:

```c
Bytes@3 src = must(descend disk { read("secret") });
demand hex(seal src);        // Str@0, deposited
```

Two hundred and fifty-six bits of a depth-3 file, at depth 0. And because §5.3
makes equality a cairn comparison:

```c
Bool@0 it_is = (seal src == #<sixty-four hex digits>);
if (it_is) { /* a depth-0 branch decided by a depth-3 value */ }
```

A low-entropy deep value — a version string, a four-digit number, a file that
is one of three — is recoverable at depth 0 by guessing, holding no capability
and writing no witness.

## Why this is not a bug in §1.5

It is not. `seal` is sound for what depth actually tracks: **where a value came
from**. No witness is missing, nothing in the trace is falsified, and §1.2 is
untouched.

The defect is that `spec/02-calculus.md` §2.3 argues for something else:

> The condition belongs in that maximum because which arm was taken is itself
> something the condition knew — `if (secret) { 0 } else { 1 }` tells you about
> `secret` whichever arm runs.

That is an information-flow argument, and it is the only justification [PRIM]
offers for including the condition. A reader who takes it at face value has
been told the lattice confines what a deep value can reveal — one section
before `seal` hands them a thirty-two byte summary of one and `hex` turns it
into text.

§1.5's own defence has the same shape: "Knowing that a file's contents hash to
`a1f0c93d` tells you nothing about the file that you could not have computed
yourself given the same bytes." True of a *name*. Equality on that name is a
decision procedure over the contents.

## What to write

A paragraph in §1.5 saying what depth is a claim about and what it is not, and
a justification in §2.3 for [PRIM] that does not appeal to secrecy: the
condition is in the maximum because the result's *history* reaches through it,
which is provenance and is the thing being tracked.

§90.3 gains the exclusion, stated as plainly as the others: depth is not a
confidentiality lattice, and a program that must not reveal a deep value cannot
be checked by this type system.

## Acceptance criteria

- [x] §1.5 says what depth claims and what it does not
- [x] §2.3's [PRIM] justification does not rest on what an observer learns
- [x] §90.3 lists the exclusion beside the other honest ones
- [x] `spec/10-glossary.md`'s entry for depth agrees

## 2026-09-16

Written. §1.5 now states the rule in a block quote -- depth is a claim about where a value has been and about nothing else, and not a claim about what a deep value can be made to reveal -- and then demonstrates the gap with a real sample rather than the item's sketch: a depth-3 read sealed and compared against a cairn literal, giving a Bool@0. It says plainly that none of that is a hole in the rule, only in the rule a reader might have inferred. §2.3's [PRIM] justification no longer appeals to what an observer learns; the condition is in the maximum because the result's history runs through it -- a value that exists because a depth-3 file said one thing rather than another is a value the disk was consulted for, and a trace calling it pure would owe no witness for a read that happened -- and it points at §1.5 for the thing depth does not do. §90.3 gains 'Keeping a secret' beside the other exclusions, and says why the confusion is natural: a lattice is what a confidentiality type system looks like too. The glossary's Depth and Seal entries both agree. The new sample is registered in crates/nether-syntax/tests/spec/mod.rs as Statements and recorded in tests/transcripts/MANIFEST.tsv, so it parses and checks like every other sample in the specification. scripts/task check passes in full. No code changed and none needed to: the item is right that seal is sound and the defect was the justification beside it.
