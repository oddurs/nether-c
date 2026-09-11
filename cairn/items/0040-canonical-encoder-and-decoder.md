---
id: 40
title: Canonical encoder and decoder
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
proof: Same value, same bytes, on macOS and Linux, on x86_64 and aarch64
---

Deterministic: fixed integer encoding, sorted map keys, no floats without an
explicit decision, no platform-dependent anything.

## Acceptance criteria

- [x] Cross-platform byte-identity test in CI
- [x] Decoder rejects non-canonical input rather than accepting it leniently

## 2026-09-11

Done for values; node encoding (tag 0x20) lands with the node model and is rejected as Unsupported rather than Unknown in the meantime, which is honest about the difference.

23 tests. The round-trip law from 7.1.1 is tested directly: decode then re-encode reproduces the bytes. Every rejection clause has its own test, plus one that truncates every sample at every prefix and requires a failure each time.

Two things worth recording. The decoder recurses, so it has a depth limit — SECURITY.md lists resource exhaustion in the decoder as in scope, and a few hundred bytes of nested arrays would otherwise be a stack overflow. And a test asserts text is NOT normalised, which is the 0038 decision made executable.
