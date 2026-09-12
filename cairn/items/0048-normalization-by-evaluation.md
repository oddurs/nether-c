---
id: 48
title: Normalization by evaluation
type: feature
status: buried
milestone: calculus
assignee: Oddur Sigurdsson
depends_on:
- 20
- 46
created: 2026-09-10
updated: 2026-09-12
priority: p0
effort: xl
area: crates/nether-core
stratum: '0'
proof: fib(30) burns to a literal; the residual contains no call
---

The partial evaluator. Pure code evaluates away entirely; anything that needs
the world residualises. This is burial.

## 2026-09-12

The rule for what reduces is depth and nothing else. A depth-0 expression is arithmetic on things that are already here; anything deeper is a question, and nothing has been granted, so it stays. That is section 6.1 read literally and it made the evaluator much smaller than a list of special cases would have.

## 2026-09-12

A function is not unfolded when it cannot be finished. The residue is smaller for it and section 6.2's compile(src) — pure, starving on a hole — comes out as the call it was written as rather than an inlined body with the same hole in it. A loop is unrolled speculatively and the unrolling abandoned whole if it stops going, which is safe because what stopped it was reaching the world and reaching the world is what did not happen.

## 2026-09-12

Only eight prelude functions fold: min max abs len slice concat starts_with raw. The rest need the world, need the ledger, or produce an array or a struct — and there is no way to write one of those down, so a value that is one cannot be residualised. 0116 is the reason and fixing it widens this list.

## 2026-09-12

Fuel is one step per expression evaluated, and the proof is that a burial one step short of what it spent runs out. The diagnostic that names where it starved rather than where it stopped is 0050.
