---
id: 73
title: Replay refuses the world
type: feature
status: unmarked
milestone: world
depends_on:
- 61
- 67
created: 2026-09-10
updated: 2026-09-10
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
