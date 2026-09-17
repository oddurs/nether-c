---
id: 231
title: Draw a bold weight, and refuse to fake one
type: feature
status: buried
milestone: face
assignee: Oddur Sigurdsson
created: 2026-09-14
updated: 2026-09-16
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

The advance. The face is monospace at seven columns, and a bold that is wider
than its regular breaks every listing and every diagram on the site. Two
pixels of stem have to fit inside the same six ink columns and preserve the
seventh column's metric bearing.

## Acceptance criteria

- [x] A bold face, drawn, not derived
- [x] `font-synthesis: none` in the stylesheet
- [x] The advance is identical to the regular's
- [x] A check that the two faces have the same glyph set

## Delivery plan — 2026-09-15

### Starting point and scope

The regular face now advances seven pixels: six ink columns plus one metric bearing (site/font.py). There is still only a normal WOFF; bold rules remain in site/nether.css. Keep this a weight addition, not another regular-face redesign.

### Steps

1. Freeze the current glyph set and regular metrics as the comparison baseline.
2. Draw bold glyphs individually, preserving counters and the seven-pixel advance; do not mechanically dilate every bitmap.
3. Generate a separate 700 face, disable synthesis and extend tests/font/run plus real-browser width/load checks.

### Acceptance and evidence

- [x] Both weights cover the same codepoints and measure identically for mixed punctuation, narrow and wide glyph runs. Review both palettes at actual CSS sizes; regular outlines remain unchanged.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

## 2026-09-16

Drawn. site/font.py now holds a second set of bitmaps: 117 glyphs drawn twice and 22 named in SAME, which are the same drawing at both weights -- blocks and box drawing, because they rule listings and headings that must join whatever the text around them weighs, and a handful of punctuation with no room in six columns for a second pixel anywhere. The rule is that a vertical run of two or more rows gains a pixel inward, and nothing else does: a diagonal is a column of one-row runs and keeps its angle, a horizontal bar is a row of them and keeps its weight, and a row may not lose a gap. That last one is why M, W and V are heavier at the stems and not at the vertex; filling the one column between two strokes is what a browser's synthetic bold does and it is why M comes out of one a filled square. Twenty-eight glyphs were drawn by hand where that rule was wrong or did nothing: the rules and dots, the chevrons and slashes, 0 (squared rather than slashed, because a bold ring leaves a two-pixel counter and a slash through it is a filled counter, and a bold 0 without the slash is a bold O), 1 and l and i and j (whose stems would otherwise all become the same bar), 4, s, x, X, Q, and the punctuation whose dot stayed one pixel under a two-pixel stem. site/font.py weigh() refuses a bold that is wider than six columns, lighter than its regular, short of a counter the regular has, no heavier without being declared so, or declared the same and is not; both failures were provoked deliberately and both were caught. tests/font/run runs every existing check against both files and adds the ones that only mean anything between two faces: identical coverage, identical advances, identical hhea, and an OS/2 and head.macStyle that say which weight each is. The browser suite checks the bold face loads, that font-synthesis computes to none, that a canvas measures a bold run exactly as wide as the same text unbolded at 8/16/24/32, and that a strong element in the page is the same width as a span with the same text; removing font-synthesis from the stylesheet fails that assertion, which is how I know it runs. The regular's glyf, loca, hmtx, cmap and OS/2 tables are byte-identical to what was committed before; only name and head changed, because the regular now says Regular in its subfamily and Nether8-Regular in its PostScript name, which is what a two-weight family needs. 3,312 bytes regular and 3,316 bold. scripts/task check passes in full. Limits: nine glyphs -- ! ' . : > | · ᵢ • -- start one column further left in the bold than in the regular, which moves the drawn bearing and not the advance; m keeps its regular drawing because three stems will not fit twice in six columns, and § is only one pixel heavier.
