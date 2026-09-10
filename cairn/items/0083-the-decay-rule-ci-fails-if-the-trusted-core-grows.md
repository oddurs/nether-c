---
id: 83
title: 'The Decay Rule: CI fails if the trusted core grows'
type: chore
status: buried
milestone: codex
created: 2026-09-10
updated: 2026-09-10
priority: p1
effort: m
area: .github/workflows
stratum: '0'
proof: A PR that grows the trusted core without lowering the ceiling fails CI
---

TempleOS was fixed at a size given by covenant. We invert: the trusted core
may only ever get smaller.

A checked-in ceiling file records the current line count of the trusted
crates. CI fails if the count exceeds it. Lowering the ceiling is an ordinary
commit; raising it means editing the file deliberately and saying why in the
PR body.

Below, things only decay.

## 2026-09-10

PROVEN. CI run 34439643442 failed at the decay step: ceiling 64, core 72, 'the trusted core grew'. PR #10 was opened deliberately to grow the core, went red for that reason alone, and was closed unmerged; the branch is gone. tests/decay/run guards all four behaviours permanently so the rule cannot silently stop working. Worth recording: the first attempt failed CI at clippy instead, because the filler landed after the test module. A red build is not a proof on its own — it has to be red for the reason claimed.
