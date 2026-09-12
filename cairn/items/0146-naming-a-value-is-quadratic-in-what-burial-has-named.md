---
id: 146
title: Naming a value is quadratic in what burial has named
type: chore
status: buried
milestone: rites
assignee: Oddur Sigurdsson
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

- [x] `remember` is constant time in what has already been named
- [x] The order of `named` and `holes` is unchanged

## 2026-09-12

A HashSet beside the Vec: the vector keeps the order section 6.2 fixes and the set answers whether a name is already there. Residue::get stays a scan and says so -- named is iterated by whoever writes it, and nothing looks one up in a loop.

## 2026-09-12

Proved as a shape rather than a duration: four times the questions is four times the work and the scan made it sixteen. With the scan back the test fails at 13.6x.
