---
id: 105
title: A conformance suite for a second implementation
type: chore
status: unmarked
milestone: after
depends_on:
- 40
created: 2026-09-11
updated: 2026-09-15
priority: p1
effort: xl
area: tests/conformance/
stratum: '0'
proof: An implementation written by somebody who did not read our source passes it, and its cairns match ours byte for byte
---

0040's proof is 'the same value, the same bytes, on macOS and Linux'. The
stronger version is the same bytes from an implementation that shares no code
with ours, and until that exists the canonical encoding in section 07 is a
description of what our code happens to do.

The suite is the specification made executable: every normative MUST with a
case that fails when it is violated. Building it will find places where the
specification is a description rather than a requirement, which is most of the
value.

## Delivery plan — 2026-09-15

### Starting point and scope

Existing codec, transcript and calculus tests are inputs to a portable suite, not proof of independent conformance.

### Steps

1. Map normative requirements to implementation-neutral vectors for canonical bytes, cairns, refusals, depth, residue and replay.
2. Specify a small runner interface and include negative/non-canonical cases with expected diagnostics or categories.
3. Publish vectors separately from Rust internals; invite an independent implementation and record its version and results.

### Acceptance and evidence

- [ ] An independently written implementation passes and matches canonical bytes/cairns. The suite can land first, but completion waits for that implementation; do not label Rust-versus-Rust checks independent.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
