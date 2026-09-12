---
id: 73
title: Replay refuses the world
type: feature
status: buried
milestone: world
assignee: Oddur Sigurdsson
depends_on:
- 61
- 67
created: 2026-09-10
updated: 2026-09-12
priority: p0
effort: m
area: crates/nether-world
stratum: '0'
proof: With the network unplugged and the source deleted, replay still succeeds
---

`--replay` serves only from the ledger and holds no capabilities at all.
Not 'prefers the cache' — cannot reach the world, structurally. If replay can
be made to touch a socket under any argument, the flag is a lie and the
guarantee it implies is worthless.

## 2026-09-12

A Replay holds a map from a question to what was said and nothing else -- no World, no Provider, no path, no socket -- and there is no method that takes one. So it is not that replay prefers the ledger; there is nowhere else for it to look. The compiler checks the last part: granting a Replay anything is a compile_fail doctest.

## 2026-09-12

Proved with the source deleted: bury, exhume --grant disk, then remove both main.nc and build.nc, then exhume --replay prints identical.
