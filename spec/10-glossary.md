---
section: "10"
title: Glossary
status: draft
---

# Glossary

Every term used normatively elsewhere, defined once.

**Ambient depth** (δ) — the deepest stratum whose capability is currently
held. 0 at the top level; raised only by `descend`.
[§2.1](02-calculus.md#21-judgement-form)

**Bury** — to evaluate a program as far as the granted capabilities allow,
producing a trace. The only form of evaluation in the language.
[§6.1](06-evaluation.md#61-burial)

**Cairn** — the content address of a value: `blake3` over its canonical
encoding, under a versioned domain separator. Always at depth 0.
[§7.2](07-ledger.md#72-cairns)

**Capability** — a named grant of access to one stratum of the world.
Acquired by `descend`, lexically scoped, never ambient.
[§9.1](09-prelude.md#91-capabilities)

**Demand** — a top-level statement naming an expression that must be
evaluated. Nothing else is evaluated. There is no `main`.
[§6.2](06-evaluation.md#62-demand)

**Deposit** — a value left in the trace by a bare-expression statement. Not
printed; read afterwards with `lamp`.
[§4.7](04-grammar.md#47-the-bare-expression-statement)

**Depth** (d) — how far into the world a value's history reaches, 0 to 8.
Part of the value's type; composes by maximum; never decreases.
[§1.1](01-strata.md#11-the-lattice)

**Descend** — the expression that acquires a capability for the extent of a
block. One-way within the block; only the tail value leaves.
[§1.3](01-strata.md#13-descent)

**Exhume** — to grant a capability, answer a trace's holes, record the
answers, and re-bury the residue, producing a new trace.
[§6.6](06-evaluation.md#66-exhumation)

**Fuel** — the deterministic bound on evaluation steps a burial may take.
Exhausting it is a diagnostic naming a span.
[§6.4](06-evaluation.md#64-starvation-and-fuel)

**Graft** — to substitute a subtrace by cairn and re-bury only what changed.
[§8.7](08-rites.md#87-graft)

**Hole** — a suspended world-question: the call, its stratum, its span, and
the nodes waiting on it. Identical calls are one hole.
[§6.3](06-evaluation.md#63-holes)

**Lamp** — the rite that renders a value or walks its provenance. A tool the
operator carries; never a capability the program holds.
[§8.4](08-rites.md#84-lamp)

**Latent depth** — the depth a function reaches when applied, carried in its
arrow type and written as a trailing `@n` on its signature.
[§2.1](02-calculus.md#21-judgement-form)

**Ledger** — the content-addressed store of nodes. Immutable, append-only, not
garbage-collected. [§7](07-ledger.md)

**Look** — to open a shade. Well-typed only where the ambient depth is at
least the shade's origin depth: the Orpheus rule.
[§1.6](01-strata.md#16-shade-and-the-orpheus-rule)

**Node** — the unit the ledger stores, addressed by its cairn: a literal, an
application, a hole, a deposit, a witness, or a trace.
[§7.3](07-ledger.md#73-nodes)

**Opaque** — a barrier burial will not evaluate through, even where it could.
A typing rule, not a flag. [§6.4](06-evaluation.md#64-starvation-and-fuel)

**Orpheus rule** — you may carry a shade up out of any stratum; you may only
look at it by going back down.
[§1.6](01-strata.md#16-shade-and-the-orpheus-rule)

**Provenance** — the backwards reading of the apply and witness nodes: what
produced this value, and what produced that. [§7.4](07-ledger.md#74-provenance)

**Replay** — re-burying a trace with no capabilities at all, serving every
answer from the ledger. Produces the same cairn, or reports the first node
that differs. [§6.7](06-evaluation.md#67-replay)

**Residue** — what survives a burial: the unreduced nodes, the holes, and the
edges between them. Itself a complete program.
[§6.5](06-evaluation.md#65-residue)

**Rite** — one of the six commands: `bury`, `exhume`, `lamp`, `cairn`,
`strata`, `graft`. [§8](08-rites.md)

**Seal** — to take the cairn of a value. Always yields depth 0, because a name
is pure regardless of what it names. [§1.5](01-strata.md#15-seal)

**Sealed trace** — a trace with no holes. It has a value.
[§6.6](06-evaluation.md#66-exhumation)

**Shade** — a value carried up out of a deeper stratum, opaque: storable,
sealable, comparable, not observable.
[§1.6](01-strata.md#16-shade-and-the-orpheus-rule)

**Starve** — to be unevaluable because of a transitive dependency on a hole,
or because a prelude function could not answer.
[§6.4](06-evaluation.md#64-starvation-and-fuel)

**Stratum** — one of the nine depths, each with a grant, a cost and a witness
obligation. [§1.1](01-strata.md#11-the-lattice)

**Trace** — the immutable, content-addressed record a burial produces.
[§6.1](06-evaluation.md#61-burial)

**Unrecorded** — stratum 8: foreign code whose behaviour cannot be witnessed.
Marks a trace permanently. [§1.7](01-strata.md#17-stratum-8-the-unrecorded)

**Witness** — the record an implementation must write before returning a
world-derived value to the program.
[§1.4](01-strata.md#14-what-the-trace-records)
