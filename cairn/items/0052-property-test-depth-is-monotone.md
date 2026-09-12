---
id: 52
title: 'Property test: depth is monotone'
type: chore
status: buried
milestone: calculus
assignee: Oddur Sigurdsson
depends_on:
- 47
created: 2026-09-10
updated: 2026-09-12
priority: p1
effort: m
area: crates/nether-core
stratum: '0'
proof: Ten million generated programs, no counterexample to the bound in section 2.4
---

Generate arbitrary well-typed programs; assert that no evaluation step ever
lowers the depth of a value, and that application depth is exactly the max of
its parts. The central invariant deserves a generative test, not examples.

## 2026-09-12

Ten million programs across eight seeds, 1507967 of them deeper than the surface, in 235 seconds. Four claims per program: the checker agrees with the generator, the checker complains when one stated depth is changed, burial never exceeds the bound, and what burial hands back is a program the checker accepts.

## 2026-09-12

The generator is a second implementation of section 2.2 rather than a wrapper round the first. Programs are built bottom-up with their depths computed as they go, so claim one is two implementations agreeing and not one implementation agreeing with itself. Claim two is what stops claim one being vacuous.

## 2026-09-12

It found three things. Round 47 of the first seed refuted section 2.4's monotonicity law, which is 0129. Round 50 found burial stating a depth on a residual call that its reduced arguments no longer justified. Round 3180 of the soak found a residual shade claiming an origin its value never had, from the same cause: an arm that is not taken can make an operand shallower than its type said, and anything derived from that operand has to be derived again.
