---
section: "01"
title: The strata
status: draft
---

# The strata

Every value in Nether C carries a **depth**: a natural number between 0 and 8
recording how far into the world its history reaches. Every expression has a
depth. Every function's type includes the deepest stratum it may reach.

Depth is not a permission that is checked and forgotten. It is part of the
type, it propagates through every operation, and it is recorded permanently in
the trace.

## 1.1 The lattice

The nine strata form a total order, 0 ≤ 1 ≤ … ≤ 8. Depth composes by
maximum: an expression built from parts is exactly as deep as its deepest
part.

| Depth | Name | Grants | Costs | Witness obligation |
| ---: | --- | --- | --- | --- |
| 0 | **Pure** | arithmetic, data, functions | nothing | none |
| 1 | **Store** | reading the ledger by cairn | nothing observable | the cairn read |
| 2 | **Frozen Environment** | declared variables, a pinned clock, the target triple | the value must be declared in advance | the declaration and the value |
| 3 | **Read the Disk** | reading files | the result depends on a filesystem | path, bytes read, and their cairn |
| 4 | **Write the Disk** | creating and modifying files | irreversibility outside the ledger | path and the cairn of what was written |
| 5 | **Read the Network** | fetching | the result depends on a remote party | request, response, and its cairn |
| 6 | **Write the Network** | sending | the world remembers what was said | request and the response, if any |
| 7 | **Entropy** | true randomness | replayability | the drawn value |
| 8 | **Unrecorded** | foreign code | provenance itself | nothing can be recorded |

Strata 0 through 7 are **recordable**: everything the world answered can be
written to the ledger and served back later, so a trace that reached them can
be replayed exactly. Stratum 8 is not, and is treated separately in §1.7.

## 1.2 The monotonicity law

> **Law.** No evaluation step lowers the depth of a value.

This is the one invariant the whole design rests on. An implementation MUST
NOT provide any operation that produces a value of lower depth than its
inputs, except the two escapes defined in §1.5 and §1.6, both of which carry
a name or an opacity rather than the value itself.

There is no `ascend`.

## 1.3 Descent

```c
Bytes@3 src = descend disk { read("kernel.nc") };
```

`descend κ { … }` is an **expression**. It acquires the capability named κ for
the duration of the block, evaluates the block, and takes the value of the
block's tail expression. The body is checked at depth `max(δ, stratum(κ))`,
where `δ` is the ambient depth of the enclosing scope.

A `descend` whose tail expression is `U0` may be used as a statement.

- Descent is lexically scoped: the capability is not available outside the
  block, and neither are the block's local bindings. Only the tail value
  leaves, carrying its depth with it.
- Descent is one-way *within* the block: no construct inside the block returns
  the scope to a shallower depth.
- Descents nest, and nesting takes the maximum. `descend disk { descend net {
  … } }` has a body at depth 5.
- A `descend` whose body reaches no deeper than the enclosing scope is legal
  and has no effect. An implementation SHOULD warn about it.

The capability names and their strata are fixed by this specification and
listed in [section 09](09-prelude.md). An implementation MUST NOT define
additional capability names; a capability that is not in the prelude cannot be
audited by a reader of this document, which defeats the purpose of naming them
at all.

## 1.4 What the trace records

When an expression at depth *d* > 0 is evaluated, the implementation MUST
write to the ledger, before the value is returned to the program:

1. the **witness** listed for that stratum in the table in §1.1;
2. the source span of the expression that reached it;
3. the depth itself.

The order matters. An answer that is returned before it is recorded is an
answer that can be lost, and a ledger with a gap in it cannot support any of
the guarantees in [section 06](06-evaluation.md).

## 1.5 Seal

```c
Cairn id = seal src;    // src : Bytes@3, id : Cairn@0
```

`seal e` produces the **cairn** of `e` — the content address of its value.
The result has depth 0 regardless of the depth of `e`.

This is sound because a cairn is a name, and a name is pure. Knowing that a
file's contents hash to `a1f0c93d` tells you nothing about the file that you
could not have computed yourself given the same bytes; it is a claim *about*
the deep value, not the deep value.

`seal` on a shade (§1.6) is legal and yields the cairn of the underlying
value.

## 1.6 Shade, and the Orpheus rule

```c
Shade<Json> reply = descend net { shade fetch("https://example.invalid/index.json") };

Cairn witness = seal reply;      // legal: Cairn@0
I64   n       = look(reply).len; // ILLEGAL here — see below
```

`shade e`, where `e : T@d`, produces a value of type `Shade<T>` at depth 0.
The shade is **opaque**: it MAY be stored, copied, passed, compared for
equality, and sealed. It MUST NOT be destructured, pattern-matched, or
otherwise observed.

`look s`, where `s : Shade<T>` originating at depth `d`, has type `T@d`, and
is well-typed **only where the current descent depth is at least `d`**.

> **The Orpheus rule.** You may carry a shade up out of any stratum. You may
> only look at it by going back down.

The check is local: it compares the shade's origin depth against the ambient
depth at the point of the `look`. It does not propagate, it does not stain the
enclosing scope, and it produces an error that names both numbers and the
`descend` that would fix it.

```
error: cannot look at a shade from stratum 5 at depth 0
  --> stamp.nc:14:11
   |
14 |   I64   n       = look(reply).len;
   |                   ^^^^^^^^^^^ this shade came from `fetch` at stratum 5
   |
   = the value is here, but you are not. Wrap the look in `descend net { … }`.
```

> Two rejected alternatives — re-staining the entire enclosing scope, and
> tainting only the binding — are recorded in [section 90](90-rationale.md).
> The roadmap item is *Decide: does look re-stain the scope, or taint the
> binding?*, which this rule proposes to close.

## 1.7 Stratum 8, the Unrecorded

Stratum 8 is foreign code: anything whose behaviour the implementation cannot
observe well enough to record a witness for.

It is in the lattice because a language that cannot call C is a language
nobody can adopt. It is at the bottom because it is the only stratum that
breaks the language's central promise rather than merely deepening it: a trace
that reached stratum 8 has a gap in it, and no amount of later care can fill
that gap in.

An implementation MUST:

- mark any trace that reached stratum 8, permanently and transitively, in the
  trace itself;
- refuse to report such a trace as replayable, in every rite that reports
  replayability;
- surface the mark in `nether strata` output without being asked.

A value derived from an unrecorded call is itself at depth 8. There is no
sealing or shading out of stratum 8 that removes the mark — `seal` still
yields a `Cairn@0`, but the *trace* remains marked, because what is unsound is
not the name but the claim that the trace is complete.

> The roadmap item *Decide: is stratum 8 admissible at all?* is open. This
> section states the position that it is, quarantined loudly. The alternative
> — refusing foreign code entirely — is purer and is recorded in section 90.

## 1.8 Why write is deeper than read

Strata 4 and 6 sit below 3 and 5 because the ordering is *how much of the
world you have disturbed*, not *how much you have learned*. A read can be
replayed from the ledger with no trace left outside it; a write cannot be
taken back, and a message sent cannot be unsent.

This makes the ordering slightly unusual: a program that only reads the
network (5) is deeper than one that writes the disk (4), which is arguable.
The specification takes the position that reaching outside the machine is a
larger commitment than modifying the machine you are already on. An
implementation MUST use the ordering in §1.1 regardless of its opinion, since
depth comparisons must mean the same thing everywhere.
