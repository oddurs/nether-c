---
id: 178
title: 'Every glyph in the face is malformed: the long delta is never written'
type: bug
status: buried
milestone: necropolis
assignee: Oddur Sigurdsson
created: 2026-09-13
updated: 2026-09-13
priority: p0
effort: s
area: site
stratum: '0'
proof: The glyf table decodes back to the bitmap it was drawn from, for every glyph
---

`site/font.py` writes a `glyf` point stream that no reader can decode. 133 of
the 135 glyphs are affected, which is every one that has a run two pixels wide
or more.

## The defect

`outline()` writes a coordinate delta only when it fits the short form:

```python
if -255 <= dx <= 255:
    flag |= 0x02 | (0x10 if dx >= 0 else 0)
    dxs.append(abs(dx))
```

When it does not fit, no bytes are written — but the flag that is written says
a **two-byte signed delta follows**, because 0x02 clear and 0x10 clear is
TrueType's long form, not "nothing". The reader consumes bytes that are not
there and every point after it is wrong.

One pixel is 128 units, so a run of two pixels is 256 and already overflows.
Nothing in the face is one pixel wide.

```
T: 7 contours, 28 points
   the reader consumes 30 bytes of x and 29 of y = 59
   the encoder emitted 54, padding included
```

## Why nothing caught it

- `tests/font/run` reads the `cmap` and never the `glyf`.
- `fc-query` reads `name`, `OS/2` and `cmap`, and reports a font it never
  rasterises.
- The specimen is drawn by `site/gfx.py` from the same bitmaps the font is
  drawn from, so it is a picture of the *source* and says nothing about the
  encoder.

Three checks, none of which looks at the thing that was wrong.

## Acceptance criteria

- [x] A delta that does not fit the short form is written long
- [x] The point stream decodes: every flag's bytes are present, exactly
- [x] Something reads the `glyf` back and compares it to the bitmap it came from

## 2026-09-13

TrueType spells a delta three ways and the flag says which: SHORT set is one unsigned byte plus a sign bit; SHORT clear with the companion bit set means the same as the last point and no bytes; both clear means a two-byte signed delta follows. The encoder only ever wrote the first, and wrote the flag for the third without the bytes.

## 2026-09-13

Nothing caught it because all three existing checks look somewhere else. tests/font/run read the cmap; fc-query reads name, OS/2 and cmap and never rasterises; and the specimen is drawn by gfx.py from the same bitmaps the font is drawn from, so it is a picture of the source. The new check decodes the glyf out of the shipped file and compares every point with the rectangle the bitmap implies, and it fails on the old encoder with 'the point stream ends inside a short delta'.
