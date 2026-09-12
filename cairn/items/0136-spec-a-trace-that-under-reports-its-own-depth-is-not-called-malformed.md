---
id: 136
title: 'Spec: a trace that under-reports its own depth is not called malformed'
type: spec
status: unmarked
milestone: rites
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

## What this must decide

Where the claim lives: §7.3 with a note that it is not checkable on `put`,
§6.5 as a rule about burial, or §8.6 as a thing only `strata` is in a position
to notice.

## 2026-09-12

Found while building nether strata (0064). The rite exits 65 on it today, which is a decision the specification has not made.
