---
id: 236
title: Decide whether the headings are the same face
type: spec
status: unmarked
milestone: face
created: 2026-09-14
updated: 2026-09-15
priority: p2
effort: s
area: site/
stratum: '0'
proof: Section 90 records whether one face at two sizes was a decision or an accident, and what was rejected
---

The headings and the body are the same face at different sizes, which is
either a strong decision or something nobody chose. It reads as deliberate and
that is not the same as being deliberate.

If it is deliberate, it belongs in `spec/90-rationale.md` with the rejected
alternative — a display face drawn on a bigger grid, which is the obvious other
answer and would cost a second set of bitmaps. If it is not, this is where the
choice gets made.

## Delivery plan — 2026-09-15

### Starting point and scope

Body/headings use Nether 8; graphics also use the separate 5x7 bitmap alphabet in site/gfx.py. Decide whether this division is intentional without assuming the project has only one drawing set.

### Steps

1. Inventory live text, ASCII signage and raster labels, and capture representative current examples.
2. Compare retaining the existing roles with a separate heading face; account for glyph maintenance, legibility and download cost.
3. Record the chosen roles and rejected alternative in spec/90-rationale.md, linking the size evidence from 0234 when available.

### Acceptance and evidence

- [ ] The rationale accurately distinguishes live typography from raster lettering. A decision is sufficient; do not redraw glyphs or introduce another face without a separately scoped item.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
