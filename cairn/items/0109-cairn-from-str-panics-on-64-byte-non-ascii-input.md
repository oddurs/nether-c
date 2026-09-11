---
id: 109
title: Cairn::from_str panics on 64-byte non-ASCII input
type: bug
status: buried
milestone: ledger
created: 2026-09-11
updated: 2026-09-11
priority: p0
area: crates/nether-ledger
proof: A 64-byte string containing a multi-byte character is refused rather than panicking, pinned as a unit test
---

## What happens

`str::len` counts bytes; the loop sliced `&s[i*2..i*2+2]`, which indexes bytes.
A 64-byte string containing a multi-byte character panics on a boundary.

    "\u{20ac}" + "a".repeat(61)   // 64 bytes
    -> panicked: byte index 2 is not a char boundary

## What should happen

`ParseCairnError::NotLowercaseHex`.

`from_str` is how every rite takes a cairn from whoever is typing, and
SECURITY.md puts panics on untrusted input in scope. Fixed in #31 by working on
bytes throughout. The fuzzer had never touched this entry point; it does now.
