---
id: 229
title: The diagrams that teach were too small to read
type: bug
status: buried
milestone: necropolis
created: 2026-09-14
updated: 2026-09-14
priority: p1
effort: s
area: site/
stratum: '0'
proof: orpheus.gif, hole.gif and descending.gif render at an integer 2x, and their labels are legible at 1440
---

Three graphics on the front page carried labels that explained something, and
all three were drawn small and shown at 1:1 in a column three times their
width. `site/gfx.py` draws them with the 5x7 bitmap face, which at 1x is seven
pixels tall on screen, so `LOOK: NOT FROM UP HERE`, `DESCEND FIRST` and the
`HERE` marker on the descent ladder were mud.

Shown at an integer 2x instead — exact for pixel art under
`image-rendering: pixelated`, and not one new byte on the wire.

Landed in #154. The item is recorded after the fact: the commit that carried
the code was amended to include it and the amend was never pushed, so the
change merged without it.
