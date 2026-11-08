---
id: 289
uid: c3d5b36f-c29b-40cc-858c-92a1f0444d5d
title: Publish at netherc.org
type: docs
status: buried
milestone: assay
assignee: Oddur Sigurdsson
created: 2026-11-08
updated: 2026-11-08
closed_at: 2026-11-08
priority: p2
effort: s
area: site
proof: No page under site/ names oddurs.github.io, and the og:image every page names returns 200 from https://netherc.org
---

## What changes

The site moves from the GitHub Pages project address to netherc.org. The only
absolute link to the old address was the `og:image` in `site/template.html`,
which every baked page carries. Item 0003 also said where the site is published.

## Acceptance and evidence

- [x] `grep -r` for the old project address finds nothing under `site/`.
- [x] `https://netherc.org/gfx/card.png` returns 200 `image/png`.

## 2026-11-08

GitHub Pages serves netherc.org: DNS on Cloudflare (DNS only), domain verified, certificate issued, HTTPS enforced. The deploy is an Actions workflow, so the domain lives in the Pages settings and a CNAME file in site/ would be ignored. The og:image in site/template.html was the only absolute link to the old address.
