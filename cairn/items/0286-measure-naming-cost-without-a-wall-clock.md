---
id: 286
uid: b81b0870-7894-4203-8924-2a38aa6aaea3
title: Measure naming cost without a wall clock
type: bug
status: unmarked
milestone: assay
created: 2026-11-07
updated: 2026-11-07
priority: p2
effort: s
stratum: '0'
area: crates/nether-bury/tests/holes.rs
proof: naming_is_constant_in_what_has_already_been_named measures work, not elapsed time, and passes under a loaded machine
---

## What happens

`naming_is_constant_in_what_has_already_been_named` in
`crates/nether-bury/tests/holes.rs` compares two wall-clock durations. It
failed on `main` at 6773547, run alone: "four times the questions took
377.550542ms against 36.110125ms". It passes on most runs.

## What should happen

The test measures the shape of the work, as its comment says it means to, and
it does not depend on how loaded the machine is.

## Fix boundary and regression proof

Count the work (for example, fuel spent or comparisons made) instead of
`Instant::elapsed`, or compare medians of several runs with a bound wide enough
that a quadratic scan still fails it. Show that the quadratic version still
fails.
