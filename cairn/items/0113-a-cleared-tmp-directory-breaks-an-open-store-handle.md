---
id: 113
title: A cleared tmp directory breaks an open Store handle
type: bug
status: buried
milestone: ledger
created: 2026-09-11
updated: 2026-09-11
priority: p2
area: crates/nether-ledger
proof: A put succeeds after the tmp directory is removed, as a test
---

## What happens

`write_atomically` created the destination's parent but wrote into
`<root>/tmp` on faith. `open` creates it once; a handle is long-lived and
`tmp/` is exactly what an operator or a tmp reaper clears. A write that died
before its rename also stayed there forever.

## What should happen

Fixed in #32: the directory is created per write, and a failed write removes
its own partial.
