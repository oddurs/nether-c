---
id: 181
title: A vertical scrollbar in the header of every page
type: bug
status: buried
milestone: necropolis
assignee: Oddur Sigurdsson
created: 2026-09-13
updated: 2026-09-13
priority: p1
effort: s
area: site
stratum: '0'
proof: The sign has no scrollbar, and every horizontal scroll container names both axes
---

`pre.sign` is the block-letter sign at the top of every page. It carries a
scrollbar it does not need.

## Why

Two things, and the second is what makes the first visible.

`overflow-x: auto` does not leave `overflow-y` alone. CSS says that when one
axis is `visible` and the other is not, the `visible` one computes to `auto` —
so an element with a horizontal scroll container has a vertical one too, and
anything at all below the content box shows it.

And 0177 flattened `line-height: 1.05` to `1` while snapping every size in the
stylesheet to multiples of the cell. The `.05` was not decoration: the face's
ascent and descent are exactly one em, so at `line-height: 1` the line boxes
are exactly the content height and a fraction of a pixel of rounding is enough.

## What to do

Say `overflow-y: hidden` where the intent is a horizontal scroll container,
here and in `figure.listing pre` and `div.scroll`, which have the same shape
and the same latent bug. A rule that only holds because the arithmetic came
out even is a rule that breaks the next time somebody changes a number.

## Acceptance criteria

- [x] No element in the header scrolls vertically
- [x] Every horizontal scroll container says so on both axes

## 2026-09-13

The 1.05 line-height was load-bearing and looked like a stray decimal. 0177 flattened it to 1 while snapping every size to the cell, which is how a change that was right in general broke one thing in particular.

## 2026-09-13

overflow-y: hidden rather than more leading. Leading would change the spacing of block art that is meant to be solid, and the fix belongs on the property that is actually wrong.
