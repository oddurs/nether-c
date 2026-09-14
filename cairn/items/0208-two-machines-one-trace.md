---
id: 208
title: Two machines, one trace
type: feature
status: unmarked
milestone: cortege
depends_on:
- 215
created: 2026-09-13
updated: 2026-09-13
priority: p2
effort: xl
area: crates/nether-world
stratum: '6'
proof: Two machines each answer half a trace's holes, and the sealed result is byte-identical to one machine answering all of them
---

## The idea

A trace is a set of questions and a residue. Nothing says one machine has to
answer all of them.

Content addressing makes the merge trivial: two exhumations produce witnesses,
witnesses are values, and values with the same name are the same value. There
is no conflict to resolve because there is no way to disagree.

## Why it is xl anyway

Moving objects between stores is a protocol, and a protocol is a format, and
this project freezes formats carefully. It should not be invented in a hurry.
