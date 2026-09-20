---
id: 276
title: nether-world answers an authority question with the depth order
type: bug
status: unmarked
created: 2026-09-19
updated: 2026-09-19
priority: p2
effort: s
area: crates/nether-world
proof: '`World::holds` answers `d ∈ δ` against the set of granted capabilities, and no test asserts that granting one stratum confers another'
---

## What happens

## What should happen

## Reproduction

1.

## Cairn of the offending trace

## Fix boundary and regression proof

Name the smallest affected path, the failing test before the fix, and the
verification after it. Preserve unrelated behavior.

`nether-core` gets this right and says why:

> `δ`: the strata whose capabilities are held. A set, because authority is
> which doors are open and that does not compose by maximum: `disk!` is
> stratum 4 and `net` is 5, so an order fit for comparing two histories would
> make *may fetch a URL* mean *may delete a file*.

`Held` is a bitset, `holds` is `d ∈ δ`, `subset_of` is `dƒ ⊆ δ`, and the
checker's [LOOK] uses `ambient.holds(origin)`. All faithful to §2.1 after 0178.

`nether-world` then does the opposite:

```rust
/// By the lattice and not by the name. §1.1 makes the strata a total order
/// and [DESCEND] raises the ambient depth to the one granted, so a
/// `descend disk!` may `read`: stratum 3 is within stratum 4.
pub fn holds(&self, capability: Capability) -> bool {
    self.depth() >= capability.stratum()
}
```

Both sentences in that comment were repudiated by 0178. §1.1's total order is
about **depth** — where a value has been, composing by maximum — and this is an
**authority** question, which is δ. Collapsing the granted set to its join and
comparing with `>=` is exactly the confusion `Held`'s comment names.

It is not a vulnerability. `World::ask` dispatches by name —
`granted.iter().find(|p| p.answers(&call.function))` — so a world granted
`entropy` cannot answer `get`. `World::holds` is called from no production
path.

It is called from five tests, which assert the repudiated semantics and cite
§1.1 in the message:

```rust
assert!(a.world.holds(Capability::Net), "§1.1 is a total order");   // entropy.rs:60
assert!(a.world.holds(Capability::Entropy), "§1.1 is a total order"); // unrecorded.rs:89
```

A world granted only `entropy` asserting that it holds `net` is the literal
form of "may draw random bytes" implying "may fetch a URL". That is worse than
dead code: anyone fixing `holds` to use `Held` sees five red tests whose
messages say the fix is wrong.

Fix: give `World` a `Held` built from what was granted, make `holds` set
membership, and rewrite the five assertions to say what is actually true —
`disk!` answers `read` because the disk-write provider answers that function,
not because 3 ≤ 4. `World::depth` stays: it is the deepest stratum anything
granted can reach, which is a depth question and a correct one.

Found by auditing each concept in the language against its implementation.
