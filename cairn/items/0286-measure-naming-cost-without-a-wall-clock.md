---
id: 286
uid: b81b0870-7894-4203-8924-2a38aa6aaea3
title: Measure naming cost without a wall clock
type: bug
status: buried
milestone: assay
assignee: Oddur Sigurdsson
created: 2026-11-07
updated: 2026-11-08
closed_at: 2026-11-08
priority: p2
effort: s
stratum: '0'
area: crates/nether-bury/tests/holes.rs
proof: naming_is_constant_in_what_has_already_been_named passes 20 of 20 at load 26 and 10 of 10 at load 52, and fails with the scan put back
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

## 2026-11-08

It was not only a clock. On every core, the parallel handoff's cost grows with the number of demands (0291), which is what tipped the old 4x-and-8 form over. The test now runs on one thread, compares 500 against 8,000 questions, and keeps the fastest of five interleaved runs. At load 40, a debug build gave 25 to 41 times for the code as it is and 200 times with the scan put back in remember; the bound is 90. 20 of 20 passed at load 26, and 10 of 10 at load 52 with eight extra busy loops.
