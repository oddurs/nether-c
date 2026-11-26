---
id: 333
uid: f849b806-d29a-463a-aeed-7e2a4ba8e45a
title: Website visual craft, CSS fixes, and Necropolis continuity
type: feature
status: buried
created: 2026-11-26
updated: 2026-11-26
closed_at: 2026-11-26
priority: p1
area: site,web/necropolis
stratum: '0'
effort: m
proof: All CSS, contrast, font, link, and Chromium browser checks pass in both dark and light rooms across desktop and mobile viewports
---

## Problem

## Proposal

## Starting point and non-goals

Name existing implementation/tests; distinguish missing work from work already delivered.

## Delivery steps and dependencies

1. Resolve specification changes first, in their own PR.
2. Implement the smallest slice; name the paths and prerequisite items.
3. Verify success, failure and boundary behavior.

## Which stratum does this reach?

## Acceptance criteria

- [ ]

## Evidence to close

Set `proof` to the observable gate. Record the tested commit, exact checks and
remaining limits here. Human or elapsed-time evidence cannot be replaced by CI.

## 2026-11-26

Fixed light-mode 3D bevels, manual chapter hover contrast, nav bar compactness, 3/2/1 swatch grid, recessed --sunk code wells, heading line-height and clean # permalinks, table padding and zebra tint, reusable .btn/.pill/.release-head/.meta-bar/.chapter-pager/.copy-btn/.sr-only components, micro-defects, and Necropolis terminal header/nav/button/mobile continuity. Extended tests/contrast/run for all syntax and stratum colors on --sunk plus ground on --good. Verified via scripts/task check (CSS, contrast, font, links, site, gfx, wasm, and Chromium desktop/mobile browser checks) and visual inspection of dark/light screenshots.
