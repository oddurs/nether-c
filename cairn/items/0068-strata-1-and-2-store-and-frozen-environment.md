---
id: 68
title: 'Strata 1 and 2: store and frozen environment'
type: feature
status: buried
milestone: world
assignee: Oddur Sigurdsson
depends_on:
- 67
- 170
- 171
- 173
created: 2026-09-10
updated: 2026-09-13
priority: p1
effort: m
area: crates/nether-world
stratum: '2'
proof: Two burials with the same declared environment produce the same cairn
---

Reading the ledger, and the pinned environment: declared variables, a frozen
clock, the target triple. Undeclared reads are an error, not an empty string.

## 2026-09-13

Two providers, and the Provider trait had to grow a second failure. §9.9 has exactly two -- a refusal is an answer, a collapse is a bug -- and answer() could only report a ledger that would not write. env on a name that was never declared is the first thing in the language that has to say the other one, so answer() now returns Refuse: NotRecorded or Collapse.

## 2026-09-13

The shaping rule (Prim::refusable, from 0171) is stated once in provider.rs and used by all three providers. It was copied into each of them first, which is the thing the rule about stating a claim once is for.

## 2026-09-13

§8.3.1's flags made exhume's argument loop long enough to need its own type. Asked::parse reads them, Asked::settled refuses the invocations §8.3 and §8.3.1 refuse, and run() is nine lines again.
