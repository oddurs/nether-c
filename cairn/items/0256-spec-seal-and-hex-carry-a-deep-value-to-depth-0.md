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

Neither seal nor the lattice was wrong. §2.3 was: it justified PRIM's condition term with an information-flow argument, which is the one thing the lattice does not do. PRIM takes the condition because the result's history reaches through it — provenance — and §1.5 now says outright that depth records where a value came from and does not bound what it reveals.

## 2026-09-16

Two ways to make the confinement reading true were rejected and are in §90.2. Giving seal a depth deletes the escape, since a name as deep as what it names is the value with fewer bits and every trace is built from cairns held at 0. Forbidding cairn comparison breaks equality itself (§5.3) and closes only the deciding half; hex still hands over the bits.
