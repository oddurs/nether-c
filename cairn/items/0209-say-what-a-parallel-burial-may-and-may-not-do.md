---
id: 209
title: Say what a parallel burial may and may not do
type: spec
status: unmarked
milestone: cortege
created: 2026-09-13
updated: 2026-09-13
priority: p0
effort: m
area: spec/06-evaluation.md
stratum: '0'
proof: Section 06 states which orders are fixed and which are free, and the parallel implementation is checked against it
---

6.2 says evaluation order is unspecified except that it must be deterministic
for a given source and capability set. That sentence was written when there was
one thread.

It has to say more now: which observable orders are part of the trace — hole
discovery, deposits, witnesses — and which are genuinely free. An
implementation that parallelises without that written down is guessing at what
it is allowed to reorder.
