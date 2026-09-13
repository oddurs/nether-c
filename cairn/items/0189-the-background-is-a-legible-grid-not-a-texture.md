---
id: 189
title: The background is a legible grid, not a texture
type: bug
status: buried
milestone: necropolis
created: 2026-09-13
updated: 2026-09-13
priority: p1
effort: s
area: site
stratum: '0'
proof: The background texture is within 1.2:1 of the ground
---

## What happens

## What should happen

## Reproduction

1.

## Cairn of the offending trace

`bg_tile` drew its sediment in `ASH`. That was a mid grey when the palette was
the complemented VGA sixteen. 0187 rebound the sixteen slots to the generated
system and `ASH` became `--dimmer`, which is **3.46:1** against the ground.

3.46:1 is the ratio a *boundary* has to clear to be seen on purpose. A page
tiled with it is a page with a grid drawn over the text.

`--raised` is 1.13:1: there if you look for it, gone if you do not, which is
what sediment is. It also thins out -- two broken strata to a tile and a few
grains, rather than a dotted line every eight rows.

## Acceptance criteria

- [x] The texture is under 1.2:1 against the ground
- [x] Nothing in the background is a straight line all the way across

## 2026-09-13

A rebinding that was right in general was wrong in one place: every other use of ASH wanted a visible grey and this one wanted an invisible one. Nothing measured it, because a contrast test checks that things are legible enough and never that something is too legible.
