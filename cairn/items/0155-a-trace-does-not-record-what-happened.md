---
id: 155
title: A trace does not record what happened
type: bug
status: unmarked
milestone: rites
created: 2026-09-12
updated: 2026-09-12
priority: p0
effort: l
area: crates/nether-ledger
stratum: '0'
proof: A trace names its witnesses, and section 7.3.2 states one depth rule that both section 8.2's and section 6.6's transcripts satisfy
---

## Three things, one tangle

### Nothing names a `Witness`

[§6.5](../spec/06-evaluation.md) replaced `Trace.roots` with `residue`, `holes`,
`deposits` and `source`. `Trace::nodes()` yields holes and deposits; a `Hole`
names none. Grep the node model: `Witness` appears only as its own variant.
No node names one.

So a witness is written and then unreachable, and three things break.

- **[§7.3.2](../spec/07-ledger.md) is unsatisfiable.** It still says a trace's
  depth is "the greatest stratum of any `Witness` reachable from its
  **roots**", and there are no roots.
- **`strata` can never see a witness.** Its own comment says so, so
  `deepest_answered()` is always 0 and the exit-65 path is dead code.
- **[§6.7](../spec/06-evaluation.md)'s replay law has no forward edge.**
  "Replay re-buries a trace using only the answers already recorded" — with no
  trace-to-witness link, finding them means matching calls across the store.

§6.6's own transcript settles which way to fix it:

```
sealed   4c02ab7f + a1f0c93d -> 77de9b31   depth 3   holes 0
```

An exhumed trace with depth 3 and no holes. Under the current node model
nothing in that trace reaches stratum 3, so the example is unrepresentable.
A trace has to name its witnesses.

### The depth field means two things

§7.3.2 says a trace whose holes are at 5 and whose witnesses are at 0 has depth
0. [§8.2](../spec/08-rites.md)'s transcript says

```
buried   build.nc -> 0380c8ae   depth 3   holes 1   4 nodes
```

which is a stage-one burial with one hole at stratum 3, and depth 3. The two
sections disagree, and `nether bury` follows §8.2 by writing `residue.depth` —
the number `Residue::depth`'s own doc comment warns is not this field, three
lines above the read. 0136 filed that warning against 0128. It was sprung.

The result contradicts itself on screen:

```
depth 3   disk

  3  read("main.nc")           6f4d11b1:10:35   pending
```

One rule satisfies both transcripts: a trace's depth is the deepest stratum
anything in it **reaches or has reached** — the join over its holes and its
witnesses. Stage-one `build.nc`: holes at 3, no witnesses, depth 3. §6.6's
exhumed trace: no holes, a witness at 3, depth 3. The owed-against-reached
distinction stays where it is useful, on the line, as `pending`.

That also dissolves the trap: `Residue::depth` and `Trace::depth` become the
same quantity, and the warning comes out.

### The stratum-8 mark is hardcoded

`nether bury` writes `unrecorded: false` always. [§1.7](../spec/01-strata.md)
requires marking any trace that **reached** stratum 8, and a stage-one burial
reaches nothing, so `false` is right today and right by accident. Derived from
the witnesses it stays right when `exhume` exists.

### What the proofs were doing

Three `strata` tests build their trace by putting a `Witness` in `deposits`,
which §7.3's table says is deposits. They are green over a trace no burial
produces — including the one covering the exit-65 path.

## What this costs

A field on `Trace` is a change to a frozen encoding: §7.3.1 gains it and §7.2's
domain separator goes v2 to v3, which changes every cairn that has ever
existed. That is the stated and correct consequence, and a draft is when to pay
it.

## Acceptance criteria

- [ ] A `Trace` names its witnesses, and `Node::nodes()` reaches them
- [ ] §7.3.2 states one depth rule, and §8.2's and §6.6's transcripts satisfy it
- [ ] The stratum-8 mark is derived from what reached stratum 8
- [ ] `strata`'s malformed check can fire on a trace a burial could produce
- [ ] No fixture puts a `Witness` in `deposits`
- [ ] §7.2's domain is bumped
