---
id: 206
title: Answer many holes at once
type: feature
status: unmarked
milestone: cortege
depends_on:
- 209
created: 2026-09-13
updated: 2026-09-13
priority: p1
effort: l
area: crates/nether-world
stratum: '5'
proof: A trace with N independent holes answers them concurrently, and the witnesses are recorded in discovery order
---

Exhumation is where the wall clock actually hurts: N network fetches in
sequence is N round trips.

The holes are independent by construction — 6.3 says a hole is formed only once
every argument is a finished value — so they can be asked at once. What must
not move is the order the answers are *recorded* in, because that is in the
trace.

Stratum 6 is different and should be excluded until somebody argues otherwise:
sending is not idempotent and a concurrent send is a different act from a
sequential one.
