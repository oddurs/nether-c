---
id: 90
title: Icon, metadata and social preview
type: chore
status: buried
milestone: lamp
created: 2026-09-10
updated: 2026-09-10
priority: p2
effort: s
area: site/
proof: A pasted link previews correctly on GitHub, Slack and Bluesky
---

The favicon is four bands of the depth ramp, drawn as an SVG with
shape-rendering: crispEdges, at sixteen pixels. Still needed: an OG image, and
it should be baked rather than drawn by hand.

## 2026-09-10

Favicon done: site/icon.svg, four bands of the depth ramp at 16px with crispEdges. Outstanding: an OG image, baked rather than hand-drawn.

## 2026-09-10

Done. site/gfx.py gained a PNG encoder — signature, IHDR, PLTE, a zlib IDAT and IEND, about forty lines, because zlib and crc32 are both in the standard library and a link preview was not worth a dependency. card.png is 1200x630, drawn at 200x105 and scaled six times so it stays blocky, and it is baked and committed like everything else. og: and twitter: tags are in the template.
