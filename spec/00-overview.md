---
section: "00"
title: Overview
status: draft
---

# Overview

Nether C is a C dialect in which nothing runs.

A Nether C program is not executed. It is **buried**: evaluated as far as the
world currently permits, leaving behind a **trace** — a complete, immutable,
content-addressed record of everything that happened — together with the
**holes** where the program asked the world a question that has not yet been
answered. Answering a hole is a separate, later act called **exhumation**, and
it produces a new trace rather than modifying the old one.

That is the whole language. Everything below is consequence.

## 0.1 Why

Most languages draw a line between compile time and run time and then spend
their lifetime negotiating across it: constant folding, macros, `constexpr`,
build systems, staged metaprogramming, reproducible builds, provenance
tracking, incremental compilation. Each is a separate mechanism for the same
underlying question — *how much of this program can we settle now, and what
must wait for the world?*

Nether C makes that question the only question. There is one mechanism, and
the line between what is settled and what is pending is a first-class,
inspectable, addressable thing.

The design is an inversion of HolyC, in which every commitment collapses
toward the present moment of total trust: the command line is the compiler,
top-level statements run as they are read, a bare string prints, and every
task holds every privilege over all of memory at all times. Nether C takes
each of those commitments and turns it over. The reasoning for each inversion,
and what it cost, is in [section 90](90-rationale.md), which is not normative.

The comparison is a reading aid, not the argument. If Nether C only makes
sense to a reader who already knows TempleOS, the design is a costume, and
this document has failed.

## 0.2 The thesis, in one sentence

> Evaluation is not something a program does later; it is something that has
> already happened as far as it could, and the artifact is the record of how
> far that was.

## 0.3 What a Nether C implementation must provide

An implementation MUST provide:

1. A **burier**, which takes source and a set of granted capabilities and
   produces a trace ([section 06](06-evaluation.md)).
2. A **ledger**, a content-addressed store of immutable values and traces
   ([section 07](07-ledger.md)).
3. The **rites**, a command-line interface over the two
   ([section 08](08-rites.md)).

An implementation MUST NOT provide a way to execute a Nether C program that
does not produce a trace. There is no `run`. This is not a missing feature;
it is the single load-bearing constraint of the design, and an implementation
that adds it is not an implementation of this specification.

## 0.4 Reading order

The specification is written to be read in order, but sections 01, 02 and 06
are the ones that carry the design. A reader in a hurry should read those
three and skip the rest until they need it.

| Section | What it settles |
| --- | --- |
| [01 Strata](01-strata.md) | The nine depths, what each grants, what each costs |
| [02 Calculus](02-calculus.md) | The typing rules for depth, `seal`, `shade`, `look` |
| [03 Lexical](03-lexical.md) | Source encoding and tokens |
| [04 Grammar](04-grammar.md) | The complete EBNF |
| [05 Types](05-types.md) | The type system and the data model |
| [06 Evaluation](06-evaluation.md) | Burial, demand, holes, residue, replay |
| [07 Ledger](07-ledger.md) | Canonical encoding, cairns, the trace format |
| [08 Rites](08-rites.md) | The command-line contract |
| [09 Prelude](09-prelude.md) | The standard library, and what each function costs |
| [10 Glossary](10-glossary.md) | Every term, defined once |
| [90 Rationale](90-rationale.md) | Non-normative: why, and what it cost |

## 0.5 Normative language

The key words MUST, MUST NOT, REQUIRED, SHALL, SHALL NOT, SHOULD, SHOULD NOT,
RECOMMENDED, MAY and OPTIONAL are to be interpreted as described in RFC 2119.

Every normative claim appears exactly once, in one section. Where another
section needs it, it links rather than restates. A claim stated twice will
eventually be stated two different ways.

Text in a block quote, and the whole of section 90, is non-normative.

## 0.6 Status

This specification is a **draft**. Sections marked `status: draft` are subject
to change without a version bump. Nothing here is frozen except where a
section says so explicitly, and today only one thing is: **there is no `run`**.

Open design questions are tracked as items in the repository's roadmap
(`cairn list --filter 'title~Decide'`), not as inline TODOs. Where this
document states a rule that a roadmap item is still arguing about, it says so
in a block quote and names the item.

Three such questions have been settled and no longer appear that way: the form
of the Orpheus rule ([§1.6](01-strata.md#16-shade-and-the-orpheus-rule)), the
fuel budget and the `opaque` barrier
([§6.4](06-evaluation.md#64-starvation-and-fuel)), and whether stratum 8 is
admissible at all ([§1.7](01-strata.md#17-stratum-8-the-unrecorded)). What each
of them cost is in [section 90](90-rationale.md).

One question remains genuinely open and is marked as such where it appears:
what a prelude function does when it cannot answer
([§9.9](09-prelude.md#99-failure)).

## 0.7 A first program

```c
// hello.nc — there is no main; there is a demand.

U0 greet()
{
  "Hello from the nether\n";   // deposited, not printed
}

demand greet;
```

```console
$ nether bury hello.nc
buried   hello.nc → 8f3a1c0e   depth 0   holes 0   17 nodes

$ nether lamp 8f3a1c0e
Hello from the nether
```

Burying said nothing about the greeting, and it never can: a Nether C program
holds no capability that reaches a terminal. It left a deposit in the ledger.
Seeing it is a second act, performed by a person who chose to carry a light
down there. See [section 08](08-rites.md) for why observation is a tool rather
than a power.
