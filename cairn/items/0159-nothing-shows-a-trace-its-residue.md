---
id: 159
title: Nothing shows a trace its residue
type: feature
status: buried
milestone: rites
assignee: Oddur Sigurdsson
depends_on:
- 155
created: 2026-09-12
updated: 2026-09-12
priority: p2
effort: s
area: crates/nether-cli
stratum: '1'
proof: A person who buried a program can read the residue it produced
---

## Written, named, unreachable

`nether bury` prints the residue, stores it as `Bytes`, and names it in the
trace. Nothing surfaces the name: `bury --json` omits it, `lamp` on a trace
shows deposits, `strata` shows strata.

[§6.5](../spec/06-evaluation.md) makes it a MUST that a residue can be printed
and lowered back to the same program, and that is the thing a person would want
to check. Checking it today means scanning the store object by object, which is
what verifying §6.5 actually took.

## Acceptance criteria

- [x] A rite gives the residue's cairn for a trace
- [x] `bury --json` includes it

## 2026-09-12

Two ways in, because a person and a script want different ones: bury --json names the residue and the source, and lamp's one-line rendering of a trace names the residue so a provenance walk reaches it. The human summary is untouched, because section 6.8 fixes its shape.
