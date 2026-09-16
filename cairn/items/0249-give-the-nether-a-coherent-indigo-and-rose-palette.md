---
id: 249
title: Give the nether a coherent indigo and rose palette
type: feature
status: buried
milestone: face
assignee: Oddur Sigurdsson
created: 2026-09-15
updated: 2026-09-15
priority: p2
stratum: '0'
area: site/design.py, site/gfx.py
proof: Shared tokens drive both themes and GIF palettes; legibility tests cover semantic colours and all nine strata; responsive browser tests verify theme and motion changes
---

## Problem

## Proposal

## Which stratum does this reach?

## Acceptance criteria

- [ ]

## 2026-09-15

Authored separate dark indigo/rose and light porcelain/lavender/plum token tables. GIFs now ship both colour tables with identical geometry; the lamp selects the matching assets, including the type specimen. Rebuilt the nine-strata drawing as 33 smooth downward positions across numbered suspended shelves, slowed the hero, and supplied still hero/descent variants for reduced-motion preferences. Replaced stale VGA swatches and copy with live semantic roles; recorded costs in spec 90.5. Full scripts/task check passed. Real-browser tests cover 320/390/768/1248px, theme switches, still-image switches, rendered swatches and existing browser behavior. Dark/light page, palette and strata screenshots reviewed. The WOFF is unchanged.
