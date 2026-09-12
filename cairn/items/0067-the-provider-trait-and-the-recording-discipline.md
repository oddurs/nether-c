---
id: 67
title: The provider trait and the recording discipline
type: feature
status: buried
milestone: world
assignee: Oddur Sigurdsson
depends_on:
- 12
created: 2026-09-10
updated: 2026-09-12
priority: p0
effort: l
area: crates/nether-world
stratum: '0'
proof: No provider can return a value it has not first written to the ledger; enforced by the type, not by review
---

One trait, one rule: an answer is written to the ledger before it is
returned. Make the recording structural rather than a convention somebody has
to remember, because somebody will not.

## 2026-09-12

The discipline is the signature. A provider answers by returning a Recorded, the only thing that makes one is Recorder::record which writes the answer and then the witness before it returns, and Recorded has private fields and no constructor. A provider that forgot to record has nothing to return and does not compile.

## 2026-09-12

Proved by the compiler rather than by a test that could drift: Recorded carries a compile_fail doctest that forges one. Making the fields public makes that doctest fail with 'Test compiled successfully, but it is marked compile_fail', so the proof bites.

## 2026-09-12

World holds what was granted and refuses everything else, which is what makes no grant mean nothing rather than everything. A refusal is an ordinary Recorded, per section 9.9; the error type is for a ledger that would not take the answer, which is this machine failing and not the world saying no.
