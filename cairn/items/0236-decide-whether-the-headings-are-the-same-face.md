---
id: 236
title: Decide whether the headings are the same face
type: spec
status: buried
milestone: face
assignee: Oddur Sigurdsson
created: 2026-09-14
updated: 2026-09-16
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

- [x] The rationale accurately distinguishes live typography from raster lettering. A decision is sufficient; do not redraw glyphs or introduce another face without a separately scoped item.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

## 2026-09-16

Decided: one face. spec/90-rationale.md §90.6 records it. Live text on both rooms is Nether 8 from site/font.py and nothing else -- prose, headings, listings, sign and lamp; the headings are the same letterforms at 24px and 16px against a 16px body, and the hierarchy is colour, uppercase and letter-spacing on h1, a filled bar on h2, and the block characters that precede all three. The 5x7 alphabet in site/gfx.py is not a second face: 51 characters, capitals and digits, drawn into GIF frames pixel by pixel; it is lettering inside a drawing and no rule sets text in it. Rejected: a display face on a bigger grid -- 138 more glyphs, a second .woff for a few dozen words a page, and a second grid, which is the decisive one, since the face is a pixel face on an eight-unit cell and the scale is 8/16/24/32, whole multiples of it. Limit: 0234 has not run, so there is no measured evidence of how the face reads at those sizes on a real screen; this decision is about which faces exist and not about whether 24px is the right heading size. A second weight is 0231 and unaffected. scripts/task check passes.
