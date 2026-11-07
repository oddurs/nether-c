---
id: 232
title: A specimen a person can actually read
type: feature
status: buried
milestone: face
assignee: Oddur Sigurdsson
depends_on:
- 231
created: 2026-09-14
updated: 2026-11-07
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

- [x] No glyph can ship without appearing on the page. Both weights load in a real browser, remain comparable in both palettes and fit a phone without losing the grid.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

## 2026-11-07

Generated `site/specimen.html` from `site/font.py`'s glyph inventory (`G`, `bold()`, `SAME`) in `site/bake` (`specimen_page()`) and `site/font.py` (`main()`), linked into the primary navigation bar (`nav.bar`), the front-desk directory (`.desk-directory` -> `#the-face`), and its own section directory (`nav.contents`). Each of the 140 glyphs is catalogued with its `U+XXXX` codepoint, Unicode name, `400 + 700` or `SAME` weight note, side-by-side 400 and 700 `8x8` drawing grids (`pre.glyph-grid`), and live `Nether 8` text at `8px`, `16px`, `24px`, and `32px` in both weights, preceded by the four-size scale comparison and paired 400/700 narrow/wide, lookalike, shifted-bearing punctuation, calculus, box-drawing, Nether C, console, and prose samples. `tests/font/run` (`specimen_coverage`) decodes `cmap` from both `site/nether.woff` and `site/nether-bold.woff` and fails if any mapped glyph is absent from `site/specimen.html`, if a grid disagrees with `site/font.py`, or if any of the four live sizes in either weight is missing; removing `U+0041`'s card provoked `U+0041 (A) is in the regular cmap and not on site/specimen.html` (`returncode != 0`). `tests/necropolis/browser.mjs` navigates `/site/specimen.html` in headless Chrome across `1248px`, `768px`, `390px`, and `320px` viewports in both dark (`data-lamp="out"`) and lit (`data-lamp="lit"`) palettes, verifying 140 cards, 280 intact `8x8` grids with zero horizontal overflow, and equal-advance regular/bold rendering at `8px`, `16px`, `24px`, and `32px`. Limits: `U+0020` (`SPACE`) has an empty `8x8` grid by definition and shows its 7-column advance in the four live size boxes.
