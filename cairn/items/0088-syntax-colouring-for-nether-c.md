---
id: 88
title: Syntax colouring for Nether C
type: feature
status: buried
milestone: lamp
created: 2026-09-10
updated: 2026-09-10
priority: p1
effort: m
area: site/bake
proof: Depth annotations are visually distinct from types in every sample on the site
---

Nether C is not any existing grammar, so nothing off the shelf knows it. A
small tokenizer inside site/bake.

Depth annotations are coloured by the stratum they name, on a ramp from sulphur
at 0 to lilac at 8 — depth is the language's whole point and should be the most
visible thing in a sample.
