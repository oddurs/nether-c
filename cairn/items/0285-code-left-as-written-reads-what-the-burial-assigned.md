---
id: 285
uid: 8e3adcf4-1b8b-46d4-b066-aafc9fe541c1
title: Code left as written reads what the burial assigned
type: bug
status: buried
milestone: assay
assignee: Oddur Sigurdsson
created: 2026-11-07
updated: 2026-11-08
closed_at: 2026-11-08
priority: p0
effort: m
stratum: '0'
area: crates/nether-bury
proof: Burying the residue with an answer deposits what burying the program with it does, for an undecided branch that assigns, a statement left as written after an assignment, a branch reading a reassigned local, and a write in a finished block; the residue buried again is unchanged
---

## What happens

When burial leaves code as written, that code reads its locals in the residue.
A statement that only assigned was not in the residue, because its value lived
only in the burial's environment. So code left as written read a stale value.

```c
I64 g() {
  I64 x = 0;
  Bytes@3 a = must(descend disk { read("f") });
  x = 5;
  if (len(a) > 0) { x = x + 1; }
  x
}
```

Exhumed with a non-empty `f`, the trace deposited 5. The program gives 6. The
undecided branch was folded to its value, `U0`, and the assignment in its arm
went with it. Before #239 the same program gave 6, because burial stopped at
the starved statement.

The same gap is in every place burial leaves code as written: the statements
after a speculative one that jumps or collapses, the tail after them, and a
loop that stops unrolling.

## What should happen

§6.5: burying the residue with an answer gives what burying the program with
it gives.

## Fix boundary and regression proof

`crates/nether-bury/src/bury.rs`. `Env` records which locals an assignment the
residue does not hold has changed. Code left as written is preceded by an
assignment of each stale local it names, and after it, the locals it writes are
read as themselves. An arm of an undecided branch that writes a local the
branch outlives is left as written.

Four tests in `crates/nether-bury/tests/projection.rs` fail on the old code.
`burying_the_residue_again_leaves_the_same_residue` still passes.

The review also named a `flow.take()` in the guard of `after_starved`'s tail.
No program reached that arm (a probe panic in it never fired), so it is
unchanged.

## 2026-11-08

Verified: four tests in projection.rs fail on the old code; the bury suites pass, including burying_the_residue_again_leaves_the_same_residue. The ten-million monotone soak finds no counterexample on this branch with the Orpheus class skipped; that class fails the same way on main, came in with #239, and is 0287. naming_is_constant_in_what_has_already_been_named is flaky on main too, and is 0286.
