---
id: 175
title: What a foreign call looks like from the other side
type: spec
status: buried
milestone: world
assignee: Oddur Sigurdsson
created: 2026-09-13
updated: 2026-09-13
priority: p0
effort: s
area: spec
stratum: '8'
proof: A C function somebody could write against, and a declared place to load it from
---

[§9.8](../spec/09-prelude.md) gives `call_foreign` a signature in Nether C and
says nothing about the other side of it. 0072 needs three things it does not
have.

## Where the code comes from

`Disk` has a root, `net` has a reach ([§8.3.2](../spec/08-rites.md)), and
`unrecorded` has nothing. A stratum-8 call that can load any shared object on
the machine is the ambient authority §9.1 refuses, in the one place where it
matters most.

## What the callee's signature is

`Answer<Bytes> call_foreign(Str sym, Bytes args)` says what the *program*
writes. It does not say what C function `sym` has to be, so no C is writable
against it and no implementation can call one.

## Where the `unsafe` lives

`Cargo.toml` has `unsafe_code = "forbid"` for the whole workspace, and a
foreign call cannot be written under it. [§1.7](../spec/01-strata.md) settled
that stratum 8 exists and is quarantined loudly; this is the same quarantine,
one layer down, and it belongs in its own crate rather than in a relaxed lint.

## Acceptance criteria

- [x] §8.3.3 says how a loadable object is declared
- [x] §9.8 gives the callee's signature and says who owns the memory
- [x] The rejected alternatives are in `spec/90-rationale.md`

## 2026-09-13

Caller owns both buffers, and is called twice when the first is too small. The callee allocating and the caller freeing makes the two sides share an allocator they do not have, and the failure is a corrupted heap at some later unrelated moment -- unusable in a language whose whole argument is that you can tell afterwards what happened.

## 2026-09-13

The unsafe gets a crate, not a relaxed lint. forbid exists so it cannot be locally undone, and downgrading it to deny to make one module possible makes every module possible. §1.7 quarantines this stratum loudly; this is the same quarantine one layer down.

## 2026-09-13

The witness is written before the call, so a callee that does not return still leaves a trace saying what was attempted. That is the only guarantee available at stratum 8.
