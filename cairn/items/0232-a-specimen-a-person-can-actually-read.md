---
id: 232
title: A specimen a person can actually read
type: feature
status: unmarked
milestone: face
depends_on:
- 231
created: 2026-09-14
updated: 2026-09-15
priority: p1
effort: m
area: site/
stratum: '0'
proof: A specimen page shows every glyph at its drawn size and at the sizes the site uses, in both weights, and the font check fails if a glyph is in the face but not on it
---

The specimen today is a GIF, drawn by `specimen()` in `site/font.py`. It
proves the face exists; it does not let anybody look at it.

A face that asks people to contribute glyphs needs a page that shows what is
there, at the size it will be read at, next to the grid it was drawn on. That
page is also the place a contributor checks their glyph against its neighbours
before opening a pull request.

It should be generated, like everything else on this site, and the font check
should fail if the face grows a glyph the specimen does not show.

## Delivery plan — 2026-09-15

### Starting point and scope

The generated GIF specimen has dark/light companions but is not an inspectable glyph catalogue. Build the page from site/font.py's glyph inventory, not a second handwritten list.

### Steps

1. Generate a catalogue with codepoint, name or label, drawing grid and actual-font text at 8/16/24/32px.
2. Show regular immediately; add both weights once 0231 lands. Include narrow/wide pairs, punctuation, box drawing and representative code/prose.
3. Link the page into the existing site directory and check its inventory against each shipped face.

### Acceptance and evidence

- [ ] No glyph can ship without appearing on the page. Both weights load in a real browser, remain comparable in both palettes and fit a phone without losing the grid.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
