---
id: 163
title: fuel_spent is the one thing stopping replay
type: spec
status: unmarked
milestone: rites
created: 2026-09-12
updated: 2026-09-12
priority: p0
effort: m
area: spec/07-ledger.md
stratum: '0'
proof: Replaying a sealed trace produces a trace with the same cairn
---

## Measured

`nether exhume` now answers a hole and seals a trace. Replaying that sealed
trace produces a trace identical to it in every field but one:

```
11356301  trace  depth 3  residue 9ed210cd  0 hole(s)  1 witness(es)  0 deposit(s)  13 steps
32f3a812  trace  depth 3  residue 9ed210cd  0 hole(s)  1 witness(es)  0 deposit(s)   1 steps
```

Same residue, same witnesses, same holes, same deposits, same depth. Thirteen
steps against one.

## Why it cannot be fixed in the rite

`fuel_spent` is the cost of the burial that *produced* a trace. Replaying means
burying that trace's residue again, and the residue is what burial already
finished with — so the second burial is a different and much cheaper one. There
is no way to spend thirteen steps folding a program that has already been
folded.

Nor can the rite re-bury the *predecessor's* residue instead: a trace names its
residue-after and nothing names its residue-before.

## Why not to add the edge

Giving `Trace` a `from` would let replay re-run the original burial. It would
also make a trace's cairn depend on its whole ancestry, and then
[§6.5](../spec/06-evaluation.md)'s staging law is false: `bury(bury(p, A), B)`
and `bury(p, A ∪ B)` reach the same program by different routes and would get
different names for it.

## What to do

`fuel_spent` leaves the trace. §8.2 already makes the argument for the budget —
"It is **not** recorded in the trace, because it cannot have affected one" — and
it applies to the spending as much as to the room: how many steps a burial took
is a fact about that burial, not about the program it produced. A rite still
reports it, because that is where it is a fact.

That leaves a sealed trace a fixed point: burying it again gives it back. Which
is what [§6.7](../spec/06-evaluation.md) says replay is.

The domain goes v3 to v4.

## Acceptance criteria

- [ ] Replaying a sealed trace produces a trace with the same cairn
- [ ] §7.3 and §8.2 agree about what a trace records
- [ ] `bury` and `exhume` still report what they spent
- [ ] §7.2's domain is bumped
