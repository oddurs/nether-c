---
id: 287
uid: 9d922ffe-212c-4bf7-a9d0-73e4cc21b367
title: A residue can lower a shade's origin out of what look holds
type: spec
status: buried
milestone: assay
assignee: Oddur Sigurdsson
created: 2026-11-08
updated: 2026-11-08
closed_at: 2026-11-08
priority: p0
effort: m
stratum: '0'
area: spec/02-calculus.md
proof: Burial of a program the checker accepts never leaves a residue with an Orpheus fault, and the monotone soak passes ten million programs with no class of fault skipped
---

## What this section must answer

What burial does to a `shade` whose operand reduces to something shallower,
when a `look` of that shade sits under a descent that holds the old origin but
not the new one.

## What happens

The monotone soak (`scripts/task fuzz`) fails at seed `0x78dde6e5f03ff054`,
round 10265:

```
descend net { look shade (if (<false>) { <net-deep> } else { descend disk { must(read("x")) } }) }
```

The branch is `max(5, 3) = 5` deep, so the shade's origin is 5, and `look`
under `descend net` holds {0, 5}: the program checks. The condition is
decided, burial takes the `disk` arm, and the residue is
`look shade descend disk { ... }` with origin 3. [LOOK] asks for membership,
3 is not in {0, 5}, and the residue is refused with an Orpheus fault.

Burial may lower a depth (`soundness_of_bound`, §90.7). [LOOK] is not monotone
in the depth it asks about when δ is not downward-closed
(`numeric_vs_membership_counterexample`, the same d = 3, δ = {0, 5}). The two
theorems together say a residue need not type. Nothing proves subject
reduction for [LOOK].

#239 (0281) exposed it: the commit before it passes the round, and #239 fails
it. It fails the same way on `main`. The soak is not in `scripts/task test`, so
CI did not see it.

## Constraints it inherits

- §90.2 rejected the numeric ceiling for [LOOK]; the proof in
  `proof/orpheus.py` is why.
- §6.5: a residue is a program, and burying it again leaves it.

## Answer

[§6.5, *A residue keeps its depths*](../../spec/06-evaluation.md): a residual
that is not a value MUST NOT be shallower than the code it replaced. A decided
branch whose untaken arm is deeper, or writes a local more deeply, stays whole;
a starved call whose reduced body returns less deeply stays the call. The
rejected answers are in §90.2, *A residue that lowers a depth*. The
implementation is a separate item.

## Candidate answers

- Burial does not reduce under `shade` past what changes the origin: the
  select stays as written.
- A residue can state a shade's origin, so reducing the operand keeps it.
- [LOOK] changes. This reopens §90.2.

Record the rejected ones in `spec/90-rationale.md`.

## 2026-11-08

Keeping only a branch under shade misses two paths, both checked with the binary on main: a specialised call (net_or_disk(false) becomes a function returning from 3) and a local whose net-deep arm was dropped (the example now in 6.5). Both residues are refused with 'cannot look at a shade from stratum 3; 5 is held here'. So the rule is stated about depths.
