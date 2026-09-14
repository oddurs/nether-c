---
id: 205
title: One program, one cairn, however many threads
type: chore
status: unmarked
milestone: cortege
depends_on:
- 204
created: 2026-09-13
updated: 2026-09-13
priority: p0
effort: m
area: crates/nether-bury
stratum: '0'
proof: A thousand parallel burials of one program produce exactly one distinct cairn
---

6.2 requires two burials of the same input to produce the same trace including
the order holes were found in. Parallelism is where that stops being free.

Run it a thousand times and count the distinct cairns. One is the only
acceptable answer, and a generative test is worth more than an example here:
the failure is a race and races do not reproduce on request.
