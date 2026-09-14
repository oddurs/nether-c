---
id: 232
title: A specimen a person can actually read
type: feature
status: unmarked
milestone: face
created: 2026-09-14
updated: 2026-09-14
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
