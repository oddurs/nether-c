---
id: 284
uid: d694a99c-9e17-49b6-905a-f61489cf2ffd
title: A depth a loop carries is found however many turns it takes
type: bug
status: buried
milestone: assay
assignee: Oddur Sigurdsson
created: 2026-11-07
updated: 2026-11-07
closed_at: 2026-11-07
priority: p0
effort: m
stratum: '0'
area: crates/nether-core, crates/nether-syntax
proof: Bytes@0 w = z after while (c) { z = y; y = x; x = must(descend disk { read("f") }); } is refused, the differential generator produces chains of one to four links and agrees ten million times, and forty nested loops lower and check
---

## What happens

A depth that a loop carries through more than one assignment does not reach
the end of the chain. The checker accepts a value that holds a disk read as
pure.

## What should happen

Each turn starts in the state the last one ended in. A local is as deep as the
deepest value any turn can write into it (§5.4, §1.2).

## Reproduction

```c
U0 f(Bool c) @0 {
  Bytes x = b""; Bytes y = b""; Bytes z = b"";
  while (c) { z = y; y = x; x = must(descend disk { read("f") }); }
  Bytes@0 w = z;   // accepted; `w = y` is refused with "inference gives 3"
}
```

Before #257 both were refused. Nested loops also lowered in time that doubled
per level: 16 levels took 0.33 s and 18 took 1.78 s.

## Fix boundary and regression proof

`Lowering::looping` in `crates/nether-syntax/src/lower.rs`, the `Loop` rule in
`crates/nether-core/src/check.rs`, and the `Loop` rule of `model_check` in
`crates/nether-core/tests/calculus.rs`. Each walks to a fixed point.

- `a_depth_a_loop_carries_reaches_every_link_of_a_chain`, in `calculus.rs` and
  in `nether-syntax/tests/aggregates.rs`, fails on the old code.
- Generator mode 18 builds chains of one to four links. With the old checker
  the 50,000-case differential fails at step 35.
- `forty_nested_loops_lower_once_each` was still running on the old code when
  a 60 s alarm killed it.

## 2026-11-07

Found by the review of 1d2edf4..main. #257 replaced a check that caught this (the rules give depth 3; this node says 0) with a two-walk loop in both lower.rs and check.rs, and model_check copied the same two walks, so the soak agreed. Fix: walk to a fixed point in all three. The checker compares only state that outlives a turn, which keeps a loop with no carried change at one walk; the model compares the whole state on purpose. Verified: the new tests fail on the old code, and the forty-level nesting test was still running when a 60 s alarm killed it. 10,000,000 programs, 0 disagreements, 28.7 s.
