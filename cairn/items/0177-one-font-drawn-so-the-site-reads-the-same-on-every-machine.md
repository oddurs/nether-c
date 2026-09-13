---
id: 177
title: One font, drawn, so the site reads the same on every machine
type: feature
status: buried
milestone: necropolis
assignee: Oddur Sigurdsson
created: 2026-09-13
updated: 2026-09-13
priority: p1
effort: l
area: site
stratum: '0'
proof: The site renders identically on a machine with no fonts installed but the defaults
---

The site is hand-made everywhere except the one place a reader actually looks.

```css
font-family: "DejaVu Sans Mono", ui-monospace, SFMono-Regular, Menlo, …
```

That is whatever the visitor happens to have installed, so the site renders
differently on every machine. For a project whose thesis is that a trace means
the same thing on a machine that has never seen your filesystem, leaving the
letterforms to chance is the wrong thing to leave to chance. It is also the
only smooth, anonymous surface left on the page: everything else — the
palette, the banners, the rules — is drawn here.

TempleOS had **one 8x8 fixed-width font**, and that was the whole of it.

## What it needs

- `site/font.py` — a TrueType writer. Each lit pixel is a square contour;
  `head`, `hhea`, `hmtx`, `maxp`, `cmap`, `glyf`, `loca`, `name`, `post` and
  `OS/2`; then WOFF, which is a header and zlib per table, and `zlib` is
  stdlib. There is no font library in this repository and there will not be
  one.
- **150 glyphs**, typed out. 94 printable ASCII plus the 37 the specification
  actually uses — `§ « » × ƒ Γ δ κ λ τ — … ′ ℓ → ∈ − ∪ ≡ ≤ ≥ ⊕ ⊢ ─ └ █ ⟨ ⟩ ⟶`
  and the super- and subscripts — plus ①..⑳ for the transcripts. A font that
  falls back to the system for `⊢` looks worse than what is there now.
- A specimen sheet, rendered by `site/gfx.py`, so the glyphs can be looked at
  rather than trusted.
- A byte budget, as `.wasm-ceiling` is one, or the file grows quietly.

## What falls out of it

At an 8x8 cell, eighty columns is exactly **640 pixels**. The content column
stops being `80ch` of somebody else's font and becomes the covenant number.
The type scale becomes 8, 16, 24, 32 — no fractional `rem`, and at 16px on a
2x display every glyph lands on thirty-two device pixels.

## What it does not touch

The palette and its contrast proofs, the GIF banners and rules, the descent,
`site/bake`, or the committed-output discipline. Those work. The risk in a
redesign is redesigning the parts that already work.

## Acceptance criteria

- [x] The site loads one font, and it is in this repository
- [x] Every glyph the site renders is in it
- [x] A specimen sheet shows all of them
- [x] The content column is 640 pixels because the cell is 8
- [x] The file has a budget and the build enforces it

## 2026-09-13

135 glyphs, 2900 bytes. The face is exactly as large as it has to be: tests/font/run reads every codepoint the built site and spec/ render, checks it against the cmap of the shipped file, and reports how many the face carries that nothing uses. It says zero.

## 2026-09-13

The check found three glyphs the first survey missed -- middle dot, bullet and leftwards arrow -- because the survey read the HTML without unescaping entities and the check does not. That is the argument for having it.

## 2026-09-13

The advance is six columns, not eight. A square cell is right for a 640x480 screen and wrong for a page: a face whose advance equals its height sets eighty columns half again as wide as anything reads at. Ink is columns 0-5, the gap is 6-7, and the glyphs meant to tile -- the box rules, the em dash, the full block -- end at the advance so a rule drawn from them has no gap every six pixels.

## 2026-09-13

No byte ceiling, and that was deliberate. A third ceiling file to approximate what the coverage check already proves exactly is the kind of thing this repository calls admiring complexity.
