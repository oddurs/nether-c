---
id: 231
title: Draw a bold weight, and refuse to fake one
type: feature
status: unmarked
milestone: face
created: 2026-09-14
updated: 2026-09-14
priority: p0
effort: l
area: site/font.py
stratum: '0'
proof: A second face at weight 700 ships, font-synthesis is none, and a bold run measures the same advance as the same text unbolded
---

## Problem

Seventeen rules in `site/nether.css` ask for `font-weight: bold`. The face has
one weight and nothing sets `font-synthesis`, so the browser fakes it — it
smears each glyph sideways and composites it over itself.

On a face drawn as 8x8 pixels that is precisely the wrong thing. Every stem
that was one pixel becomes one pixel and a fraction, the grid stops lining up,
and the letterform stops being the letterform. It is visible on the front page
in every `strong`, every keyword in a listing, and all nine depth colours.

## Proposal

Draw the bold. The glyphs are bitmaps in `site/font.py`; a bold is a second
set of bitmaps, not an algorithm. Where a stem is one pixel it becomes two, and
where that would close a counter it does not — which is a decision per glyph
and the reason this is not a transform.

Then set `font-synthesis: none` so that a weight or a style the face does not
have fails visibly instead of being invented. A missing bold that looks wrong
is a bug report; a smeared one is a thing people assume was intended.

## Watch out for

The advance. The face is monospace at six columns, and a bold that is wider
than its regular breaks every listing and every diagram on the site. Two
pixels of stem have to fit inside the same six columns.

## Acceptance criteria

- [ ] A bold face, drawn, not derived
- [ ] `font-synthesis: none` in the stylesheet
- [ ] The advance is identical to the regular's
- [ ] A check that the two faces have the same glyph set
