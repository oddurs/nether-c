---
id: 110
title: Struct and array counts are believed without checking the input
type: bug
status: buried
milestone: ledger
created: 2026-09-11
updated: 2026-09-11
priority: p1
area: crates/nether-ledger
proof: A count of u64::MAX is refused before any allocation, for every tag that carries a count
---

## What happens

`cairns()` refused a count the input could not carry. `Struct` and `Array` did
not, and a `Value` is larger than a cairn, so the amplification was worse:
8 MB of input reserved 396 MB before erroring.

## What should happen

7.1.1 already requires rejecting a count that exceeds the bytes remaining.

Fixed in #31: one `believable` check wherever a count is read, before any
allocation. The fuzzer does not catch this because it checks panics and
canonicity and has no opinion about memory.
