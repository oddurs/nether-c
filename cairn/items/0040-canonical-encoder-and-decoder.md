---
id: 40
title: Canonical encoder and decoder
type: feature
status: unmarked
milestone: ledger
depends_on:
- 38
created: 2026-09-10
updated: 2026-09-10
priority: p0
effort: l
area: crates/nether-ledger
stratum: '0'
proof: Same value, same bytes, on macOS and Linux, on x86_64 and aarch64
---

Deterministic: fixed integer encoding, sorted map keys, no floats without an
explicit decision, no platform-dependent anything.

## Acceptance criteria

- [ ] Cross-platform byte-identity test in CI
- [ ] Decoder rejects non-canonical input rather than accepting it leniently
