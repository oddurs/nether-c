---
id: 114
title: put writes objects that get can never read back
type: bug
status: buried
milestone: ledger
created: 2026-09-11
updated: 2026-09-11
priority: p0
area: crates/nether-ledger
proof: Every invalid state constructible through the public API is refused by put and never reaches disk
---

## What happens

`put` encoded without validating, but `decode_stored` enforces invariants the
`Value` and `Node` types do not: empty struct name, stratum past the lattice,
empty call name, backwards span. All are constructible through the public API.

    put(Value::Struct { name: "", fields: vec![] })  -> Ok(45032e25)
    get(45032e25)                                     -> Err(Corrupt { EmptyStructName })

The object is then permanently in the store, and `Corrupt` is the store's bit
rot signal — so an upstream bug is reported forever afterwards as a failing
disk.

## What should happen

Fixed: `put` round-trips the encoding before anything reaches disk and returns
`StoreError::Invalid`. Costs one decode per put, which is the same work `get`
does anyway.

The better fix is to make the invalid states unconstructible; that is its own
item.
