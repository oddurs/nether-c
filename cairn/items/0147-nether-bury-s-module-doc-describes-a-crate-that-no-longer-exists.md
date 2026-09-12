---
id: 147
title: nether-bury's module doc describes a crate that no longer exists
type: docs
status: unmarked
milestone: rites
created: 2026-09-12
updated: 2026-09-12
priority: p2
effort: s
area: crates/nether-bury
stratum: '0'
proof: Every claim in the module doc is true of the crate under it
---

## What it says

`crates/nether-bury/src/bury.rs` opens with three things "deliberately not here
yet":

> - **Holes.** A world-question residualises as the expression that asked it
>   rather than as a `Hole` node.
> - **`seal`.** Naming a value means encoding it and hashing it, which is the
>   ledger, and this crate does not depend on the ledger. It residualises.
> - **Aggregates.**

The first two are here. `dig` builds `Node::Hole` and pushes it onto
`Residue::holes`; `Rite::Seal` folds to a `Literal::Cairn`; and `Cargo.toml`
depends on `nether-ledger` with a comment — "naming is the ledger" — that
contradicts the module doc four lines further down.

Only aggregates is still true, and it has an item: 0116.

## Why nothing caught it

`tests/transcripts/run` proves every sample in the *specification*. Nothing
proves a doc comment, and there is no cheap way to. This one is caught by
reading.

## Acceptance criteria

- [ ] Every claim in the module doc is true of the crate under it
