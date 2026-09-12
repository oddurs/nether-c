---
id: 136
title: 'Spec: a trace that under-reports its own depth is not called malformed'
type: spec
status: buried
milestone: rites
assignee: Oddur Sigurdsson
created: 2026-09-12
updated: 2026-09-12
priority: p2
effort: s
area: spec/07-ledger.md
proof: Section 7.3's list of what makes a node malformed either covers a trace whose depth disagrees with its witnesses, or says why it cannot
---

## What is missing

§7.3 lists what makes a node malformed: a stratum above 8, a `call` with an
empty name, a `span` whose end is before its start. Every one of those is
decidable from the node alone.

A `Trace` also records a `depth` — "the deepest stratum reached" — and nothing
says what it means for that number to disagree with the witnesses under it. A
trace claiming depth 3 over a witness at 5 has written down a falsehood about
the one thing a trace is for, and the ledger accepts it.

## Why it was not simply added

The check is not local. `put` sees one node; the disagreement is only visible
after walking a graph that may not be wholly present, and §7.3's other
conditions are all things `put` can refuse on the spot. Making this one of
them would mean the ledger validates reachability, which is a different and
much larger promise.

So it is probably a rule about *writing* a trace — §6.5 — rather than about
what a node may hold. `nether strata` already reports it, and exits 65 for it,
because it is the one rite that has the whole graph in hand.

## Where the claim went

§7.3.2, "What a decoder cannot check", beside the list of what a decoder must.
Two claims live there: a `Trace`'s `depth` is the greatest stratum of any
`Witness` reachable from its roots, and its stratum-8 mark is set exactly when
one of those witnesses is at 8. Neither is checkable on `put`, and the section
says so rather than leaving an implementer to discover it.

Not §6.5, which is about producing a residue and is still moving under 0128.
Not §8.6, because a rite is where a claim is *caught* and not where it is made.

## What it turned up

`nether-bury`'s `Residue::depth` and `Node::Trace`'s `depth` are two different
quantities with the same name. The residue's is "the deepest stratum anything
in the residue still reaches" — what is owed. The trace's is what was reached.
A burial that has answered nothing has a residue reaching stratum 5 and a trace
at depth 0, and nothing yet maps one to the other, so the trap is still
unsprung. Both are now documented against each other, and 0128 is told.

## Acceptance criteria

- [x] §7.3 says what a `Trace`'s `depth` means, once
- [x] It says which of its claims a decoder cannot check, and why
- [x] §8.6 says that holes are reported apart from witnesses

## 2026-09-12

Found while building nether strata (0064). The rite exits 65 on it today, which is a decision the specification has not made.
