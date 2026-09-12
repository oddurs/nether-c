---
id: 46
title: 'nether-core: the IR'
type: feature
status: buried
milestone: calculus
assignee: Oddur Sigurdsson
depends_on:
- 18
- 19
created: 2026-09-10
updated: 2026-09-12
priority: p0
effort: l
area: crates/nether-core
stratum: '0'
proof: The IR expresses every construct in spec section 04
---

A small typed intermediate representation with depth on every node. Under two
thousand lines for the whole crate is the target — this is where the idea
either turns out to be beautiful or turns out not to be.

## 2026-09-12

Landed: crates/nether-core, 542 lines of trusted core against a 2000-line target. The IR keeps only the forms with their own typing rule in section 02 and folds the rest: && and || and if are one Select, while and for are one Loop that carries its step so continue still runs it, compound assignment is assignment of a binary, typedef is an alias that does not survive. Depth is stated once per expression and nowhere else — a block has no depth of its own, a local has no depth of its own, because a claim stated twice is eventually stated two different ways. No dependencies, the ledger included: an IR is a fact about a source file.

## 2026-09-12

The proof reads the specification back rather than asserting against a copy of it. tests/grammar.rs parses the production names out of section 04 and fails if one is unaccounted for, walks one sample unit with exhaustive matches so a new IR form cannot compile until it is built and named, and re-reads the prelude and capability tables out of section 09 to compare against the tables in prim.rs and depth.rs. Filed 0116, 0117 and 0118 for three holes it found.
