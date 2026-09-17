#!/usr/bin/env python3
"""
font — an 8x8 bitmap face, and the TrueType writer that ships it.

Two weights of one 8x8 fixed-width face, with their drawings and their file
format kept together. There is no font library here: the writer is small
enough to inspect in full.

The site used to name a stack of fonts it hoped the reader already had, which
meant it rendered differently on every machine. A project whose whole claim is
that a trace means the same thing on a machine that has never seen your
filesystem should not leave its letterforms to chance.

    python3 site/font.py            write both faces and the specimen
    python3 site/font.py --check    fail if what is committed is stale
    python3 site/font.py --sheet    print every glyph as ASCII, to look at
    python3 site/font.py --sheet 700   the same, for the bold

Everything it emits is committed. Nothing here downloads anything.
"""

from __future__ import annotations

import struct
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "site"

# ── the grid ────────────────────────────────────────────────────────────────
#
# Eight by eight, because that was the covenant. One em is the drawing cell;
# the advance is separate. The type scale is 8, 16, 24, 32.
#
# Rows 0..6 sit above the baseline and row 7 below it, which gives a seven-row
# capital and one row of descender — the same proportion the 5x7 face in
# gfx.py uses, one row taller.

CELL = 8
UPEM = 1024
PIXEL = UPEM // CELL  # 128 font units to the pixel

# Six drawing columns plus one blank bearing make the fixed cell. The bearing
# belongs to the metric, not to a destructive transform of the drawings.
INK_COLS = 6
ADVANCE_COLS = INK_COLS + 1
ADVANCE = ADVANCE_COLS * PIXEL  # 896
DESCENT_ROWS = 1
ASCENT = (CELL - DESCENT_ROWS) * PIXEL  # 896
DESCENT = DESCENT_ROWS * PIXEL  # 128

NAME = "Nether 8"
VERSION = "Version 1.000"


# ── the outline of one glyph ────────────────────────────────────────────────


def runs(rows: tuple[str, ...]) -> list[tuple[int, int, int]]:
    """Every lit pixel, merged along each row into (row, first, last) runs.

    Merging matters: a glyph is up to sixty-four squares and a run turns a
    whole row into one. It halves the file and costs four lines.
    """
    out: list[tuple[int, int, int]] = []
    for r, bits in enumerate(rows):
        c = 0
        while c < len(bits):
            if bits[c] != "#":
                c += 1
                continue
            start = c
            while c < len(bits) and bits[c] == "#":
                c += 1
            out.append((r, start, c - 1))
    return out


def outline(rows: tuple[str, ...]) -> bytes:
    """One `glyf` entry: a simple glyph, one closed contour per run.

    Every point is on-curve, so there are no control points and no curves.
    Contours wind clockwise with y upwards, which is what TrueType asks of an
    outer contour.
    """
    boxes = runs(rows)
    if not boxes:
        return b""  # an empty glyph is zero bytes, and `loca` says so

    xs: list[int] = []
    ys: list[int] = []
    ends: list[int] = []
    for r, first, last in boxes:
        x0, x1 = first * PIXEL, (last + 1) * PIXEL
        y0 = ASCENT - (r + 1) * PIXEL
        y1 = y0 + PIXEL
        xs += [x0, x0, x1, x1]
        ys += [y0, y1, y1, y0]
        ends.append(len(xs) - 1)

    out = struct.pack(">hhhhh", len(boxes), min(xs), min(ys), max(xs), max(ys))
    out += struct.pack(f">{len(ends)}H", *ends)
    out += struct.pack(">H", 0)  # no instructions: there is no hinting here

    # One flag byte per point, and the deltas in two streams after them.
    #
    # TrueType spells a delta three ways and the flag says which, so the flag
    # and the bytes have to agree exactly: SHORT set is one unsigned byte with
    # the companion bit for its sign; SHORT clear with the companion bit SET
    # means "the same as the last one" and no bytes at all; SHORT clear and
    # the companion clear means a two-byte signed delta follows.
    #
    # The third case is the one that matters here. A pixel is 128 units, so a
    # run two pixels wide is 256 and does not fit a byte — and writing the
    # flag for a long delta without writing the delta desynchronises every
    # point after it. 0178.
    flags = bytearray()
    dxs = bytearray()
    dys = bytearray()
    px = py = 0
    for x, y in zip(xs, ys):
        flag = 0x01  # on-curve
        for delta, short, same, stream in (
            (x - px, 0x02, 0x10, dxs),
            (y - py, 0x04, 0x20, dys),
        ):
            if delta == 0:
                flag |= same
            elif -255 <= delta <= 255:
                flag |= short | (same if delta > 0 else 0)
                stream.append(abs(delta))
            else:
                stream += struct.pack(">h", delta)
        flags.append(flag)
        px, py = x, y

    out += bytes(flags) + bytes(dxs) + bytes(dys)
    return out + b"\x00" * (-len(out) % 4)


# ── the tables ──────────────────────────────────────────────────────────────


def pad(data: bytes) -> bytes:
    return data + b"\x00" * (-len(data) % 4)


def checksum(data: bytes) -> int:
    data = pad(data)
    total = 0
    for (word,) in struct.iter_unpack(">I", data):
        total = (total + word) & 0xFFFFFFFF
    return total


def name_table(records: list[tuple[int, str]]) -> bytes:
    """`name`, in Windows/Unicode BMP, which is the one every reader wants."""
    strings = b""
    entries = b""
    for name_id, text in records:
        encoded = text.encode("utf-16-be")
        entries += struct.pack(">HHHHHH", 3, 1, 0x0409, name_id, len(encoded), len(strings))
        strings += encoded
    head = struct.pack(">HHH", 0, len(records), 6 + 12 * len(records))
    return head + entries + strings


def cmap_table(codes: list[int]) -> bytes:
    """`cmap` format 4. Glyph ids run in codepoint order, so a run of
    consecutive codepoints is a run of consecutive glyphs and one segment."""
    segments: list[tuple[int, int, int]] = []  # start, end, first glyph id
    for i, code in enumerate(codes):
        gid = i + 1  # glyph 0 is .notdef
        if segments and code == segments[-1][1] + 1:
            segments[-1] = (segments[-1][0], code, segments[-1][2])
        else:
            segments.append((code, code, gid))
    segments.append((0xFFFF, 0xFFFF, 0))

    count = len(segments)
    entry = max(0, count.bit_length() - 1)
    sub = struct.pack(">HHHHHHH", 4, 16 + 8 * count, 0, count * 2, 2 ** entry * 2, entry,
                      count * 2 - 2 ** entry * 2)
    sub += struct.pack(f">{count}H", *[e for _, e, _ in segments])
    sub += struct.pack(">H", 0)  # reservedPad
    sub += struct.pack(f">{count}H", *[s for s, _, _ in segments])
    deltas = []
    for start, _, gid in segments:
        # The terminator has to map U+FFFF to .notdef, and a delta of 1 is how:
        # 0xFFFF + 1 wraps to glyph 0. A delta of 0 maps it to glyph 65535,
        # which a 137-glyph font does not have, and a browser's sanitiser
        # throws out the whole face for it. Silently -- a face that fails to
        # load is indistinguishable from one nobody asked for.
        deltas.append(1 if start == 0xFFFF else (gid - start) & 0xFFFF)
    sub += struct.pack(f">{count}h", *[d - 0x10000 if d > 0x7FFF else d for d in deltas])
    sub += struct.pack(f">{count}H", *([0] * count))  # idRangeOffset: all delta

    return struct.pack(">HHHHI", 0, 1, 3, 1, 12) + sub


