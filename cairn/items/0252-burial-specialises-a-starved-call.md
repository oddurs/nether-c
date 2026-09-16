---
id: 252
title: Burial specialises a starved call
type: feature
status: unmarked
milestone: futamura
created: 2026-09-16
updated: 2026-09-16
priority: p1
effort: l
stratum: 0
area: crates/nether-bury
proof: Burying lib/interpreter.nc against build.nc leaves a residue whose demands do not name `interpret`
part_of: [80]
---

## Problem

`crates/nether-bury/src/bury.rs` throws away everything a starved call
reduced. `apply` returns `None` the moment the body's result is stuck, so
`call` residualises the call as it was written. `block` does the same at a
smaller scale: it returns at the first statement that starves and keeps
nothing.

`spec/06-evaluation.md` §6.5 now requires the opposite. Nothing else in the
repository does.

## Proposal

Two changes, both in `bury.rs`.

`block` builds the residue §6.5 describes: the bindings that finished, then
the statement that starved and every statement after it, unreduced. A
statement that finished and bound nothing is dropped — it happened, and its
deposit is in the trace.

`apply` mints a function for a body that starved. Every argument of a starved
call is a value, so the specialised body takes no parameters: the arguments
become bindings at the top of it, the locals table is the one the original
body had, and the call site names the minted function. `Residue` carries the
minted functions and `as_unit` appends them.

## Starting point and non-goals

A prototype of both was written against `3b0195e` and measured; the numbers in
`spec/90-rationale.md` are from it. It specialised `interpret(build.nc)` into
21 functions, and `world_request` came out as `read("main.nc")` with the
guest's path already resolved. It was not kept: it had no dead-binding
elimination, no memoisation of a function already specialised to the same
arguments, and no accounting for what the minted functions do to the
`.decay-ceiling`.

Not in scope: reducing inside an arm that may not be taken (0251). The residue
this produces is specialised up to the first unanswered question on its path
and interprets from there, which is what §6.5 claims and no more.

## Delivery steps and dependencies

1. `block` first, on its own: it is observable through existing starving-call
   tests without any minting.
2. `apply` and the minting, with memoisation on `(FuncId, arguments)` so a
   function specialised twice to the same constants is one function.
3. Drop bindings nothing in the residual body reads, and measure the residue
   against the 1,478 lines the prototype produced.
4. Check the residue: `check(&residue.as_unit(&unit))` must stay empty, and
   the printed residue must parse, lower and re-bury to the same result. The
   minted functions' `ret_depth` and `latent` are where that will break first.
5. Confirm the staging law on deposits. Today a starved call's body is
   evaluated, its deposits recorded, and the call re-run by the next burial;
   once the body is specialised the second burial does not redo it, so
   `crates/nether-bury/tests/interpreter.rs` has cases that will need to say
   something different from what they say now.

## Which stratum does this reach?

0. Burial holds no capability and this changes nothing about what it asks.

## Acceptance criteria

- [ ] `block` and `apply` residualise as §6.5 requires
- [ ] The interpreter buried against `build.nc` leaves no demand naming `interpret`
- [ ] The printed residue parses, checks and re-buries to the result the
      unspecialised route gives, deposits included
- [ ] `scripts/task check` passes, with `.decay-ceiling` justified in the PR body

## Evidence to close

Record the tested commit, the residue's line count against the 1,163 lines the
unspecialised route produces, and the fuel both routes spend.
