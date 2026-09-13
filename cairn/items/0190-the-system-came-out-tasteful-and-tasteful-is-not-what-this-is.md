---
id: 190
title: The system came out tasteful, and tasteful is not what this is
type: bug
status: buried
milestone: necropolis
created: 2026-09-13
updated: 2026-09-13
priority: p1
effort: m
area: site
stratum: '0'
proof: The nine strata are nine distinct colours, on the page and in the graphics
---

0187 replaced a loud borrowed palette with a derived one and lost the project
in the process. Everything it produced was correct — Oklch, one lightness
across the ramp, every pair WCAG AA — and correct came out as *safe*. Nine
near-identical pastels on a washed navy. It looked like a well-made
developer-tools site.

## What the constant lightness actually did

The argument was that depth should not mean "harder to see", so the nine held
one lightness and only the hue moved. Rendered side by side against
alternatives, that rule does two bad things:

- nine colours at one lightness read as one weight, so the ramp does not
  travel, it just changes tint;
- the most saturated colours sRGB has at high lightness are the pale ones, so
  holding lightness up **bleaches the deep end**. d6, d7 and d8 came out washed
  while d0 was vivid — the opposite of what the nine mean.

Falling a little in lightness and rising a little in chroma keeps every step
above 5:1 *and* lets depth look like depth.

## And the graphics were lying about the ramp

`DEPTH` in `site/gfx.py` was `[SULPHUR, SULPHUR, BILE, ROT, SALMON, SALMON,
PLUM, LILAC, LILAC]` — nine strata drawn with six colours, three of them
repeated, because sixteen slots were never nine plus everything else. Under
the old palette it was an approximation; under a generated ramp it collapsed to
one blue.

The colour table is thirty-two entries now and the nine have their own.

## Acceptance criteria

- [x] The ground has colour in it and the accent punches
- [x] The ramp travels, and the deep end is the most saturated rather than the palest
- [x] Every step still clears AA on both grounds
- [x] The strata graphic draws nine colours

## 2026-09-13

Found by rendering eight candidate palettes as mock pages and looking at them rather than arguing about them. The winner has a ground with real chroma in it -- black has none, so nothing put on it is related to it and the page reads as a terminal rather than as a place.

## 2026-09-13

A thirty-two entry colour table needs the packed field to say so and the LZW minimum code size to grow with it, and the table has to actually contain thirty-two entries. A short table is not a smaller table: everything after it shifts three bytes an entry and the file stops being a GIF.