def build(glyphs: dict[str, tuple[str, ...]], weight: int = 400) -> bytes:
    """Every table, in one sfnt.

    The weight reaches four fields and no outline. A face whose drawings say
    one thing and whose `OS/2` says another is a face a browser will pick for
    the wrong text and then synthesise the difference on top of.
    """
    bold = weight >= 700
    style = "Bold" if bold else "Regular"
    codes = sorted(ord(c) for c in glyphs)
    order = [None] + [chr(c) for c in codes]  # glyph 0 is .notdef

    data = b""
    loca = [0]
    bearings = []
    extents = []
    for ch in order:
        glyph = outline(glyphs[ch]) if ch is not None else b""
        # Preserve each drawing's inset. A zero bearing would shift narrow
        # punctuation left even though the outline itself is correctly drawn.
        bearings.append(struct.unpack(">h", glyph[2:4])[0] if glyph else 0)
        extents.append(struct.unpack(">h", glyph[6:8])[0] if glyph else 0)
        data += glyph
        loca.append(len(data))

    # `loca` is short format when every offset is even and fits in a u16 when
    # halved, which for a font this size it is.
    short = loca[-1] <= 0x1FFFE and all(o % 2 == 0 for o in loca)
    loca_table = (struct.pack(f">{len(loca)}H", *[o // 2 for o in loca]) if short
                  else struct.pack(f">{len(loca)}I", *loca))

    count = len(order)
    head = struct.pack(
        ">IIIIHHQQhhhhHHhhh",
        0x00010000, 0x00010000, 0, 0x5F0F3CF5,
        0b0000_0000_0000_1011,  # baseline at y=0, lsb equals xMin, integer ppem
        UPEM, 0, 0,             # created, modified: zero, so the file is reproducible
        0, -DESCENT, max(extents), ASCENT,
        1 if bold else 0, 8, 2, 0 if short else 1, 0,
    )
    hhea = struct.pack(">IhhhHhhhhhhhhhhhH", 0x00010000, ASCENT, -DESCENT, 0,
                       ADVANCE, min(bearings), ADVANCE - max(extents),
                       max(extents), 1, 0, 0, 0, 0, 0, 0, 0, count)
    maxp = struct.pack(">IHHHHHHHHHHHHHH", 0x00010000, count, 4 * CELL * CELL,
                       CELL * CELL, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0)
    hmtx = b"".join(struct.pack(">Hh", ADVANCE, bearing) for bearing in bearings)
    # `post` version 3.0: no glyph names. isFixedPitch is 1, because it is.
    post = struct.pack(">IIhhIIIII", 0x00030000, 0, -PIXEL, PIXEL, 1, 0, 0, 0, 0)

    # `OS/2` version 4, ninety-six bytes, written a field at a time. One long
    # format string was wrong by one value and said so in a way nobody could
    # read.
    os2 = struct.pack(">HhHHH", 4, ADVANCE, weight, 5, 0)       # version..fsType
    os2 += struct.pack(">10h",                                # sub, super, strikeout
                       UPEM // 2, UPEM // 2, 0, 0,
                       UPEM // 2, UPEM // 2, 0, ASCENT // 2,
                       PIXEL, 3 * PIXEL)
    os2 += struct.pack(">h", 8)                               # sFamilyClass: monospace
    os2 += bytes([2, 0, 5, 9, 0, 0, 0, 0, 0, 0])              # PANOSE: modern, monospaced
    os2 += struct.pack(">4I", 0b11, 0, 0, 0)                  # Latin-1 and Latin Ext
    os2 += b"NETH"                                            # achVendID
    # fsSelection: bit 5 is bold and bit 6 is regular, and they are exclusive.
    os2 += struct.pack(">HHH", 0b100000 if bold else 0b1000000, min(codes), max(codes))
    os2 += struct.pack(">hhh", ASCENT, -DESCENT, 0)           # sTypo*
    os2 += struct.pack(">HH", ASCENT, DESCENT)                # usWin*
    os2 += struct.pack(">II", 1, 0)                           # ulCodePageRange: Latin-1
    os2 += struct.pack(">hh", 5 * PIXEL, 7 * PIXEL)           # sxHeight, sCapHeight
    os2 += struct.pack(">HHH", 0x20, 0x20, 1)                 # default, break, maxContext
    if len(os2) != 96:
        raise SystemExit(f"font: OS/2 is {len(os2)} bytes and should be 96")

    tables = {
        b"OS/2": os2,
        b"cmap": cmap_table(codes),
        b"glyf": data,
        b"head": head,
        b"hhea": hhea,
        b"hmtx": hmtx,
        b"loca": loca_table,
        b"maxp": maxp,
        b"name": name_table([
            (1, NAME), (2, style), (3, f"{NAME} {style} {VERSION}"),
            (4, NAME if bold is False else f"{NAME} {style}"), (5, VERSION),
            (6, f"{NAME.replace(' ', '')}-{style}"),
        ]),
        b"post": post,
    }
    return sfnt(tables)


def sfnt(tables: dict[bytes, bytes]) -> bytes:
    """The directory, the tables, and the one checksum that covers the file."""
    count = len(tables)
    entry = max(0, count.bit_length() - 1)
    directory = struct.pack(">IHHHH", 0x00010000, count, 2 ** entry * 16, entry,
                            count * 16 - 2 ** entry * 16)

    offset = 12 + 16 * count
    body = b""
    for tag in sorted(tables):
        raw = tables[tag]
        directory += struct.pack(">4sIII", tag, checksum(raw), offset, len(raw))
        body += pad(raw)
        offset += len(pad(raw))

    font = directory + body
    # head.checkSumAdjustment, which is the whole file's checksum subtracted
    # from a magic number. It is written last because it covers itself.
    at = font.index(b"head")
    head_at = struct.unpack(">I", font[at + 8:at + 12])[0]
    adjust = (0xB1B0AFBA - checksum(font)) & 0xFFFFFFFF
    return font[:head_at + 8] + struct.pack(">I", adjust) + font[head_at + 12:]


def woff(font: bytes) -> bytes:
    """WOFF 1.0: the same tables, each one zlib'd. Forty lines, no library."""
    count = struct.unpack(">H", font[4:6])[0]
    entries = []
    for i in range(count):
        at = 12 + 16 * i
        tag, csum, offset, length = struct.unpack(">4sIII", font[at:at + 16])
        raw = font[offset:offset + length]
        squeezed = zlib.compress(raw, 9)
        if len(squeezed) >= len(raw):
            squeezed = raw  # WOFF says equal lengths mean stored, not deflated
        entries.append((tag, csum, raw, squeezed))

    offset = 44 + 20 * count
    directory = b""
    body = b""
    for tag, csum, raw, squeezed in entries:
        directory += struct.pack(">4sIIII", tag, offset, len(squeezed), len(raw), csum)
        body += pad(squeezed)
        offset += len(pad(squeezed))

    head = struct.pack(
        ">4sIIHHIHHIIIII",
        b"wOFF", 0x00010000, 44 + 20 * count + len(body), count, 0,
        len(font), 1, 0, 0, 0, 0, 0, 0,
    )
    return head + directory + body


# ── looking at it ───────────────────────────────────────────────────────────


def sheet(glyphs: dict[str, tuple[str, ...]]) -> str:
    """Every glyph, eight across, as text. A font nobody has looked at is a
    font with a broken glyph in it."""
    out = []
    items = sorted(glyphs.items(), key=lambda kv: ord(kv[0]))
    for i in range(0, len(items), 8):
        block = items[i:i + 8]
        out.append("  ".join(f"U+{ord(c):04X} {c}  " for c, _ in block))
        for row in range(CELL):
            out.append("  ".join(
                rows[row].replace("#", "█").replace(".", "·") + "  "
                for _, rows in block
            ))
        out.append("")
    return "\n".join(out)


def specimen(glyphs: dict[str, tuple[str, ...]], mode: str = "nether"):
    """The specimen, drawn with the face itself, as a GIF for the site."""
    sys.path.insert(0, str(ROOT / "site"))
    import gfx  # noqa: E402  — a sibling, not a dependency

    items = sorted(glyphs.items(), key=lambda kv: ord(kv[0]))
    across, scale, gap = 16, 3, 1
    cols = across * (CELL + gap) + gap
    rowsn = (len(items) + across - 1) // across
    frame = gfx.Frame(cols * scale, (rowsn * (CELL + gap) + gap) * scale, 0)
    for i, (_, rows) in enumerate(items):
        ox = (gap + (i % across) * (CELL + gap)) * scale
        oy = (gap + (i // across) * (CELL + gap)) * scale
        colour = gfx.DEPTH[(i // across) % 9]
        for r, bits in enumerate(rows):
            for c, on in enumerate(bits):
                if on == "#":
                    for dy in range(scale):
                        for dx in range(scale):
                            frame.set(ox + c * scale + dx, oy + r * scale + dy, colour)
    return gfx.write_gif(OUT / "gfx" / "specimen.gif", [frame], [0], loop=False, mode=mode)


# ── the face ────────────────────────────────────────────────────────────────
#
# Six columns of ink, one column of bearing, and seven rows tall in an eight by
# eight drawing cell: the same proportion as the 5x7 face in gfx.py, one
# column and one row larger. Capitals and ascenders fill rows 0 to 6, the
# x-height is rows 2 to 6, and descenders drop into row 7. Strokes are one
# pixel, because at sixteen pixels a cell is doubled and two would be a slab.

G: dict[str, tuple[str, ...]] = {}


def g(ch: str, *rows: str) -> None:
    """One glyph. Eight rows of eight, and it says so if they are not."""
    if len(rows) != CELL or any(len(r) != CELL for r in rows):
        raise SystemExit(f"font: {ch!r} is not {CELL} rows of {CELL}")
    if any(set(r) - {"#", "."} for r in rows):
        raise SystemExit(f"font: {ch!r} has something that is not # or .")
    G[ch] = rows


# ── space and punctuation ───────────────────────────────────────────────────

g(" ", "........", "........", "........", "........",
        "........", "........", "........", "........")
g("!", "..#.....", "..#.....", "..#.....", "..#.....",
        "..#.....", "........", "..#.....", "........")
g('"', ".#.#....", ".#.#....", "........", "........",
        "........", "........", "........", "........")
g("#", ".#.#....", ".#.#....", "######..", ".#.#....",
        "######..", ".#.#....", ".#.#....", "........")
g("$", "..#.....", ".#####..", "#.#.....", ".####...",
        "..#..#..", "#####...", "..#.....", "........")
g("%", "##...#..", "##..#...", "...#....", "..#.....",
        ".#......", "#..##...", "...##...", "........")
g("&", ".##.....", "#..#....", "#..#....", ".##.....",
        "##..#...", "#..##...", ".##..#..", "........")
g("'", "..#.....", "..#.....", "........", "........",
        "........", "........", "........", "........")
g("(", "...#....", "..#.....", ".#......", ".#......",
        ".#......", "..#.....", "...#....", "........")
g(")", ".#......", "..#.....", "...#....", "...#....",
        "...#....", "..#.....", ".#......", "........")
g("*", "........", "#.#.#...", ".###....", "#####...",
        ".###....", "#.#.#...", "........", "........")
g("+", "........", "..#.....", "..#.....", "#####...",
        "..#.....", "..#.....", "........", "........")
g(",", "........", "........", "........", "........",
        "........", "..##....", "..##....", "..#.....")
g("-", "........", "........", "........", "#####...",
        "........", "........", "........", "........")
g(".", "........", "........", "........", "........",
        "........", "..##....", "..##....", "........")
g("/", ".....#..", "....#...", "...#....", "..#.....",
        ".#......", "#.......", "........", "........")

# ── digits ──────────────────────────────────────────────────────────────────

g("0", ".####...", "#....#..", "#...##..", "#.#..#..",
        "##...#..", "#....#..", ".####...", "........")
g("1", "..#.....", ".##.....", "..#.....", "..#.....",
        "..#.....", "..#.....", ".###....", "........")
g("2", ".####...", "#....#..", ".....#..", "...##...",
        "..#.....", ".#......", "######..", "........")
g("3", ".####...", "#....#..", ".....#..", "..###...",
        ".....#..", "#....#..", ".####...", "........")
g("4", "....#...", "...##...", "..#.#...", ".#..#...",
        "######..", "....#...", "....#...", "........")
g("5", "######..", "#.......", "#####...", ".....#..",
        ".....#..", "#....#..", ".####...", "........")
g("6", "..###...", ".#......", "#.......", "#####...",
        "#....#..", "#....#..", ".####...", "........")
g("7", "######..", ".....#..", "....#...", "...#....",
        "..#.....", "..#.....", "..#.....", "........")
g("8", ".####...", "#....#..", "#....#..", ".####...",
        "#....#..", "#....#..", ".####...", "........")
g("9", ".####...", "#....#..", "#....#..", ".#####..",
        ".....#..", "....#...", ".###....", "........")
g(":", "........", "..##....", "..##....", "........",
        "..##....", "..##....", "........", "........")
g(";", "........", "..##....", "..##....", "........",
        "..##....", "..##....", "..#.....", "........")
g("<", "....#...", "...#....", "..#.....", ".#......",
        "..#.....", "...#....", "....#...", "........")
g("=", "........", "........", "#####...", "........",
        "#####...", "........", "........", "........")
g(">", ".#......", "..#.....", "...#....", "....#...",
        "...#....", "..#.....", ".#......", "........")
g("?", ".####...", "#....#..", ".....#..", "...##...",
        "..#.....", "........", "..#.....", "........")
g("@", ".####...", "#....#..", "#.###...", "#.#.#...",
        "#.####..", "#.......", ".####...", "........")

# ── capitals ────────────────────────────────────────────────────────────────

g("A", "..##....", ".#..#...", "#....#..", "#....#..",
        "######..", "#....#..", "#....#..", "........")
g("B", "#####...", "#....#..", "#....#..", "#####...",
        "#....#..", "#....#..", "#####...", "........")
g("C", ".####...", "#....#..", "#.......", "#.......",
        "#.......", "#....#..", ".####...", "........")
g("D", "####....", "#...#...", "#....#..", "#....#..",
        "#....#..", "#...#...", "####....", "........")
g("E", "######..", "#.......", "#.......", "#####...",
        "#.......", "#.......", "######..", "........")
g("F", "######..", "#.......", "#.......", "#####...",
        "#.......", "#.......", "#.......", "........")
g("G", ".####...", "#....#..", "#.......", "#..###..",
        "#....#..", "#....#..", ".#####..", "........")
g("H", "#....#..", "#....#..", "#....#..", "######..",
        "#....#..", "#....#..", "#....#..", "........")
g("I", ".###....", "..#.....", "..#.....", "..#.....",
        "..#.....", "..#.....", ".###....", "........")
g("J", "...###..", ".....#..", ".....#..", ".....#..",
        ".....#..", "#....#..", ".####...", "........")
g("K", "#....#..", "#...#...", "#..#....", "###.....",
        "#..#....", "#...#...", "#....#..", "........")
g("L", "#.......", "#.......", "#.......", "#.......",
        "#.......", "#.......", "######..", "........")
g("M", "#....#..", "##..##..", "#.##.#..", "#.##.#..",
        "#....#..", "#....#..", "#....#..", "........")
g("N", "#....#..", "##...#..", "#.#..#..", "#..#.#..",
        "#...##..", "#....#..", "#....#..", "........")
g("O", ".####...", "#....#..", "#....#..", "#....#..",
        "#....#..", "#....#..", ".####...", "........")
g("P", "#####...", "#....#..", "#....#..", "#####...",
        "#.......", "#.......", "#.......", "........")
g("Q", ".####...", "#....#..", "#....#..", "#....#..",
        "#.#..#..", "#..#.#..", ".##..#..", "........")
g("R", "#####...", "#....#..", "#....#..", "#####...",
        "#..#....", "#...#...", "#....#..", "........")
g("S", ".#####..", "#.......", "#.......", ".####...",
        ".....#..", ".....#..", "#####...", "........")
g("T", "######..", "..#.....", "..#.....", "..#.....",
        "..#.....", "..#.....", "..#.....", "........")
g("U", "#....#..", "#....#..", "#....#..", "#....#..",
        "#....#..", "#....#..", ".####...", "........")
g("V", "#....#..", "#....#..", "#....#..", "#....#..",
        ".#..#...", ".#..#...", "..##....", "........")
g("W", "#....#..", "#....#..", "#....#..", "#.##.#..",
        "#.##.#..", "##..##..", "#....#..", "........")
g("X", "#....#..", ".#..#...", "..##....", "..##....",
        "..##....", ".#..#...", "#....#..", "........")
g("Y", "#....#..", ".#..#...", "..##....", "..#.....",
        "..#.....", "..#.....", "..#.....", "........")
g("Z", "######..", ".....#..", "....#...", "..##....",
        ".#......", "#.......", "######..", "........")
g("[", "..###...", "..#.....", "..#.....", "..#.....",
        "..#.....", "..#.....", "..###...", "........")
g("\\", "#.......", ".#......", "..#.....", "...#....",
         "....#...", ".....#..", "........", "........")
g("]", "..###...", "....#...", "....#...", "....#...",
        "....#...", "....#...", "..###...", "........")
g("^", "..#.....", ".#.#....", "#...#...", "........",
        "........", "........", "........", "........")
g("_", "........", "........", "........", "........",
        "........", "........", "........", "######..")
g("`", ".#......", "..#.....", "........", "........",
        "........", "........", "........", "........")

# ── lowercase ───────────────────────────────────────────────────────────────

g("a", "........", "........", ".####...", ".....#..",
        ".#####..", "#....#..", ".#####..", "........")
g("b", "#.......", "#.......", "#####...", "#....#..",
        "#....#..", "#....#..", "#####...", "........")
g("c", "........", "........", ".#####..", "#.......",
        "#.......", "#.......", ".#####..", "........")
g("d", ".....#..", ".....#..", ".#####..", "#....#..",
        "#....#..", "#....#..", ".#####..", "........")
g("e", "........", "........", ".####...", "#....#..",
        "######..", "#.......", ".#####..", "........")
g("f", "...##...", "..#.....", "#####...", "..#.....",
        "..#.....", "..#.....", "..#.....", "........")
g("g", "........", "........", ".#####..", "#....#..",
        "#....#..", ".#####..", ".....#..", ".####...")
g("h", "#.......", "#.......", "#####...", "#....#..",
        "#....#..", "#....#..", "#....#..", "........")
g("i", "..#.....", "........", ".##.....", "..#.....",
        "..#.....", "..#.....", ".###....", "........")
g("j", "....#...", "........", "...##...", "....#...",
        "....#...", "....#...", "#...#...", ".###....")
g("k", "#.......", "#.......", "#...#...", "#..#....",
        "###.....", "#..#....", "#...#...", "........")
g("l", ".##.....", "..#.....", "..#.....", "..#.....",
        "..#.....", "..#.....", ".###....", "........")
g("m", "........", "........", "##.##...", "#.#.#...",
        "#.#.#...", "#.#.#...", "#.#.#...", "........")
g("n", "........", "........", "#####...", "#....#..",
        "#....#..", "#....#..", "#....#..", "........")
g("o", "........", "........", ".####...", "#....#..",
        "#....#..", "#....#..", ".####...", "........")
g("p", "........", "........", "#####...", "#....#..",
        "#....#..", "#####...", "#.......", "#.......")
g("q", "........", "........", ".#####..", "#....#..",
        "#....#..", ".#####..", ".....#..", ".....#..")
g("r", "........", "........", "#.###...", "##......",
        "#.......", "#.......", "#.......", "........")
g("s", "........", "........", ".#####..", "#.......",
        ".####...", ".....#..", "#####...", "........")
g("t", "..#.....", "..#.....", "#####...", "..#.....",
        "..#.....", "..#.#...", "...#....", "........")
g("u", "........", "........", "#....#..", "#....#..",
        "#....#..", "#....#..", ".#####..", "........")
g("v", "........", "........", "#....#..", "#....#..",
        ".#..#...", ".#..#...", "..##....", "........")
g("w", "........", "........", "#....#..", "#.##.#..",
        "#.##.#..", "#.##.#..", ".#..#...", "........")
g("x", "........", "........", "#....#..", ".#..#...",
        "..##....", ".#..#...", "#....#..", "........")
g("y", "........", "........", "#....#..", "#....#..",
        "#....#..", ".#####..", ".....#..", ".####...")
g("z", "........", "........", "######..", "....#...",
        "..##....", ".#......", "######..", "........")
g("{", "...##...", "..#.....", "..#.....", ".#......",
        "..#.....", "..#.....", "...##...", "........")
g("|", "..#.....", "..#.....", "..#.....", "..#.....",
        "..#.....", "..#.....", "..#.....", "........")
g("}", "..##....", "....#...", "....#...", ".....#..",
        "....#...", "....#...", "..##....", "........")
g("~", "........", "........", "........", ".##..#..",
        "#..##...", "........", "........", "........")


# ── everything else the specification actually renders ──────────────────────
#
# Not a guess at what might be wanted: tests/font/run reads every codepoint
# the built site and spec/ put on a page and fails if one is missing. So this
# list is exactly as long as it has to be, and the day it is too short the
# build says which glyph to draw.

g("§", ".###....", "#...#...", ".##.....", "#..#....",
             "..##....", "#...#...", ".###....", "........")
g("·", "........", "........", "........", "..##....",
             "..##....", "........", "........", "........")
g("•", "........", "........", "..##....", ".####...",
             ".####...", "..##....", "........", "........")
g("←", "........", "........", "........", ".#......",
             "######..", ".#......", "........", "........")
g("«", "........", "........", "..#..#..", ".#..#...",
             "#..#....", ".#..#...", "..#..#..", "........")
g("»", "........", "........", "#..#....", ".#..#...",
             "..#..#..", ".#..#...", "#..#....", "........")
g("³", "##......", ".##.....", "##......", "........",
             "........", "........", "........", "........")
g("×", "........", "........", "#...#...", ".#.#....",
             "..#.....", ".#.#....", "#...#...", "........")
g("ƒ", "...##...", "..#.....", "#####...", "..#.....",
             "..#.....", "..#.....", "#.#.....", ".#......")
g("Γ", "######..", "#.......", "#.......", "#.......",
             "#.......", "#.......", "#.......", "........")
g("δ", ".####...", "#...#...", "..##....", ".#..#...",
             "#....#..", "#....#..", ".####...", "........")
g("κ", "........", "........", "#...#...", "#..#....",
             "###.....", "#..#....", "#...#...", "........")
g("λ", "##......", ".#......", ".#......", ".##.....",
             "#..#....", "#...#...", "#....#..", "........")
g("τ", "........", "........", "#####...", "..#.....",
             "..#.....", "..#.....", "...##...", "........")
g("ᵈ", "..#.....", ".###....", "#.##....", ".###....",
             "........", "........", "........", "........")
g("ᵢ", "........", "........", "........", "........",
             ".#......", "........", ".#......", ".#......")
g("–", "........", "........", "........", "........",
             ".####...", "........", "........", "........")
g("—", "........", "........", "........", "........",
             "######..", "........", "........", "........")
g("…", "........", "........", "........", "........",
             "........", "........", "#.#.#...", "........")
g("′", "..#.....", ".#......", "........", "........",
             "........", "........", "........", "........")
g("⁶", ".##.....", "#.......", "###.....", ".##.....",
             "........", "........", "........", "........")
g("₁", "........", "........", "........", "........",
             ".#......", "##......", ".#......", "###.....")
g("₂", "........", "........", "........", "........",
             "##......", "..#.....", ".#......", "###.....")
g("ₙ", "........", "........", "........", "........",
             "........", "###.....", "#.#.....", "#.#.....")
g("ℓ", "..##....", ".#..#...", ".#..#...", ".##.....",
             ".#......", ".#......", "..##....", "........")
g("→", "........", "........", "........", "....#...",
             "######..", "....#...", "........", "........")
g("∈", "........", "........", "..####..", ".#......",
             ".####...", ".#......", "..####..", "........")
g("−", "........", "........", "........", "........",
             "######..", "........", "........", "........")
g("∪", "........", "........", "#....#..", "#....#..",
             "#....#..", "#....#..", ".####...", "........")
g("≡", "........", "........", "######..", "........",
             "######..", "........", "######..", "........")
g("≤", "........", "...##...", ".##.....", "#.......",
             ".##.....", "...##...", "######..", "........")
g("≥", "........", "##......", "..##....", "....##..",
             "..##....", "##......", "######..", "........")
g("⊆", "........", "..####..", ".#......", ".#......",
             ".#......", "..####..", "######..", "........")
g("⊕", "........", ".####...", "#..#.#..", "#.###...",
             "#..#.#..", ".####...", "........", "........")
g("⊢", "#.......", "#.......", "#.......", "#####...",
             "#.......", "#.......", "#.......", "........")
g("①", "........", ".####...", "#..#.#..", "#..#.#..",
             "#..#.#..", ".####...", "........", "........")
g("─", "........", "........", "........", "........",
             "######..", "........", "........", "........")
g("■", "........", "######..", "######..", "######..",
             "######..", "######..", "######..", "........")
g("▸", "........", "#.......", ".##.....", "...##...",
             ".##.....", "#.......", "........", "........")
g("└", "..#.....", "..#.....", "..#.....", "..#.....",
             "..####..", "........", "........", "........")
g("█", "######..", "######..", "######..", "######..",
             "######..", "######..", "######..", "######..")
g("⟨", "....#...", "...#....", "..#.....", ".#......",
             "..#.....", "...#....", "....#...", "........")
g("⟩", ".#......", "..#.....", "...#....", "....#...",
             "...#....", "..#.....", ".#......", "........")
g("⟶", "........", "........", "........", "....#...",
             "######..", "....#...", "........", "........")


# ── the bold ────────────────────────────────────────────────────────────────
#
# A second set of drawings and not a transform of the first. Where a stem is
# one pixel it becomes two, drawn inward so the seven-column advance does not
# move; where the second pixel would close a counter or fill the gap between
# two strokes, the stroke keeps the weight it had. That is why `M` and `W`
# are heavier at the stems and not at the vertex, and it is what a browser's
# synthetic bold cannot do: it smears every glyph sideways and `M` comes out
# of it a filled square.
#
# Twenty-two glyphs are the same drawing at both weights. Blocks and box
# drawing rule listings and headings that have to join whatever the text
# around them weighs, and the rest have no room in six columns for a second
# pixel anywhere.

SAME = frozenset("*^_`m~«»×…′ₙ≡⊕①─└█■▸⟨⟩")

B: dict[str, tuple[str, ...]] = {}


def b(ch: str, *rows: str) -> None:
    """One bold glyph, checked the way `g` checks a regular one."""
    if len(rows) != CELL or any(len(r) != CELL for r in rows):
        raise SystemExit(f"font: bold {ch!r} is not {CELL} rows of {CELL}")
    if any(set(r) - {"#", "."} for r in rows):
        raise SystemExit(f"font: bold {ch!r} has something that is not # or .")
    B[ch] = rows


# ── space and punctuation, bold ───────────────────────────────────────────────

b(" ", "........", "........", "........", "........",
        "........", "........", "........", "........")
b("!", ".##.....", ".##.....", ".##.....", ".##.....",
        ".##.....", "........", ".##.....", "........")
b('"', ".##.##..", ".##.##..", "........", "........",
        "........", "........", "........", "........")
b("#", ".##.##..", ".##.##..", "######..", ".##.##..",
        "######..", ".##.##..", ".##.##..", "........")
b("$", "..##....", ".#####..", "#.##....", ".####...",
        "..##.#..", "#####...", "..##....", "........")
b("%", "###..#..", "###.#...", "...#....", "..#.....",
        ".#......", "#.###...", "..###...", "........")
b("&", ".##.....", "#..#....", "#..#....", ".##.....",
        "##..#...", "##.##...", ".##..#..", "........")
b("'", ".##.....", ".##.....", "........", "........",
        "........", "........", "........", "........")
b("(", "...#....", "..#.....", ".##.....", ".##.....",
        ".##.....", "..#.....", "...#....", "........")
b(")", ".#......", "..#.....", "..##....", "..##....",
        "..##....", "..#.....", ".#......", "........")
b("+", "........", "..##....", "..##....", "######..",
        "..##....", "..##....", "........", "........")
b(",", "........", "........", "........", "........",
        "........", "..##....", "..##....", "..##....")
b("-", "........", "........", "#####...", "#####...",
        "........", "........", "........", "........")
b(".", "........", "........", "........", "........",
        "........", ".###....", ".###....", "........")
b("/", "....##..", "...##...", "..##....", ".##.....",
        "##......", "##......", "........", "........")

# ── digits, bold ──────────────────────────────────────────────────────────────

b("0", "######..", "##..##..", "##..##..", "##..##..",
        "##..##..", "##..##..", "######..", "........")
b("1", "..##....", ".###....", "..##....", "..##....",
        "..##....", "..##....", ".####...", "........")
b("2", ".####...", "#...##..", "....##..", "...##...",
        "..#.....", ".##.....", "######..", "........")
b("3", ".####...", "#...##..", "....##..", "..###...",
        "....##..", "#...##..", ".####...", "........")
b("4", "....##..", "...###..", "..#.##..", ".#..##..",
        "######..", "....##..", "....##..", "........")
b("5", "######..", "##......", "#####...", "....##..",
        "....##..", "#...##..", ".####...", "........")
b("6", "..###...", ".#......", "##......", "#####...",
        "##..##..", "##..##..", ".####...", "........")
b("7", "######..", "....##..", "....#...", "...#....",
        "..##....", "..##....", "..##....", "........")
b("8", ".####...", "##..##..", "##..##..", ".####...",
        "##..##..", "##..##..", ".####...", "........")
b("9", ".####...", "##..##..", "##..##..", ".#####..",
        "....##..", "....#...", ".###....", "........")
b(":", "........", ".###....", ".###....", "........",
        ".###....", ".###....", "........", "........")
b(";", "........", "..##....", "..##....", "........",
        "..##....", "..##....", "..##....", "........")
b("<", "....##..", "...##...", "..##....", ".##.....",
        "..##....", "...##...", "....##..", "........")
b("=", "........", "#####...", "#####...", "........",
        "#####...", "#####...", "........", "........")
b(">", "##......", ".##.....", "..##....", "...##...",
        "..##....", ".##.....", "##......", "........")
b("?", ".####...", "#...##..", "....##..", "...##...",
        "..##....", "........", "..##....", "........")

# ── capitals, bold ────────────────────────────────────────────────────────────

b("@", ".####...", "##...#..", "#.###...", "#.#.#...",
        "#.####..", "##......", ".####...", "........")
b("A", "..##....", ".#..#...", "##..##..", "##..##..",
        "######..", "##..##..", "##..##..", "........")
b("B", "#####...", "##..##..", "##..##..", "#####...",
        "##..##..", "##..##..", "#####...", "........")
b("C", ".####...", "##...#..", "##......", "##......",
        "##......", "##...#..", ".####...", "........")
b("D", "####....", "##..#...", "##..##..", "##..##..",
        "##..##..", "##..#...", "####....", "........")
b("E", "######..", "##......", "##......", "#####...",
        "##......", "##......", "######..", "........")
b("F", "######..", "##......", "##......", "#####...",
        "##......", "##......", "##......", "........")
b("G", ".####...", "##...#..", "##......", "##.###..",
        "##..##..", "##..##..", ".#####..", "........")
b("H", "##..##..", "##..##..", "##..##..", "######..",
        "##..##..", "##..##..", "##..##..", "........")
b("I", ".###....", ".##.....", ".##.....", ".##.....",
        ".##.....", ".##.....", ".###....", "........")
b("J", "...###..", "....##..", "....##..", "....##..",
        "....##..", "#...##..", ".####...", "........")
b("K", "##...#..", "##..#...", "##.#....", "###.....",
        "##.#....", "##..#...", "##...#..", "........")
b("L", "##......", "##......", "##......", "##......",
        "##......", "##......", "######..", "........")
b("M", "##..##..", "##..##..", "#.##.#..", "#.##.#..",
        "##..##..", "##..##..", "##..##..", "........")
b("N", "##..##..", "##..##..", "#.#..#..", "#..#.#..",
        "##..##..", "##..##..", "##..##..", "........")
b("O", ".####...", "##..##..", "##..##..", "##..##..",
        "##..##..", "##..##..", ".####...", "........")
b("P", "#####...", "##..##..", "##..##..", "#####...",
        "##......", "##......", "##......", "........")
b("Q", ".####...", "##..##..", "##..##..", "##..##..",
        "##..##..", "##..##..", ".####...", "...###..")
b("R", "#####...", "##..##..", "##..##..", "#####...",
        "#..#....", "##..#...", "##...#..", "........")
b("S", ".#####..", "##......", "##......", ".####...",
        "....##..", "....##..", "#####...", "........")
b("T", "######..", "..##....", "..##....", "..##....",
        "..##....", "..##....", "..##....", "........")
b("U", "##..##..", "##..##..", "##..##..", "##..##..",
        "##..##..", "##..##..", ".####...", "........")
b("V", "##..##..", "##..##..", "##..##..", "##..##..",
        ".#..#...", ".#..#...", "..##....", "........")
b("W", "##..##..", "##..##..", "##..##..", "#.##.#..",
        "#.##.#..", "##..##..", "##..##..", "........")
b("X", "##..##..", ".#..#...", "..##....", "..##....",
        "..##....", ".#..#...", "##..##..", "........")
b("Y", "#....#..", ".#..#...", "..##....", "..##....",
        "..##....", "..##....", "..##....", "........")
b("Z", "######..", "....##..", "....#...", "..##....",
        ".#......", "##......", "######..", "........")
b("[", "..###...", "..##....", "..##....", "..##....",
        "..##....", "..##....", "..###...", "........")
b("\\", "##......", ".##.....", "..##....", "...##...",
         "....##..", "....##..", "........", "........")
b("]", "..###...", "...##...", "...##...", "...##...",
        "...##...", "...##...", "..###...", "........")

# ── lowercase, bold ───────────────────────────────────────────────────────────

b("a", "........", "........", ".####...", "....##..",
        ".#####..", "#...##..", ".#####..", "........")
b("b", "##......", "##......", "#####...", "##..##..",
        "##..##..", "##..##..", "#####...", "........")
b("c", "........", "........", ".#####..", "##......",
        "##......", "##......", ".#####..", "........")
b("d", "....##..", "....##..", ".#####..", "##..##..",
        "##..##..", "##..##..", ".#####..", "........")
b("e", "........", "........", ".####...", "##..##..",
        "######..", "##......", ".#####..", "........")
b("f", "...##...", ".##.....", "#####...", ".##.....",
        ".##.....", ".##.....", ".##.....", "........")
b("g", "........", "........", ".#####..", "##..##..",
        "##..##..", ".#####..", "....##..", ".####...")
b("h", "##......", "##......", "#####...", "##..##..",
        "##..##..", "##..##..", "##..##..", "........")
b("i", "..##....", "........", ".###....", "..##....",
        "..##....", "..##....", ".####...", "........")
b("j", "....##..", "........", "...###..", "....##..",
        "....##..", "....##..", "#...##..", ".####...")
b("k", "##......", "##......", "##..#...", "##.#....",
        "###.....", "##.#....", "##..#...", "........")
b("l", ".##.....", "..##....", "..##....", "..##....",
        "..##....", "..##....", ".####...", "........")
b("n", "........", "........", "#####...", "##..##..",
        "##..##..", "##..##..", "##..##..", "........")
b("o", "........", "........", ".####...", "##..##..",
        "##..##..", "##..##..", ".####...", "........")
b("p", "........", "........", "#####...", "##..##..",
        "##..##..", "#####...", "##......", "##......")
b("q", "........", "........", ".#####..", "##..##..",
        "##..##..", ".#####..", "....##..", "....##..")
b("r", "........", "........", "#.###...", "##......",
        "##......", "##......", "##......", "........")
b("s", "........", "........", ".#####..", "##......",
        ".####...", "....##..", "#####...", "........")
b("t", ".##.....", ".##.....", "#####...", ".##.....",
        ".##.....", ".##.#...", "...#....", "........")
b("u", "........", "........", "##..##..", "##..##..",
        "##..##..", "##..##..", ".#####..", "........")
b("v", "........", "........", "##..##..", "##..##..",
        ".#..#...", ".#..#...", "..##....", "........")
b("w", "........", "........", "##..##..", "#.##.#..",
        "#.##.#..", "#.##.#..", ".#..#...", "........")
b("x", "........", "........", "##..##..", ".#..#...",
        "..##....", ".#..#...", "##..##..", "........")
b("y", "........", "........", "##..##..", "##..##..",
        "##..##..", ".#####..", "....##..", ".####...")
b("z", "........", "........", "######..", "...##...",
        "..##....", ".##.....", "######..", "........")
b("{", "...##...", "..##....", "..##....", ".#......",
        "..##....", "..##....", "...##...", "........")
b("|", ".##.....", ".##.....", ".##.....", ".##.....",
        ".##.....", ".##.....", ".##.....", "........")
b("}", "..##....", "...##...", "...##...", ".....#..",
        "...##...", "...##...", "..##....", "........")

# ── everything else, bold ─────────────────────────────────────────────────────

b("§", ".###....", "#...#...", ".##.....", "#.##....",
        "..##....", "#...#...", ".###....", "........")
b("³", "##......", "###.....", "##......", "........",
        "........", "........", "........", "........")
b("·", "........", "........", "........", ".###....",
        ".###....", "........", "........", "........")
b("ƒ", "...##...", ".##.....", "#####...", ".##.....",
        ".##.....", ".##.....", "#.#.....", ".#......")
b("Γ", "######..", "##......", "##......", "##......",
        "##......", "##......", "##......", "........")
b("δ", ".####...", "#..##...", "..##....", ".#..#...",
        "##..##..", "##..##..", ".####...", "........")
b("κ", "........", "........", "##..#...", "##.#....",
        "###.....", "##.#....", "##..#...", "........")
b("λ", "###.....", ".##.....", ".##.....", ".##.....",
        "##.#....", "##..#...", "##...#..", "........")
b("τ", "........", "........", "#####...", ".##.....",
        ".##.....", ".##.....", "...##...", "........")
b("ᵈ", ".##.....", ".###....", "#.##....", ".###....",
        "........", "........", "........", "........")
b("ᵢ", "........", "........", "........", "........",
        ".#......", "........", "##......", "##......")
b("–", "........", "........", "........", ".####...",
        ".####...", "........", "........", "........")
b("—", "........", "........", "........", "######..",
        "######..", "........", "........", "........")
b("•", "........", "........", ".####...", "######..",
        "######..", ".####...", "........", "........")
b("⁶", ".##.....", "##......", "###.....", "###.....",
        "........", "........", "........", "........")
b("₁", "........", "........", "........", "........",
        "##......", "##......", "##......", "###.....")
b("₂", "........", "........", "........", "........",
        "##......", "..#.....", "##......", "###.....")
b("ℓ", "..##....", ".#..#...", ".#..#...", ".##.....",
        ".##.....", ".##.....", "..##....", "........")
b("←", "........", "........", "........", ".##.....",
        "######..", ".##.....", "........", "........")
b("→", "........", "........", "........", "...##...",
        "######..", "...##...", "........", "........")
b("∈", "........", "........", "..####..", ".##.....",
        ".####...", ".##.....", "..####..", "........")
b("−", "........", "........", "........", "######..",
        "######..", "........", "........", "........")
b("∪", "........", "........", "##..##..", "##..##..",
        "##..##..", "##..##..", ".####...", "........")
b("≤", "........", "...##...", ".##.....", "#.......",
        ".##.....", "..###...", "######..", "........")
b("≥", "........", "##......", "..##....", "....##..",
        "..##....", "###.....", "######..", "........")
b("⊆", "........", "..####..", ".##.....", ".##.....",
        ".##.....", "..####..", "######..", "........")
b("⊢", "##......", "##......", "##......", "#####...",
        "##......", "##......", "##......", "........")
b("⟶", "........", "........", "........", "...##...",
        "######..", "...##...", "........", "........")


# ── the bold, checked against the regular ───────────────────────────────────


def bold() -> dict[str, tuple[str, ...]]:
    """The bold face: what `b` drew, and the regular where `SAME` says so.

    A glyph with no bold falls back to the regular rather than raising, so
    that `weigh` can say which one it was. Somebody drawing their first glyph
    should be told they forgot the second weight, not shown a traceback from
    inside the thing they were told they would not have to read.
    """
    return {ch: B[ch] if ch in B and ch not in SAME else G[ch] for ch in G}


def enclosed(rows: tuple[str, ...]) -> int:
    """Counters: runs of background the outside cannot reach.

    The thing a synthetic bold destroys, and the reason this face has a drawn
    one. Four-connected, so a counter a diagonal only touches at a corner is
    still a counter to look through.
    """
    ink = {(r, c) for r, bits in enumerate(rows) for c, v in enumerate(bits) if v == "#"}
    free = {(r, c) for r in range(CELL) for c in range(CELL)} - ink

    def spread(seed: tuple[int, int], pool: set[tuple[int, int]]) -> set[tuple[int, int]]:
        found, edge = {seed}, [seed]
        while edge:
            r, c = edge.pop()
            for to in ((r + 1, c), (r - 1, c), (r, c + 1), (r, c - 1)):
                if to in pool and to not in found:
                    found.add(to)
                    edge.append(to)
        return found

    outside: set[tuple[int, int]] = set()
    for at in free:
        if at[0] in (0, CELL - 1) or at[1] in (0, CELL - 1):
            outside |= spread(at, free - outside)
    counters, left = 0, free - outside
    while left:
        counters += 1
        left -= spread(next(iter(left)), left)
    return counters


def weigh() -> list[str]:
    """What is wrong with the bold, in the regular's terms.

    Every one of these is a way a face can go quietly wrong: a bold that is
    wider breaks every listing, one that is lighter is a mistake, one that
    lost a counter is the smear this was drawn to avoid, and one that is not
    heavier at all is a glyph somebody forgot — unless it says so.
    """
    faults = [f"bold {ch!r} has not been drawn; draw it with `b`, or put it in "
              "SAME if six columns cannot hold a second pixel anywhere in it"
              for ch in sorted(set(G) - set(B) - SAME, key=ord)]
    if faults:
        return faults
    heavy = bold()
    for ch in sorted(G, key=ord):
        reg, bol = G[ch], heavy[ch]
        ink = sum(r.count("#") for r in reg)
        fat = sum(r.count("#") for r in bol)
        if any(r[INK_COLS:].strip(".") for r in bol):
            faults.append(f"bold {ch!r} puts ink past column {INK_COLS - 1}")
        if enclosed(bol) < enclosed(reg):
            faults.append(f"bold {ch!r} closed a counter, {enclosed(reg)} -> {enclosed(bol)}")
        if fat < ink:
            faults.append(f"bold {ch!r} is lighter than the regular")
        elif fat == ink and ch not in SAME and ch != " ":
            faults.append(f"bold {ch!r} is no heavier and is not in SAME")
        elif fat > ink and ch in SAME:
            faults.append(f"bold {ch!r} is in SAME and is not the same drawing")
    for ch in sorted(set(B) - set(G), key=ord):
        faults.append(f"bold {ch!r} is a glyph the regular does not have")
    for ch in sorted(SAME - set(G), key=ord):
        faults.append(f"SAME names {ch!r}, which the face does not have")
    return faults


# ── the seam ────────────────────────────────────────────────────────────────


def main() -> int:
    if "--sheet" in sys.argv:
        print(sheet(bold() if "700" in sys.argv else G))
        return 0

    faults = weigh()
    if faults:
        print("font: the bold does not answer to the regular:", file=sys.stderr)
        for f in faults:
            print(f"  {f}", file=sys.stderr)
        return 1

    check = "--check" in sys.argv
    work = [
        (OUT / "nether.woff", woff(build(G)), f"{len(G)} glyphs"),
        (OUT / "nether-bold.woff", woff(build(bold(), 700)), f"{len(G)} glyphs, bold"),
        (OUT / "gfx" / "specimen.gif", specimen(G), "the specimen"),
        (OUT / "gfx" / "specimen-lit.gif", specimen(G, "lit"), "the lit specimen"),
    ]

    stale = []
    for path, data, note in work:
        current = path.read_bytes() if path.exists() else None
        if current == data:
            continue
        if check:
            stale.append(path)
        else:
            path.write_bytes(data)
            print(f"drew    {path.relative_to(ROOT)}  {len(data):>6} bytes  {note}")

    if check:
        if stale:
            print("font: what is committed is stale:", file=sys.stderr)
            for p in stale:
                print(f"  {p.relative_to(ROOT)}", file=sys.stderr)
            print("\n  run python3 site/font.py and commit the result", file=sys.stderr)
            return 1

    # No byte ceiling. tests/font/run is the better gate and it is stricter:
    # it fails on a glyph the site renders and the face lacks, and reports how
    # many the face carries that nothing renders. A face with none spare is as
    # small as it can be, which is what a ceiling was going to approximate.
    drawn = len(G) - len(SAME)
    print(f"font: {len(G)} glyph(s), {len(work[0][1])} bytes regular, "
          f"{len(work[1][1])} bytes bold ({drawn} of them drawn twice)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
