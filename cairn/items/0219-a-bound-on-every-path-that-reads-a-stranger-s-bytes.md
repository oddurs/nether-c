---
id: 219
title: A bound on every path that reads a stranger's bytes
type: chore
status: unmarked
milestone: warden
depends_on:
- 216
created: 2026-09-13
updated: 2026-09-13
priority: p1
effort: l
area: crates/
stratum: '0'
proof: No input under one megabyte causes more than ten megabytes of allocation, on any entry point, asserted
---

Two allocation bugs have been found by hand: a cairn count that reserved 396 MB
from 8 MB of input, and an integer overflow in the error path of the guard that
was supposed to stop it.

Both were in the decoder because that is where somebody looked. The rule should
hold everywhere and be checked rather than reasoned about.
