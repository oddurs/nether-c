---
id: 332
uid: d71bbbdb-b4d1-4ced-9ba1-4199309b583a
title: Restore missing serifs, stems, and symmetry on bold Nether 8 glyphs
type: bug
status: buried
created: 2026-11-26
updated: 2026-11-26
closed_at: 2026-11-26
priority: p1
area: site
proof: Bold Nether 8 glyphs preserve symmetry, strokes, and distinct ASCII shapes in weigh() and tests/font/run, verified in Chromium
---

## What happens

## What should happen

## Reproduction

1.

## Cairn of the offending trace

## Fix boundary and regression proof

Name the smallest affected path, the failing test before the fix, and the
verification after it. Preserve unrelated behavior.

## 2026-11-26

Fixed bold Nether 8 glyphs whose leftward stem thickening sheared off serifs, corners, or stroke connections: restored symmetric serifs on I (previously identical in shape to '['), top-right pixel on l, continuous 2px left stem on R, centered 2px stems and connected hooks/bars on f, t, ƒ, τ, !, ', |, 2px upper tips on Y, stem-aligned ascender on ᵈ, and symmetric dot/stem on ᵢ. Added horizontal-symmetry preservation, 4-connected stroke preservation, and printable-ASCII shape-collision checks to weigh() in site/font.py (exercised by tests/font/run). Rebuilt site/nether-bold.woff (3304 bytes) and site/specimen.html, and visually verified 'If you want to work on it' on site/manual/09-where-to-go-from-here.html and the 8/16/24/32px scale rows on site/specimen.html in Chromium.
