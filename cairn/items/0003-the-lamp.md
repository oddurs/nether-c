---
id: 3
key: lamp
title: The Lamp
type: milestone
status: buried
created: 2026-09-10
updated: 2026-09-10
priority: p2
due: 2026-10-31
---

## 2026-09-10

Buried. Raw HTML and one stylesheet, baked from spec/ by site/bake. Every graphic drawn by site/gfx.py — a GIF89a encoder, an LZW compressor, a PNG encoder and a 5x7 bitmap font, none of them imported, because a link preview is not worth a dependency. The palette is the VGA sixteen complemented; fourteen land back inside it and the two that do not are named ROT and BILE. Published at https://oddurs.github.io/nether-c/ from site/ as committed. Every colour pair clears WCAG AA on both grounds, enforced by tests/contrast/run in CI.
