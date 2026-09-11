---
id: 43
title: Round-trip one million nodes
type: chore
status: buried
milestone: ledger
depends_on:
- 42
created: 2026-09-10
updated: 2026-09-11
priority: p0
effort: m
area: crates/nether-ledger
stratum: '4'
proof: One million nodes in and out, byte-identical, with the wall time recorded in the PR
---

The stage 0 proof. Not a benchmark to optimise — a demonstration that the
format and store hold up at a size where mistakes become visible.

## 2026-09-11

PROVEN. 1,000,012 nodes built in 0.25s and round-tripped in 0.33s, byte-identical, 98.1 MB encoded, 3.0M nodes/sec (release, M-series). Root 17b566cc. scripts/task proofs runs it; it is #[ignore]d so it does not run on every commit, but check still compiles it so it cannot rot unnoticed.
