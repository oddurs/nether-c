---
id: 154
title: Naming a local freezes it
type: feature
status: unmarked
milestone: surface
created: 2026-09-12
updated: 2026-09-12
priority: p0
effort: m
area: crates/nether-core
stratum: '0'
proof: 5.4's own sample is rejected at its last line, naming where the value was sealed, and the sample's Shape moves from Unchecked to Illegal
---

## The rule

§5.4: a local may be assigned until it is **named** — sealed, shaded,
deposited, returned, or passed as an argument. After that its cairn exists and
nothing can change what a cairn names.

0153 built the declaring and the assigning. The freezing is not built.

## Why it matters

Without it the language is unsound against its own central claim. This
compiles today and should not:

    Header h;
    h.len = 3;
    Cairn c = seal h;
    h.len = 4;          // §5.4 says this is an error

`c` names a value that then changed. That is the one thing the ledger promises
cannot happen.

It is §5.4's own sample, and its entry in `crates/nether-syntax/tests/spec`
is `Shape::Unchecked("0154")` for exactly this reason — the sample parses, the
checker accepts it, and it should not.

## What it needs

An escape analysis in the depth checker. A local is frozen from the first
expression that uses its value as a value; assigning a frozen local is a
diagnostic that names where it was named, not merely that it was.

Reading a field that was never assigned collapses, which is the other half of
§5.4 and is also not built.

## Watch out for

A branch that names a local on one arm only. The conservative answer — frozen
if any path names it — is almost certainly right, and it should be written
down rather than discovered.
