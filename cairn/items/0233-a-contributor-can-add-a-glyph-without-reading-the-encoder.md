---
id: 233
title: A contributor can add a glyph without reading the encoder
type: docs
status: buried
milestone: face
assignee: Oddur Sigurdsson
created: 2026-09-14
updated: 2026-09-16
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

## Delivery plan — 2026-09-15

### Starting point and scope

CONTRIBUTING.md needs the missing-codepoint-to-PR path. Preserve the current deterministic, committed WOFF workflow; changing release storage is outside this documentation task.

### Steps

1. Trigger a missing-codepoint diagnostic in a disposable worktree and trace the named glyph to site/font.py.
2. Document eight-row syntax, six ink columns, seven-pixel advance, bearings, baseline and the matching bold glyph once 0231 exists.
3. Document `scripts/task font` and `scripts/task font:check`, the visual sheet and full check, then follow the instructions from a clean worktree.

### Acceptance and evidence

- [x] A contributor can reproduce the diagnostic, add a glyph, regenerate tracked assets and pass checks without reading the TrueType encoder. Any proposed distribution change belongs to 0235.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

## 2026-09-16

Written, and followed. CONTRIBUTING.md gains a 'Drawing a glyph' section: the diagnostic as it is actually printed, where the drawings live, the six things the grid asks (eight rows of eight, six ink columns, seven rows tall with row 7 the descender, name the character rather than its number, draw both weights or declare SAME, and look at it with --sheet), then scripts/task font and scripts/task check. I verified it by adding a paragraph with U+22A5 to spec/10-glossary.md, baking, watching tests/font/run name it, pasting the example straight out of CONTRIBUTING.md into site/font.py, and running the two commands: green, 140 glyphs. Walking it found two things the prose could not paper over. The regular glyphs beyond ASCII were written as \\uXXXX escapes while the bold ones I added in 0231 use the character itself, so a contributor had to guess which; 44 of them now name the character they draw, which changes no bytes in either .woff. And a contributor who drew the regular and forgot the bold got a KeyError traceback out of bold() -- the exact experience this item exists to remove. bold() now falls back to the regular for a glyph with no bold and weigh() reports it first, so both the forgot-it and the genuinely-un-boldable paths end in a sentence naming the glyph and the two ways out. On the question the item raised: the .woff files stay committed. The thing a reviewer has to read is the bitmaps beside them, the files are a deterministic function of those, and scripts/task font:check rebuilds and compares -- which is the opposite of web/necropolis/nether.wasm, never committed because a Rust release build is not byte-reproducible. That reasoning is in CONTRIBUTING.md; 0235 is about releasing the face with a licence and is untouched. scripts/task check passes in full.
