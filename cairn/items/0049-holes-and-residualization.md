---
id: 49
title: Holes and residualization
type: feature
status: buried
milestone: calculus
assignee: Oddur Sigurdsson
depends_on:
- 41
- 48
created: 2026-09-10
updated: 2026-09-12
priority: p0
effort: l
area: crates/nether-core
stratum: '0'
proof: A program with one read produces exactly one hole, carrying its full dependency graph
---

What survives burial, and in what form. A hole carries its call, its stratum,
its source span and everything that depends on it.

## 2026-09-12

Burial residualises seal rather than folding it, because naming a value means encoding it and hashing it and nether-core does not depend on the ledger. Wiring the two together is this item: when a hole gets a Node it also gets a cairn, and seal of a known value becomes a literal Cairn at the same moment.

## 2026-09-12

Burial moved to a crate of its own. nether-core is the IR and the calculus and has no dependencies, which is a property worth keeping: an IR is a fact about a source file. Burial is where that fact meets the ledger, so it is where the two crates join, and nether-bury depends on both. The Cairn literal stayed in nether-core as thirty-two bytes, because representing a name needs no ledger even though computing one does.

## 2026-09-12

Values are named as values, not wrapped in Literal nodes. A hole's arguments and the result of seal are then the same cairn for the same value, which is the entire point of addressing by content, and fetch_node resolves either. The store already holds Stored = Value | Node, so this needed nothing new.

## 2026-09-12

An abandoned loop unrolling keeps its questions. The loop residualises whole, so burying the residue runs it again from the same restored environment and asks the same things; dropping them would hide from nether bury exactly the capability somebody has to grant. The first draft truncated them and was wrong.

## 2026-09-12

Filed 0127 and 0128. Section 5.3 defines equality twice and the two answers differ for a shade, and section 1.5 sides with the second; and section 6.5 says a residue is nodes while section 7.3 has no node that can hold a branch or a loop, which every residue with a waiting condition contains. The depends field of a hole is provably always empty and is folded into 0128.
