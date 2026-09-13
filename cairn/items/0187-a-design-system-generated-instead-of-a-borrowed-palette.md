---
id: 187
title: A design system, generated, instead of a borrowed palette
type: feature
status: buried
milestone: necropolis
created: 2026-09-13
updated: 2026-09-13
priority: p1
effort: l
area: site
stratum: '0'
proof: No stylesheet contains a colour, and both grounds clear AA including the strata
---

The palette was the sixteen VGA colours, complemented. Clever, and wrong twice:
it was somebody else's sixteen colours underneath, and they were chosen for a
text mode in the 1980s rather than for reading. Two of them had no opposite
inside the set and had to be invented, which was the first sign.

## What replaced it

`site/design.py` — Oklch, which is nine multiplications and a cube root and no
library. A colour is a lightness, a chroma and an angle, and two colours with
the same lightness look equally bright. That is the whole reason a ramp built
this way looks designed.

One axis: the ground is indigo, the complement of indigo is amber, the nine
strata sweep between them at a **single lightness**. Depth reads as hue and
never as "harder to see" — which matters, because §1.1 makes the strata a total
order and a ramp that dimmed as it went would make the deep ones a legibility
problem instead.

Nothing is at full chroma.

## Three scales, and no other numbers

- **Colour**, above, generated into `site/tokens.css`.
- **Space** is the cell: twelve pixels, the advance of the face. Every margin
  and rule is a count of them.
- **Type** is the cell doubled and trebled. Four sizes, because a face on an
  eight-pixel grid has four at which a pixel of it is a whole number of the
  screen's.

`nether.css` says what things are made of and contains no colours at all.

## Acceptance criteria

- [x] Every colour on the site is generated, and stated as L, C and H
- [x] Both grounds clear WCAG AA, including all nine strata on each
- [x] The stylesheet contains no hex
- [x] The graphics are made of the same colours as the page

## 2026-09-13

The dark ramp lands near 10:1 across all nine and the light one near 6:1, because lightness is held constant and only the hue moves. That is the property the old palette could not have: it was sixteen colours with sixteen different lightnesses.

## 2026-09-13

tests/contrast/run stopped duplicating the table. Duplication was right while the stylesheet was written by hand; retyping generated output is not the same discipline, and what is worth checking moved -- whether the generator produces something legible, not whether somebody copied it correctly.
