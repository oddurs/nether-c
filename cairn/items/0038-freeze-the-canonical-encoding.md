---
id: 38
title: Freeze the canonical encoding
type: chore
status: unmarked
milestone: ledger
depends_on:
- 21
created: 2026-09-10
updated: 2026-09-10
priority: p0
effort: m
area: crates/nether-ledger
stratum: '1'
proof: The encoding document is marked frozen and every later change requires a format version bump
---

Blocked on spec section 07. Nothing else in this descent starts until the
format is written down and frozen — the whole project's reproducibility claim
rests on this one document being right before there is code depending on it.
