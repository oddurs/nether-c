---
id: 234
title: Read the face at the sizes it is read at
type: chore
status: unmarked
milestone: face
created: 2026-09-14
updated: 2026-09-14
priority: p1
effort: m
area: site/nether.css
stratum: '0'
proof: A written decision on which pixel sizes the face is used at, and why, with a screenshot of each
---

The face is drawn on an 8x8 grid and rendered at whatever `rem` works out to.
A bitmap face has opinions about that: at an integer multiple of its grid every
stem is a whole pixel, and between multiples the rasteriser guesses.

Nobody has checked what size the site actually renders it at, or whether that
is one of the good ones. This is a measurement and a paragraph, not a project.
