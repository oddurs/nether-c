---
id: 128
title: 'Spec: a residue is nodes, and no node can hold a branch'
type: spec
status: unmarked
milestone: rites
depends_on:
- 21
created: 2026-09-12
updated: 2026-09-12
priority: p0
effort: m
area: spec/07-ledger.md
proof: Every residue burial can produce has an encoding in section 07, and burying that encoding again gives the same result
---

## The hole

`spec/06-evaluation.md` §6.5:

> What survives burial is the **residue**: every node that could not be
> reduced, plus the holes, plus the provenance edges between them.
>
> The residue is a complete program. It can be buried again …

`spec/07-ledger.md` §7.3 lists every node there is: `Literal`, `Apply`,
`Hole`, `Deposit`, `Witness`, `Trace`. None of them holds a branch, a loop, a
binding or a block, and a residue routinely contains all four — a `while`
whose condition waits on a hole residualises whole, and so does the `if`
around it.

So either a residue is not nodes, or §7.3 is missing most of them. Both
sections are load-bearing: §6.5's staging law is stated over residues, and
§7.3 is frozen.

Found while building holes: burial produces the holes as nodes and the
residual program as an IR, and nothing in the specification says how the
second becomes a trace.

## The smaller question inside it

A `Hole`'s `depends` — "the cairns of the nodes that must exist for this call
to be made" — is provably always empty as things stand. A hole is only formed
once every argument is a finished value ([§6.3](../spec/06-evaluation.md)), so
nothing it needs can still be waiting on another hole. If the field is meant
to hold the argument values it duplicates `call.args`; if it is meant for
something else, nothing in a burial produces it.

## What this must decide

How a residue is written down. Either §7.3 gains whatever node kinds a
residual program needs and the frozen table changes with everything that
implies, or §6.5 stops calling the residue nodes and says what it is instead —
a source-shaped thing that a trace *refers* to, with the holes as the nodes.

The second is smaller and probably right: the trace records what happened,
and the residue is a program, and those have never needed the same encoding.

## Acceptance criteria

- [ ] §6.5 and §7.3 agree about what a residue is made of
- [ ] `depends` has a value some burial actually produces, or it goes
- [ ] The staging law is stated over something that has an encoding
- [ ] `spec/90-rationale.md` records the rejected shape
