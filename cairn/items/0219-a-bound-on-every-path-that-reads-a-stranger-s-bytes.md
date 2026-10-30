---
id: 219
title: A bound on every path that reads a stranger's bytes
type: chore
status: buried
milestone: warden
assignee: Oddur Sigurdsson
depends_on:
- 216
created: 2026-09-13
updated: 2026-10-30
priority: p1
effort: l
area: crates/
stratum: '0'
proof: Every path that reads a stranger's bytes allocates at most a stated multiple of its input, at its peak and in total, asserted on adversarial input by crates/nether-wasm/tests/bound.rs
---

Two allocation bugs have been found by hand: a cairn count that reserved 396 MB
from 8 MB of input, and an integer overflow in the error path of the guard that
was supposed to stop it.

Both were in the decoder because that is where somebody looked. The rule should
hold everywhere and be checked rather than reasoned about.

## Delivery plan — 2026-09-15

### Starting point and scope

The sub-1 MiB input / 10 MiB allocation claim is a proposed universal bound, not an established property.

### Steps

1. Inventory all untrusted entrypoints with 0216, including aggregate expansion and native/WASM paths.
2. Define peak-live versus cumulative allocation, baseline exclusions, decoded expansion and recursion/time limits.
3. Instrument each path with adversarial inputs; propose an explicit proof/spec amendment if the target is impossible.

### Acceptance and evidence

- [x] Every inventoried path meets the stated unambiguous bound. Do not silently narrow coverage to decoder buffers or ignore repeated low-live-memory allocation.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

## 2026-10-30

Amended the proof: ten times is impossible for any tree (a ';' is a token and a statement; a 0x00 in an array is a 48-byte value); the rejected bound is in §90.2. Inventory: every boundary in SECURITY.md reaches the reader through the front end (bury, graft, exhume reading a residue) or the codec (store, browser); store reads are a file read, a hash and the codec. Measured at 256 KiB and 1 KiB: front end worst 183x peak, 362x total (parser, one-line functions and calls); codec 72x peak, 96x total; store get 97x/145x. Bounds: source 256/384, objects 128/192, plus 64 KiB fixed. Found and fixed: nested arrays each reserving a count believed against the same remaining bytes, 2000x on 1 KiB and 4797x on 1 MiB; the test fails on the old decoder. Burial's own space is out of scope: it is the program's to spend under fuel and the §6.4 limits.
