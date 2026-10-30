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

The design's history, rejected alternatives, and costs are in
[section 90](90-rationale.md), which is not normative. This specification
stands on its own: no knowledge of another language is required.

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
to change without a version bump. Nothing here is frozen except where a section
says so explicitly, and three do: the canonical encoding
([§7.1](07-ledger.md#71-canonical-encoding)), the node encoding on the same
terms ([§7.3.1](07-ledger.md#731-node-encoding)), and the one frozen
*requirement*, which is that **there is no `run`** ([§8.0](08-rites.md#80-there-is-no-run)).

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

A fourth — what a prelude function does when it cannot answer — is settled in
[§9.9](09-prelude.md#99-failure-and-the-difference-between-two-of-them): a no
from the world is an *answer* and gets a value; a mistake in the program is
*collapse* and stops the burial.

Which questions are open is a fact about the roadmap and not about this
document, so this section does not count them. The filter above lists them, and
each is named where it bites: a rule a roadmap item is still arguing about is
block-quoted in its own section, beside the rule, where somebody relying on it
will see it rather than somewhere they have to think to look.

Settling one is not the same as knowing they are all settled. A question nobody
has asked yet leaves no trace here.

### 0.6.1 What a release promises

A trace is a fact about the past: this implementation, given this source and
these answers, wrote this. A later release of the language cannot make that
untrue, and it is not asked to keep it *reproducible* by every later
implementation either. What a trace written by an older release may still claim
is four things, and each has its own answer.

| A trace written by an older release is | when |
| --- | --- |
| **readable** | its domain separator is this implementation's ([§7.7](07-ledger.md#77-versioning)) |
| **verifiable** — its cairns check | the same: a cairn is checked under the domain it was written in |
| **replayable** ([§6.7](06-evaluation.md#67-replay)) | by the implementation that wrote it, and by any later one whose burial of that residue is unchanged |
| **true** | always. Its depth, holes and witnesses say what that implementation did, and nothing later reinterprets them |

Replay is the one that a language change can take away, because a residue is
source ([§6.5](06-evaluation.md#65-residue)) and replay buries it again. So the
promise is not that every release replays every old trace. It is that the
release which wrote one can always be had again: a release binary is built
reproducibly and its hash published (`scripts/task release:build`), and that
binary replays its own traces for as long as anybody keeps one.

After 1.0, a release follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
by these rules, layer by layer:

- **Encoding.** Any change to §7.1 or §7.3.1 moves the domain separator, and a
  moved domain is a **major** release. There is no minor change to a frozen
  format.
- **Syntax and meaning.** A change after which a program that buried before
  buries to a different trace, or no longer lowers, is **major**. A change that
  only lets more programs lower is **minor**.
- **Prelude.** Adding a function is **minor**. Changing a function's signature,
  its stratum, or what it answers is **major**, because a trace's holes name
  prelude calls and their strata.
- **Rites.** A flag that changes what a rite writes into the store is
  **major**. One that changes only what it prints is **minor**, and what it
  prints in `--json` is part of what it writes for this purpose.

A major release says, in its changelog, which of the four claims above its
older traces lose, and the rejected alternative to the change goes in
[§90.2](90-rationale.md#902-rejected-alternatives) as every design decision's
does. Before 1.0 the draft status above holds and none of this binds; it is
written down now so that 1.0 is a decision to start keeping it rather than a
decision about what it says.

## 0.7 A first program

```c
// hello.nc — there is no main; there is a demand.

U0 greet()
{
  "Hello from the nether\n";   // deposited, not printed
}

demand greet();
```

```console
$ nether bury hello.nc
buried   hello.nc → efed9b2a   depth 0   holes 0   4 nodes

$ nether lamp efed9b2a
Hello from the nether
```

Burying said nothing about the greeting, and it never can: a Nether C program
holds no capability that reaches a terminal. It left a deposit in the ledger.
Seeing it is a second act, performed by a person who chose to carry a light
down there. See [section 08](08-rites.md) for why observation is a tool rather
than a power.
