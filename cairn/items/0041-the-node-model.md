---
id: 41
title: The node model
type: feature
status: buried
milestone: ledger
depends_on:
- 38
created: 2026-09-10
updated: 2026-09-11
priority: p0
effort: l
area: crates/nether-ledger
stratum: '0'
proof: A hand-written trace for hello.nc round-trips and lamps correctly
---

Values, thunks, holes and provenance edges as one addressable graph. This is
the data structure the entire language is about, so it is specified in section
07 before it is written here.

## 2026-09-11

Done. Six node kinds, and 7.3.1 now specifies their bytes — 7.3 named them and said 'see 7.3'.

Two real problems surfaced while writing it.

6.3 listed 'dependents' as a field of a hole. It cannot be: a node is named by its content, so a hole that listed what was waiting on it would get a new name every time something came to wait, and every reference to the old name would point at a hole that no longer exists. Removed; what is suspended on a hole is derived by reading the graph backwards, which is what 7.4 already says provenance is.

7.5 said the store maps cairns to nodes, but values have cairns too — a Deposit names the value it deposited, not a Literal wrapping it. The store holds both; Stored is that union, and 7.5 now says so.

A span names its source by cairn rather than by path, because a path is a fact about one machine at one moment.

Node::references is the forward edge, with a test that every cairn a node claims actually appears in its encoding — if that drifts, the reverse index is incomplete and provenance stops early without saying so.
