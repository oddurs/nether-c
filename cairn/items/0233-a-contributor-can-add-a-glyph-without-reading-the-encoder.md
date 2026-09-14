---
id: 233
title: A contributor can add a glyph without reading the encoder
type: docs
status: unmarked
milestone: face
created: 2026-09-14
updated: 2026-09-14
priority: p0
effort: m
area: site/font.py
stratum: '0'
proof: A stated path from the build saying a codepoint is missing to a drawn glyph in a pull request, short enough to sit in CONTRIBUTING.md
---

## Why this one matters most for a community

`tests/font/run` reads every page and every specification section, collects
the codepoints, and fails naming any the face does not have. That is a good
check and it has a consequence: **the first time somebody writes prose with a
character we have not drawn, their build breaks.**

That is correct behaviour and a terrible first experience if the only way
through it is to read a TrueType writer.

## What it needs

The bitmap format is eight strings of eight characters and it is already the
easiest part of the file. What is missing is a sentence in `CONTRIBUTING.md`
saying: the build named a codepoint, here is where the grid lives, draw it,
run this, commit the `.woff`.

Worth checking whether the `.woff` should be committed at all, or built in
CI — a binary in a diff is a bad review experience, and this one is 3.2kB and
deterministic.
