---
id: 146
title: Naming a value is quadratic in what burial has named
type: chore
status: unmarked
milestone: rites
created: 2026-09-12
updated: 2026-09-12
priority: p2
effort: s
area: crates/nether-bury
stratum: '0'
proof: Naming ten thousand values costs about ten thousand operations, not a hundred million
---

## The scan

`Burial::remember` checks whether it has seen a cairn with

```rust
if !self.named.iter().any(|(c, _)| *c == cairn) {
```

which is linear in everything named so far, so naming N values is O(N²).
`Residue::get` is the same scan, and `Residue::questions()` runs it once per
hole.

§6.6 counts nine hundred and three nodes for a one-hole program. A real burial
names more.

## What to do

Keep the `Vec` — §6.2 and §6.3 both depend on the order things were discovered
in — and put a `HashMap<Cairn, usize>` beside it.

## Acceptance criteria

- [ ] `remember` is constant time in what has already been named
- [ ] The order of `named` and `holes` is unchanged
