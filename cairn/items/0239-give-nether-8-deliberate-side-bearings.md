---
id: 239
title: Give Nether 8 deliberate side bearings
type: bug
status: buried
milestone: face
assignee: Oddur Sigurdsson
created: 2026-09-14
updated: 2026-09-15
priority: p0
area: site/font.py
effort: m
stratum: '0'
proof: Nether 8 keeps its full six-pixel drawings and a seven-pixel fixed advance with one metric bearing; the rebuilt WOFF and specimen pass font, graphics, site, and Cairn checks
---

## Problem

The full-width 8x8 forms had no breathing room in the browser's fixed-pitch
cell. An attempted global horizontal compression proved that the cure cannot
be a smaller alphabet: it damaged the widest forms.

## Resolution

Keep the six-pixel bitmap drawings intact and give every glyph a seven-pixel
fixed advance. That reserves at least one blank column without pair kerning,
so listings remain aligned. Each left bearing matches its drawing's xMin;
narrow punctuation retains its inset instead of shifting to the cell's edge.

The 5x7 display face received only two construction repairs: `W` now finishes
at its inner feet, and `X` begins its diagonal immediately. A contextual pass
covered wide words, `O/0/Q`, `1/I/l`, punctuation, and prose; no further
conservative changes were warranted.

## Verification

- `scripts/task font:check`
- `scripts/task gfx:check`
- `scripts/task test`
- `cairn check`
