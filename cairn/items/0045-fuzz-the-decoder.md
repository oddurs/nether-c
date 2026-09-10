---
id: 45
title: Fuzz the decoder
type: chore
status: unmarked
milestone: ledger
depends_on:
- 40
created: 2026-09-10
updated: 2026-09-10
priority: p1
effort: m
area: crates/nether-ledger
stratum: '0'
proof: Twenty-four hours of fuzzing with no panics and no non-canonical acceptance
---

cargo-fuzz over the decoder. It parses untrusted bytes from a shared store,
so it is the one place in the project with a genuine attack surface.
