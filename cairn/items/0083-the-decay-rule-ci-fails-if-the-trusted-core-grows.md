---
id: 83
title: 'The Decay Rule: CI fails if the trusted core grows'
type: chore
status: unmarked
milestone: codex
depends_on:
- 46
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
