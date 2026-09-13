---
id: 72
title: 'Stratum 8: the Unrecorded'
type: feature
status: buried
milestone: world
assignee: Oddur Sigurdsson
depends_on:
- 16
- 67
- 175
created: 2026-09-10
updated: 2026-09-13
priority: p2
effort: l
area: crates/nether-world
stratum: '8'
proof: A trace that touched stratum 8 refuses to claim reproducibility, in its own output
---

Foreign code. Depends on the codex decision about whether this stratum exists
at all. If it does, the quarantine has to be loud: anything downstream of an
unrecorded call is marked, forever, in the artifact.

## 2026-09-13

The unsafe is a crate, nether-foreign, not a relaxed lint. The workspace forbids unsafe_code and forbid exists so it cannot be locally undone; downgrading it to deny for one module downgrades it for every module. Everything unsafe in the language is now in one directory and the claim is checkable by reading it.

## 2026-09-13

The witness goes down before the call, which is the reverse of every other provider. §9.8.1: a callee that does not return still leaves a trace saying what was attempted, and at stratum 8 that is the only guarantee available.

## 2026-09-13

§1.7 says every rite that reports replayability must refuse to report a marked trace as replayable. There are two -- strata's replayable: no, and exhume --replay's identical. -- and only the first was doing it.

## 2026-09-13

The proof calls real foreign code. tests/foreign/said.c is written against §9.8.1 and covers every branch it has: an answer that fits, one that never fits, and a refusal. A mocked stratum 8 proves nothing about the one thing this stratum is.
