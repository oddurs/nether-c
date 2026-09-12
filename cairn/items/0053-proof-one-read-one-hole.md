---
id: 53
title: 'Proof: one read, one hole'
type: chore
status: buried
milestone: calculus
assignee: Oddur Sigurdsson
depends_on:
- 49
created: 2026-09-10
updated: 2026-09-12
priority: p0
effort: s
area: crates/nether-core
stratum: '3'
proof: The stage 1 proof runs in CI on every push
---

build.nc from the spec buries to depth 3 with exactly one hole and 900-odd
nodes. Pin the shape, not the exact count, so ordinary refactors do not fail it.

## 2026-09-12

The shape is pinned and the counts are not. Depth 3 and one hole are facts about the program; nine hundred nodes is a fact about whatever compile happens to be, and an ordinary refactor should not have to argue with it. The test asserts the summary line section 6.6 prints, minus the cairn and the node count.

## 2026-09-12

Built against the real source text rather than against nothing, so the span the hole carries is the span read("main.nc") actually occupies in build.nc, named against the cairn of build.nc. That is the first end-to-end check that a span survives from an IR offset to a ledger node.

## 2026-09-12

The unused binding divides by zero in one of the tests. It is the only way to prove a thing was not evaluated: the burial finishing at all is the proof, which is what section 6.2 makes a semantic guarantee rather than an optimisation.
