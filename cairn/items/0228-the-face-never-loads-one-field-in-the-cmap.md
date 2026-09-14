---
id: 228
title: 'The face never loads: one field in the cmap'
type: bug
status: buried
milestone: necropolis
created: 2026-09-14
updated: 2026-09-14
priority: p0
effort: s
area: site/font.py
stratum: '0'
proof: document.fonts reports Nether 8 as loaded, body text measures at the face rather than the fallback, and tests/font/run fails if the field is put back
---

## Problem

`site/nether.woff` has never loaded in a browser. Every page on the site has
been rendering in `ui-monospace` — the system mono fallback — since the face
was first built.

    > [...document.fonts].map(f => f.family + ': ' + f.status)
    ["Nether 8: error"]

Nothing said so. A font that fails to load is indistinguishable from one
nobody asked for: no console error the page can see, no failed request, just
different letterforms. `scripts/task check` passed throughout, because the
font checks verify the glyphs and the outlines and never asked whether a
browser would take the file.

## Cause

One field. `cmap_table` in `site/font.py` gives the terminating segment an
`idDelta` of 0:

    deltas.append(0 if start == 0xFFFF else (gid - start) & 0xFFFF)

Format 4 requires a final segment covering U+FFFF, and `idDelta` there must be
1, so that `0xFFFF + 1` wraps to glyph 0 — `.notdef`. With a delta of 0, U+FFFF
maps to glyph 65535 in a font that has 137 glyphs. A browser sanitises a face
before using it, and a mapping past the end of the font fails that check. The
whole face is thrown out for it.

## How it was found

By elimination, because nothing reports it. Rebuilt as a bare sfnt to rule out
the WOFF container; round-tripped a system font through the same writer to
prove the writer was sound; then grafted each of our tables into that font one
at a time. `cmap` was the only one that poisoned it.

## Acceptance criteria

- [x] The terminator's `idDelta` is 1
- [x] `tests/font/run` walks the cmap the way a sanitiser does and fails on a
      mapping past the end of the font
- [x] Putting the old value back fails that check

## 2026-09-14

One field, and it had never worked. Every page on the site has rendered in ui-monospace since the face was first built.

cmap format 4 requires a final segment covering U+FFFF, and its idDelta must be 1 so that 0xFFFF + 1 wraps to glyph 0. font.py gave it 0, which maps U+FFFF to glyph 65535 in a font with 137 glyphs. A browser sanitises a face before using it and throws out the whole thing for a mapping past the end of the font.

Nothing reported it. There is no console error the page can see and no failed request — a face that fails to load looks exactly like one nobody asked for. scripts/task check passed the whole time, because the font tests verify the glyphs and the outlines and never asked whether a browser would take the file.

Found by elimination. Rebuilt it as a bare sfnt, which also failed, so the WOFF container was innocent. Round-tripped a system font through the same writer and it loaded, so the writer was sound. Then grafted our tables into that font one at a time: cmap was the only one that poisoned it. Then read the segment table and found U+FFFF mapping to 65535.

Two things I was wrong about on the way, both caught by testing rather than reasoning. The head table's stored checksum really is inconsistent — it is computed with checkSumAdjustment zeroed, which is right for an sfnt directory and wrong for WOFF's origChecksum — but patching it changed nothing, so it is a separate, harmless inconsistency and not this bug. And 102 of 137 glyphs appear to consume fewer bytes than loca allows, which is 4-byte alignment padding and legal.

tests/font/run now walks the cmap the way a sanitiser does. Reinstating the old value fails it.
