---
id: 148
title: Hygiene found by the review
type: chore
status: buried
milestone: after
created: 2026-09-12
updated: 2026-09-12
priority: p3
effort: s
area: crates
stratum: '0'
proof: Each of the five is fixed or has a stated reason not to be
---

The `find` word-splitting went with 0139, which rewrote that loop, and
`referrers`'s error type went with 0145, which gave it a new error to return.

Small things the review turned up. None of them is wrong today; each is a way
for something to become wrong quietly.

- **`nether-bury` and `nether-cli` list `nether-ledger` in both
  `[dependencies]` and `[dev-dependencies]`.** The second does nothing.
- **`lex::at` and `parse`'s span constructor clamp with
  `u32::try_from(..).unwrap_or(u32::MAX)`.** A source larger than four
  gigabytes gets silently wrong spans rather than being refused.
- **`CLAUDE.md`'s seam list omits `proofs` and `fuzz`**, which
  `scripts/task`'s own usage line has.

## And one that had already gone wrong

`scripts/decay` reads its ceiling with `tr -dc '0-9'` and does not check that
anything came out. With an empty file the two comparisons fail with "integer
expression expected", the script falls past both of them, and it reports
success — so a `.decay-ceiling` holding nothing turns the rule off and says
nothing.

One reached `main` in #84. The rule was off for four commits, and CI was green
for all of them, because CI runs the same script.

## Acceptance criteria

- [x] Each of them is fixed, or has a sentence saying why not
- [x] A ceiling that is not a number fails, and `tests/decay/run` covers it

## 2026-09-12

A source larger than four gigabytes is now refused by lex rather than clamped, with MAX_SOURCE stated beside the reason: a span is two u32s, so a longer source has bytes no diagnostic could point at. Four gigabytes is not a fixture, so what the test checks is the pair that could drift -- the limit and what a span holds.

## 2026-09-12

Turned up one that had already gone wrong: scripts/decay reported success on an empty ceiling file, because the comparisons failed and it fell past them. One reached main in PR 84 and the rule was off for four commits with CI green throughout, since CI runs the same script. Fixed, covered, and the ceiling restored.
