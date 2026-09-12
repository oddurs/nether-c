---
id: 145
title: Say whether the store is durable or only consistent
type: spec
status: unmarked
milestone: rites
created: 2026-09-12
updated: 2026-09-12
priority: p2
effort: s
area: spec/07-ledger.md
stratum: '0'
proof: Section 07 says which of crash-consistency and durability the store promises
---

## Two unstated things

**No `fsync`.** `write_atomically` writes a temp file and renames it. After a
power loss the rename can be durable while the bytes are not, and `get` then
reports `StoreError::Corrupt`, which is the store's signal for bit rot. The
failure is detectable rather than silent, which is the right side to be on —
but nothing says which of the two the store promises, so an implementer cannot
tell whether the missing `fsync` is a decision or an oversight.

**An append that is atomic in practice.** `add_ref` writes thirty-two bytes to
an `O_APPEND` handle. That is atomic on every filesystem anybody runs, and it
is not guaranteed: `write_all` may split. A torn record makes
`referrers`'s `chunks_exact(32)` produce garbage cairns, `has()` filters them
out, and an edge disappears — which is precisely the silent incompleteness the
ordering inside `put` is written to avoid.

## What to do

Say it in §07. If durability is wanted, `fsync` the temp file before the rename
and the directory after it, and say what that costs. If it is not, say that a
store is crash-consistent and that a torn write surfaces as `Corrupt`.

## Acceptance criteria

- [ ] §07 says which of the two the store promises
- [ ] The reverse-edge record says why a thirty-two byte append is safe, or is
      made safe
