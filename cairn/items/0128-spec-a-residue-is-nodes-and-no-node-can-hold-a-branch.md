---
id: 128
title: 'Spec: a residue is nodes, and no node can hold a branch'
type: spec
status: buried
milestone: rites
depends_on:
- 21
created: 2026-09-12
updated: 2026-09-12
priority: p0
effort: l
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

- [x] §6.5 and §7.3 agree about what a residue is made of
- [x] `depends` has a value some burial actually produces, or it goes
- [x] The staging law is stated over something that has an encoding
- [x] `spec/90-rationale.md` records the rejected shape

## 2026-09-12

Worked through while trying to build nether bury, which is blocked on it, and so are exhume, strata and graft. Three readings, and the third is new since the item was filed.

## 2026-09-12

One: section 7.3 gains node kinds for a residual program — a branch, a loop, a binding, an unreduced application. That is the reading section 6.6's own summary assumes, because 903 nodes for a program with one hole is the residue being counted. It changes the frozen table in section 7.3.1, which by section 7.2 changes the domain separator and therefore every cairn that has ever existed. Nothing has ever been buried, so the cost today is zero and the cost later is total.

## 2026-09-12

Two: section 6.5 stops calling the residue nodes. The trace records what happened and the residue is something else. That was the item's original recommendation and it leaves the Trace node with no way to name what is left.

## 2026-09-12

Three, and this is the one the work since has made available: the residue is source. A residue is a program, a program is text, and 0056 built a printer that writes the IR back out as Nether C — with a round-trip proof that lowering the printed source gives the same IR node for node. So a trace can name its residue as an ordinary Bytes value, burying again means lexing and parsing and lowering that source again, and section 6.5's staging law holds because the residue really is a complete program. Nothing in the frozen encoding changes.

## 2026-09-12

What three still needs is a Trace node that can hold it. Trace is roots, fuel_spent, depth, unrecorded — and a burial has a residue, a set of holes, a set of deposits and the source it came from, which is four kinds of thing and one ordered list to put them in. Whichever reading wins, Node::Trace's payload has to change, and that is a frozen-section change with a domain bump behind it. That is why this is filed rather than settled.

## 2026-09-12

Watch out when wiring the residue to the ledger: nether-bury's Residue::depth is the deepest stratum anything in the residue still reaches -- what is owed -- and Node::Trace's depth is the deepest stratum a witness reached. A burial that has answered nothing has a residue at 5 and a trace at 0. See section 7.3.2, added by 0136.

## 2026-09-12

SETTLED on reading three: a residue is a program, a program is source, and source is Bytes. Nothing in the frozen value encoding changes for it.

6.5 now says so, and adds the obligation that makes the staging law provable: an implementation MUST be able to print any residue it can produce, and lowering what it printed MUST give back the same program. crates/nether-bury/tests/residue_is_source.rs holds it — bury, print, parse, lower, bury again, and the two printed programs must be identical. Compared as source rather than as IR, because two lowerings of two different texts carry different spans and a span is a fact about where something was written.

Residue::as_unit(&buried) is the new surface. Declarations come from the unit that was buried: burial reduces demands and never touches a struct, a function or a global, so carrying them across loses nothing and an unreduced call still has something to name.

depends is GONE. The item argued it was provably always empty; nether-bury was already passing Vec::new() with a comment saying so, which is about as direct as evidence gets.

Node::Trace changed shape: residue, holes, deposits, source, fuel_spent, depth, unrecorded. That is changing what an existing tag means, which is the bumping case under the rule 0117 just established, so the domain went to v2. Nothing had been buried, so it cost nothing — and that is exactly why the rule was worth having before it was not.

Effort was m and it was l: the change reached five crates, because Trace and Hole are matched exhaustively in lamp, strata and bury. Every one of those was the compiler pointing at something that genuinely had to change.
