---
section: "06"
title: Burial, holes and replay
status: draft
---

# Burial, holes and replay

## 6.1 Burial

**Burial** is the only form of evaluation Nether C has. It takes source and a
set of granted capabilities, and produces a **trace**.

```
bury : Source × Capabilities × Fuel → Trace
```

Burial evaluates every expression it can and stops at every expression it
cannot. It can evaluate an expression when the expression's depth is within
the granted capabilities and every subexpression it depends on has a value. It
cannot when the expression needs the world and the world has not been granted.
Every such point becomes a **hole**.

A trace is complete and immutable. Burial MUST NOT modify a trace it was
given; every burial produces a new trace with a new cairn.

## 6.2 Demand

There is no `main`. A file's top level is a sequence of declarations and
`demand` statements, and nothing is evaluated except what a `demand`
transitively requires.

```c
Bytes@3 src = must(descend disk { read("main.nc") });
Bytes   obj = compile(src);      // pure, but starves: src is a hole
Bytes   unused = compile(other); // never evaluated: nothing demands it

demand obj;
```

`must` here says *this file is not allowed to be missing*. A program that wants
to handle a missing file inspects the `Answer` instead
([§9.9](09-prelude.md#99-failure-and-the-difference-between-two-of-them)).

Evaluation is demand-driven. The implementation MUST NOT evaluate any
expression that no `demand` depends on, transitively — not as an optimisation
but as a semantic guarantee, because evaluating an undemanded expression could
reach the world and leave a witness for something the program never asked for.

Where several `demand`s exist, they are evaluated in source order. Within a
single demand, evaluation order is unspecified **except** that it MUST be
deterministic for a given source and capability set: two burials of the same
input MUST produce the same trace, including the order in which holes were
discovered.

## 6.3 Holes

A **hole** is a suspended world-question. An implementation MUST record, for
each hole:

| Field | Meaning |
| --- | --- |
| `call` | the prelude function and its fully-evaluated arguments |
| `stratum` | the depth the call would reach |
| `span` | the source location that asked |
| `depends` | the cairns of the nodes that must exist for this call to be made |

A hole does **not** record its dependents. It cannot: a node is immutable and
named by its content, so a hole that listed the things waiting on it would get
a new name every time something else came to wait, and every reference to the
old name would be to a hole that no longer exists.

Which nodes are suspended on a hole is derived by reading the graph backwards,
the same way provenance is, and for the same reason
([§7.4](07-ledger.md#74-provenance)).

A hole's arguments are themselves fully buried. `read(concat(dir, name))` does
not leave a hole containing `concat`; it leaves a hole containing the finished
path string. A hole is always a question the world can answer immediately.

A hole is answered by whatever the world says, **including a refusal**. A
refusal is an ordinary witness: it is recorded, it is served back on replay, and
a trace whose program handled it replays identically to one whose program did
not. Refusal does not make a trace incomplete and does not mark it.

Two holes with identical `call` fields in the same trace MUST be the same
hole. This is what makes exhumation cheap: reading the same file twice is one
question, asked once.

The hole keeps the `span` of the first place that asked, in the order
[§6.2](#62-demand) fixes. Which place that is, is therefore a fact about the
program rather than about the implementation, and two burials of the same
source agree on it. A hole is a question for the world, and the world does not
care how many places were waiting on the answer.

## 6.4 Starvation and fuel

An expression **starves** when it cannot be evaluated and cannot become a hole
— because it depends, transitively, on a hole. Starvation is normal; a starved
expression is residualised and waits.

An expression that can never produce a value has not starved. It has
**collapsed**, and a burial that hits one caves in rather than waiting
([§9.9](09-prelude.md#99-failure-and-the-difference-between-two-of-them)).

Burial takes a **fuel budget**: a bound on the number of evaluation steps.

A **step** is one evaluation of one expression node. It is charged when
evaluation of that node begins, before anything else happens to it, and it is
charged once. Three consequences, because each is a place two implementations
could otherwise disagree:

- A unit-level binding is evaluated at most once, however many times it is
  named, and the steps its value costs are charged to the first demand that
  reaches it.
- `opaque e` costs one step and `e` costs nothing, because `e` is not
  evaluated.
- A residue costs nothing for what has already been reduced, which is what
  makes the staging law in [§6.5](#65-residue) affordable rather than merely
  true.

Fuel accounting MUST be deterministic — the same source and capabilities must
exhaust at exactly the same point on every implementation and every machine.
That is a requirement about agreement between implementations and not only
with oneself, which is why the step is defined here rather than left to one.

Exhausting fuel is a diagnostic, not a crash. The implementation MUST report
the source span at which fuel ran out and the shape of what was being
evaluated:

```
error: burial ran out of fuel after 1,000,000 steps
  --> lib/table.nc:31:3
   |
31 |   for (I64 i = 0; i < n; i++)
   |   ^^^ unrolled 262,144 times; `n` is a literal, so this will not stop
   |
   = raise the budget with --fuel, or place an `opaque` barrier at line 29
```

Burial has one other bound, and it is the implementation's own. Fuel is a
bound on work; it is not a bound on space, and no single budget is both — a
program can spend a million steps a thousand frames deep or a million steps
two frames deep. So an implementation has limits of its own: how deep a chain
of calls it can hold, how large a value it can address.

Reaching one MUST be reported the way exhausted fuel is — naming the limit,
its value, and where it was reached — and MUST NOT be a crash. An
implementation MUST state its limits.

Two implementations with different limits may therefore disagree about whether
a given program buries at all. They MUST NOT disagree about the result when
both of them finish, and that is the sentence the reproducibility claims in
this document actually rest on.

`opaque e` evaluates to `e` but is never burned through: burial residualises
it whole. It is how a programmer says *do not evaluate through this, even
though you could* — for a loop that would unroll into a gigabyte, or a
function whose specialised form would be larger than its general one.

`opaque` has a typing rule ([OPAQUE], [section 02](02-calculus.md)) rather than
being a compiler flag, because whether an expression is burned through changes
the artifact, and anything that changes the artifact belongs in the language.

This is **settled**. The rejected alternative — leaving both the budget and the
barrier to the command line — is recorded in
[§90.2](90-rationale.md#902-rejected-alternatives).

## 6.5 Residue

What survives burial is the **residue**: every node that could not be reduced,
plus the holes, plus the provenance edges between them.

The residue is a complete program. It can be buried again, with more
capabilities, and the result is the same as if those capabilities had been
granted the first time:

> **Staging law.** For capability sets *A* and *B*,
> `bury(bury(p, A), B) ≡ bury(p, A ∪ B)`.

An implementation MUST satisfy this. It is what makes descent incremental
rather than a single all-or-nothing act, and it is the property `nether graft`
relies on.

## 6.6 Exhumation

**Exhumation** grants a capability, answers the holes it can, records every
answer, and buries the residue again.

```console
$ nether bury build.nc
buried   build.nc → 4c02ab7f   depth 3   holes 1   903 nodes
  hole ①  read("main.nc")              stratum 3  disk

$ nether exhume 4c02ab7f --grant disk
  ①  read("main.nc")  →  11,204 bytes  a1f0c93d
sealed   4c02ab7f + a1f0c93d → 77de9b31   depth 3   holes 0
```

The result is a new trace with a new cairn. The original trace still exists,
unchanged, and still has its hole. Nothing in the ledger is ever revised.

A trace with no holes is **sealed**. A sealed trace has a value.

## 6.7 Replay

```console
$ nether exhume 77de9b31 --replay
identical.
```

Replay re-buries a trace using only the answers already recorded in the
ledger. The implementation MUST hold no capabilities at all during replay: it
does not prefer the ledger over the world, it cannot reach the world.

> **Replay law.** Replaying a sealed trace produces a trace with the same
> cairn, unless the original trace is marked as having reached stratum 8.

"Identical" means byte-identical under the canonical encoding of
[section 07](07-ledger.md) — not merely equal in value. A trace that replays to
a different cairn is a bug in the implementation, and `nether exhume --replay`
MUST report the first differing node rather than only the mismatch.

Stratum 7 (entropy) does not break replay: the drawn value is a recorded
witness like any other, and replaying serves it back. What entropy breaks is
*re-derivation* — burying the same source again produces a different trace —
which is a different and weaker property that Nether C does not promise for
programs that reached stratum 7.

Stratum 8 breaks replay outright, because there is no witness to serve back.
See [§1.7](01-strata.md#17-stratum-8-the-unrecorded).

## 6.8 What burial prints

Burial prints a summary of what it wrote, and nothing else:

```
buried   <source> → <cairn>   depth <n>   holes <n>   <n> nodes
```

It MUST NOT print any value the program deposited. A program's deposits are
read with `nether lamp` ([section 08](08-rites.md)), by a person who decided to
go and look. This is the inversion of HolyC's bare-string-prints, and it is
the reason a Nether C program cannot leak to a log: it holds no capability
that reaches one.
