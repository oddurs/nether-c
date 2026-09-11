---
id: 45
title: Fuzz the decoder
type: chore
status: buried
milestone: ledger
depends_on:
- 40
created: 2026-09-10
updated: 2026-09-11
priority: p1
effort: m
area: crates/nether-ledger
stratum: '0'
proof: A seeded fuzzer runs on every commit and finds no panic and no non-canonical acceptance; scripts/task fuzz soaks it for as long as you leave it
---

cargo-fuzz over the decoder. It parses untrusted bytes from a shared store,
so it is the one place in the project with a genuine attack surface.

## 2026-09-11

Done, and it found a real bug on its first run.

`cairns()` guarded a count with saturating_mul and then built the error message with plain `count * 32`. A count near usize::MAX passed the guard and panicked constructing the complaint about it — a panic on exactly the hostile path the guard exists to close, in the one place SECURITY.md calls an attack surface. Fixed, and the input is pinned as a unit test, because a fuzzer finding it again is luck rather than coverage.

320,000 inputs across 8 seeds on every commit; 20,000,000 in about two seconds via scripts/task fuzz, 2,526,070 of them decoding, all canonical, no panics.

Proof restated. The original — twenty-four hours of fuzzing — is not something a commit can demonstrate, and is now its own item under After the Burial. What this item owes is a harness that runs continuously and a law it enforces: if decode succeeds, re-encoding reproduces the input exactly.
