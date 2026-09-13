---
id: 186
title: The nether has a ground of its own
type: feature
status: buried
milestone: necropolis
created: 2026-09-13
updated: 2026-09-13
priority: p1
effort: m
area: site
stratum: '0'
proof: Both palettes clear AA and no graphic sits in a box of the wrong colour
---

The page was black, and the nether is not empty, it is deep.

The sixteen complemented VGA colours are not the question. They are derived,
they are argued for in `spec/90-rationale.md`, and every one of them is still
here. What the *page* is made of is a different question, and it had never been
answered on purpose: black ground, and whichever of the sixteen came to hand.

## Where a nether colour comes from

The one colour system this project invented rather than inherited is the nine
strata, sulphur at 0 down to magenta at 8. A reader of this site is at the
bottom of it. So the ground is **stratum eight with no light falling on it**,
and the chrome is the deep end of the ramp.

The accent stays sulphur, because `lamp` is the only thing in the language that
carries light downwards, and a warm light in a cold dark is the whole of the
picture.

## What it touches

Every graphic is opaque, so the void a drawing sits in has to be the void the
page is, or each one gets a black box around it. `site/gfx.py`'s palette index
0 moves with the stylesheet.

And the headings had all been one size, which is the one thing a display face
is for.

## Acceptance criteria

- [x] The ground is derived from the strata, not chosen
- [x] Both palettes clear WCAG AA
- [x] No graphic sits in a box of the wrong colour
- [x] The face is used at more than one size

## 2026-09-13

#150020 is stratum 8 unlit; #B79ECB and #6E5580 are the same hue with more and less light on it. Every pair was checked against WCAG before a line of CSS was written, and tests/contrast/run holds them.

## 2026-09-13

The lit palette is the surface rather than the temple: the same violet with the sun on it, #FBF2FF. Keeping the inversion, dropping the borrowed frame.
