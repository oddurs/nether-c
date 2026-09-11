---
id: 112
title: Concurrent puts silently lose reverse edges
type: bug
status: buried
milestone: ledger
created: 2026-09-11
updated: 2026-09-11
priority: p0
area: crates/nether-ledger
proof: Sixteen writers adding an edge to one target all survive, as a test that fails when the bug is put back
---

## What happens

`add_ref` was a read-modify-write with no locking. Two writers each read the
old file and wrote their own one-entry result; last rename won, other edge
gone. Nothing repaired it: `referrers` only filters edges whose *referrer*
object is missing, and a later `put` early-returns on `has` before reaching
`add_ref`.

This is precisely the silent incompleteness the ordering in `put` claims to
avoid, and it breaks 7.4.

## What should happen

Fixed in #32: append one record per edge; `referrers` deduplicates on read.

Worth noting the two bugs interacted — the temp-path failure masked this one,
because puts died before ever reaching `add_ref`. It had to be isolated to be
seen.
