---
id: 290
uid: eb101fc3-75b3-46b1-9ab7-1dc80920624c
title: Burial leaves code as written that would lower a depth
type: bug
status: buried
milestone: assay
assignee: Oddur Sigurdsson
created: 2026-11-08
updated: 2026-11-08
closed_at: 2026-11-08
priority: p0
effort: m
stratum: '0'
area: crates/nether-bury
proof: The monotone soak passes ten million programs with no class of fault skipped, and a residue buried from each of the three paths checks
---

## What happens

§6.5's *A residue keeps its depths* (0287) says a residual that is not a value
may not be shallower than the code it replaced. Burial broke it in two places:
a decided branch took a shallower arm, and a starved call was minted to a body
that returned less deeply than its function. Each left a residue the checker
refused: `cannot look at a shade from stratum 3; 5 is held here`.

## Fix boundary and regression proof

`crates/nether-bury/src/bury.rs`.

- `Burial::select`: when the condition is decided and `lowers(arm, other)`, the
  arm runs. A value is kept. Otherwise its deposits, minted functions and
  queued settles are rolled back, the questions it asked are kept, and the
  branch stays whole as written, its condition reduced.
- `Burial::apply`: a body whose `ret_depth` is below the function's stays the
  call, with the same rollback.

`a_residue_keeps_its_depths` (projection.rs) and
`the_round_that_lowered_a_shade_s_origin_holds` (monotone.rs) fail on the old
code. The ten-million monotone soak passes with no class skipped: 10,000,000
programs, 1,606,590 deeper than 0, no counterexample, 2983 s.

## 2026-11-08

After the soak, lowers() moved from a BTreeMap to a Vec indexed by local id (the map cost 3658 compressed bytes in the browser build) and select moved to let-else for clippy. A one-million soak on the final code: 1,000,000 programs, no counterexample.
