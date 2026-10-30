---
id: 275
title: A trace that owes the network calls itself pure
type: spec
status: buried
created: 2026-09-19
updated: 2026-10-30
priority: p0
effort: m
area: spec/07-ledger.md
proof: Burying a program whose only world-question is demanded through a `seal` records a depth at least as deep as its deepest hole, and `nether strata` on it does not print `pure`
---

## What this section must answer

## Constraints it inherits

## Open questions

- [x] Count holes as well as the residue, or make the residue carry what it
      seals? As well as. The other unsays §1.5; §90.2 records it.

## Delivery steps and dependencies

1. State the prerequisite decisions and the smallest reviewable specification change.
2. Name the implementation/conformance items that discharge the contract later.

## Acceptance criteria

- [x] Every normative claim is stated once, in one place
- [x] Every code sample in it is in `tests/transcripts/`
- [x] A reader who has not read the rest of the spec can follow it

§7.3.2 defines a trace's depth as "the join of the depth of its residue and the
greatest stratum of any `Witness` it names". A hole contributes nothing. That is
what `closing::depth` computes, so the implementation is faithful to the
formula — and the formula is wrong.

§1.5 makes `seal e` depth 0 whatever `e` is, and a residue's depth is the join
over what was *demanded*. So a program that reaches the world only through a
seal has a pure demand and an outstanding deep hole:

```console
$ nether bury deep.nc
buried   deep.nc → 80dd9184   depth 0   holes 1   4 nodes
  hole ①  get("https://example.invalid/x") stratum 5  net
$ nether strata 80dd9184
depth 0   pure

  5  get("https://example.invalid/x")  1ab9a540:1:35   pending
  0  everything else

  replayable: yes
```

`deep.nc` is three lines:

```c
Bytes@5 page = must(descend net { get("https://example.invalid/x") });
Cairn   c    = seal page;
demand c;
```

The headline contradicts the line under it, and the word is `pure` — the
strongest claim the language makes. §1.1 gives stratum 0 as "arithmetic, data,
functions / costs nothing". This trace cannot take one step without a network
fetch.

Nothing catches it. §8.6's malformed-trace check is
`told.deepest_answered() > told.depth`, which reads witnesses only; §7.3.2
says the residue half is `bury`'s job, and `bury` computed 0 honestly.

§7.3.2 already states the rule this breaks, as its own worked example:

> A trace whose only stratum-8 call is still a hole has depth 8 and is not
> marked, because nothing has happened off the record yet.

Substitute 5 for 8 and that sentence is false of this implementation. The
stratum-8 *mark* is right and is a separate test on purpose — `unrecorded` is
`reached, not owed`, and `closing::unrecorded` gets it right. It is the depth
that is wrong.

The fix is to join the holes' strata in as well:

```rust
pub fn depth(store: &Store, residue: u8, witnesses: &[Cairn], holes: &[Cairn]) -> u8
```

§7.3.2 argues against counting holes *instead of* the residue, and that
argument is good: an `opaque` barrier hides a deep expression that never
becomes a hole, and only the residue catches it. It is not an argument against
counting them *as well as*. A hole at stratum 5 is a true statement that this
trace cannot proceed without reaching stratum 5, which is what depth is for.

§7.3.2's definition, §8.6's check and all three writers of a trace change
together, and the rejected alternative goes in `spec/90-rationale.md`.

Not a trust boundary. SECURITY.md boundary 2 already promises only that a trace
"says what some machine recorded". This is an honest `bury` on this machine
recording the wrong fact, which is worse in a different way: nothing downstream
has reason to doubt it.

Found by auditing each concept in the language against its implementation.

## 2026-10-30

Buried. §7.3.2 makes a trace's depth the join of its residue, its holes and its
witnesses, and works `owed.nc` — this item's three lines, now
`tests/programs/owed.nc` — as the case that rules out the residue alone.
`closing::depth` takes the holes, and `bury`, `exhume` and `graft` pass them.
§8.6's malformed check reads holes as well as witnesses. `bury`'s headline now
prints the trace's depth rather than the residue's, which is what made it say
`depth 0` over a stratum-5 hole.

Proof, observed: `nether bury owed.nc` prints `depth 5   holes 1`, and
`nether strata` on it prints `depth 5   net`. Both are executed by
`tests/transcripts/run` from §7.3.2, and `a_trace_that_owes_the_network_is_not_pure`
and `a_trace_shallower_than_a_hole_it_owes_is_malformed` hold it in
`crates/nether-cli/tests/rites.rs`.

It also corrected `docs/manual/04-the-grant.md`: exhuming `errand.nc` with no
grant printed `depth 0` over the trace chapter 3 shows at `depth 3`. Same bug,
in `exhume`.

A trace written before this change that owes a hole deeper than its residue is
now reported malformed by `strata`. That is the rule working, not a regression.
