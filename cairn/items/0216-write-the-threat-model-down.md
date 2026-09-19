---
id: 216
title: Write the threat model down
type: docs
status: buried
milestone: warden
created: 2026-09-13
updated: 2026-09-18
priority: p0
effort: m
area: SECURITY.md
stratum: '0'
proof: Every trust boundary in the system is named, with what crosses it and what is assumed about the other side
---

SECURITY.md names four things worth attacking and calls the decoder the one
genuinely hostile surface. That was true when the decoder was all there was.

There are now eight crates, a wasm build, a store meant to be shared, and
foreign code at stratum 8. Somebody should say, in one place, who is trusted
and who is not: the author of a program, the author of a trace, the owner of a
store, the machine a burial runs on, and whoever wrote the shared object
stratum 8 dlopens.

## Delivery plan — 2026-09-15

### Starting point and scope

SECURITY.md still says nothing is implemented. Eight crates, WASM workers, shared stores and foreign code now exist; this is the first recommended security task.

### Steps

1. Inventory source/trace authors, providers, store writers, browser transport, build inputs and foreign libraries.
2. For each boundary, name crossing data, verification, limits, trusted components and excluded guarantees; link tests and gaps.
3. Correct stale implementation language while preserving the reporting channel and avoiding unsupported release promises.

### Acceptance and evidence

- [x] Every real boundary has a threat and defense or linked gap. Distinguish integrity from availability and recorded replay from trusting foreign code.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

## Evidence — 2026-09-18

Written against `c0f4128`. Seven boundaries, each with what crosses it, what is
checked, what is assumed and what is not promised. Every path cited in the
table was verified to exist; every claim about a check was read out of the
source rather than recalled.

`scripts/task check` green: 260 items, 0 warnings.

**Integrity and availability are separated throughout.** A shared store defends
the first and not the second — anyone who may write to it may fill it — and
that is said rather than left to be discovered.

**Recorded replay and trusting foreign code are separated.** Boundary 3 assumes
the machine and records everything it says, so replay is exact; boundary 4
assumes nothing, promises nothing, and marks the trace forever.

### The five gaps, named rather than fixed

- `nether-foreign` allocates `*out_len` bytes uncapped on return code `1`.
  `net` and `entropy` both bound their answers; this one does not.
- `nether-wasm` frees with `capacity == len`, which holds today and is not
  guaranteed.
- `nether-wasm` builds a slice from a caller-supplied length.
- `unrecorded` writes the §9.8.1 pre-call witness as `Refusal::Unreachable`, so
  a call that then succeeds leaves a permanent false refusal in an append-only
  ledger.
- Concurrent writers to one store are unsettled — 0207.

## 2026-09-18

SECURITY.md opened with 'Nothing is implemented and nothing is released' and claimed the workspace forbids unsafe. Both were true when written and neither survived: v0.2.0 is out, there are eight crates, and unsafe is allowed in two of them.

## 2026-09-18

The four gaps in the Known section were verified in the source at c0f4128 rather than recalled from the reviews that found them. A threat model that lists only defences is the security version of a design document that lists only advantages.
