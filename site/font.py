#!/usr/bin/env python3
"""
font — a hand-drawn 8x8 bitmap face, and the TrueType writer that ships it.

There is no font library here, for the same reason there is no image library:
a project that inverts TempleOS should not be assembled out of other people's
parts. TempleOS had one 8x8 fixed-width font and that was the whole of it.

The site used to name a stack of fonts it hoped the reader already had, which
meant it rendered differently on every machine. A project whose whole claim is
that a trace means the same thing on a machine that has never seen your
filesystem should not leave its letterforms to chance.

    python3 site/font.py            write site/nether.woff and the specimen
    python3 site/font.py --check    fail if what is committed is stale
    python3 site/font.py --sheet    print every glyph as ASCII, to look at

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
# Eight by eight, because that was the covenant. One em is the cell, so eighty
# columns is exactly six hundred and forty pixels and the type scale is 8, 16,
# 24, 32 with nothing in between.
#
# Rows 0..6 sit above the baseline and row 7 below it, which gives a seven-row
# capital and one row of descender — the same proportion the 5x7 face in
# gfx.py uses, one row taller.

CELL = 8
UPEM = 1024
PIXEL = UPEM // CELL  # 128 font units to the pixel

# Six of the eight columns are ink and two are the gap, so the advance is six
# and not the whole em. A square cell would be right if the page were a 640x480
# screen; it is not, and a face whose advance equals its height sets eighty
# columns half again as wide as anything reads at.
ADVANCE_COLS = 6
ADVANCE = ADVANCE_COLS * PIXEL  # 768
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
        deltas.append(0 if start == 0xFFFF else (gid - start) & 0xFFFF)
    sub += struct.pack(f">{count}h", *[d - 0x10000 if d > 0x7FFF else d for d in deltas])
    sub += struct.pack(f">{count}H", *([0] * count))  # idRangeOffset: all delta

    return struct.pack(">HHHHI", 0, 1, 3, 1, 12) + sub


def build(glyphs: dict[str, tuple[str, ...]]) -> bytes:
    """Every table, in one sfnt."""
    codes = sorted(ord(c) for c in glyphs)
    order = [None] + [chr(c) for c in codes]  # glyph 0 is .notdef

    data = b""
    loca = [0]
    for ch in order:
        data += outline(glyphs[ch]) if ch is not None else b""
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
        0b0000_0000_0000_1011,  # baseline at y=0, lsb at x=0, integer ppem
        UPEM, 0, 0,             # created, modified: zero, so the file is reproducible
        0, -DESCENT, UPEM, ASCENT,
        0, 8, 2, 0 if short else 1, 0,
    )
    hhea = struct.pack(">IhhhHhhhhhhhhhhhH", 0x00010000, ASCENT, -DESCENT, 0,
                       ADVANCE, 0, 0, CELL * PIXEL, 1, 0, 0, 0, 0, 0, 0, 0, count)
    maxp = struct.pack(">IHHHHHHHHHHHHHH", 0x00010000, count, 4 * CELL * CELL,
                       CELL * CELL, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0)
    hmtx = struct.pack(">Hh", ADVANCE, 0) * count
    # `post` version 3.0: no glyph names. isFixedPitch is 1, because it is.
    post = struct.pack(">IIhhIIIII", 0x00030000, 0, -PIXEL, PIXEL, 1, 0, 0, 0, 0)

    # `OS/2` version 4, ninety-six bytes, written a field at a time. One long
    # format string was wrong by one value and said so in a way nobody could
    # read.
    os2 = struct.pack(">HhHHH", 4, ADVANCE, 400, 5, 0)          # version..fsType
    os2 += struct.pack(">10h",                                # sub, super, strikeout
                       UPEM // 2, UPEM // 2, 0, 0,
                       UPEM // 2, UPEM // 2, 0, ASCENT // 2,
                       PIXEL, 3 * PIXEL)
    os2 += struct.pack(">h", 8)                               # sFamilyClass: monospace
    os2 += bytes([2, 0, 5, 9, 0, 0, 0, 0, 0, 0])              # PANOSE: modern, monospaced
    os2 += struct.pack(">4I", 0b11, 0, 0, 0)                  # Latin-1 and Latin Ext
    os2 += b"NETH"                                            # achVendID
    os2 += struct.pack(">HHH", 0b1000000, min(codes), max(codes))  # fsSelection: regular
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
            (1, NAME), (2, "Regular"), (3, f"{NAME} {VERSION}"),
            (4, NAME), (5, VERSION), (6, NAME.replace(" ", "")),
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


def specimen(glyphs: dict[str, tuple[str, ...]]):
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
        colour = 2 + (i // across) % 6
        for r, bits in enumerate(rows):
            for c, on in enumerate(bits):
                if on == "#":
                    for dy in range(scale):
                        for dx in range(scale):
                            frame.set(ox + c * scale + dx, oy + r * scale + dy, colour)
    return gfx.write_gif(OUT / "gfx" / "specimen.gif", [frame], [0], loop=False)


# ── the face, typed out ─────────────────────────────────────────────────────
#
# Six columns wide and seven rows tall, in an eight by eight cell: the same
# proportion as the 5x7 face in gfx.py, one column and one row larger. Capitals
# and ascenders fill rows 0 to 6, the x-height is rows 2 to 6, and descenders
# drop into row 7. Strokes are one pixel, because at sixteen pixels a cell is
# doubled and two would be a slab.

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
g("@", ".####...", "#....#..", "#.###.#.", "#.#.#.#.",
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

g("\u00a7", ".###....", "#...#...", ".##.....", "#..#....",
             "..##....", "#...#...", ".###....", "........")
g("\u00b7", "........", "........", "........", "..##....",
             "..##....", "........", "........", "........")
g("\u2022", "........", "........", "..##....", ".####...",
             ".####...", "..##....", "........", "........")
g("\u2190", "........", "........", "........", ".#......",
             "######..", ".#......", "........", "........")
g("\u00ab", "........", "........", "..#..#..", ".#..#...",
             "#..#....", ".#..#...", "..#..#..", "........")
g("\u00bb", "........", "........", "#..#....", ".#..#...",
             "..#..#..", ".#..#...", "#..#....", "........")
g("\u00b3", "##......", ".##.....", "##......", "........",
             "........", "........", "........", "........")
g("\u00d7", "........", "........", "#...#...", ".#.#....",
             "..#.....", ".#.#....", "#...#...", "........")
g("\u0192", "...##...", "..#.....", "#####...", "..#.....",
             "..#.....", "..#.....", "#.#.....", ".#......")
g("\u0393", "######..", "#.......", "#.......", "#.......",
             "#.......", "#.......", "#.......", "........")
g("\u03b4", ".####...", "#...#...", "..##....", ".#..#...",
             "#....#..", "#....#..", ".####...", "........")
g("\u03ba", "........", "........", "#...#...", "#..#....",
             "###.....", "#..#....", "#...#...", "........")
g("\u03bb", "##......", ".#......", ".#......", ".##.....",
             "#..#....", "#...#...", "#....#..", "........")
g("\u03c4", "........", "........", "#####...", "..#.....",
             "..#.....", "..#.....", "...##...", "........")
g("\u1d48", "..#.....", ".###....", "#.##....", ".###....",
             "........", "........", "........", "........")
g("\u1d62", "........", "........", "........", "........",
             ".#......", "........", ".#......", ".#......")
g("\u2013", "........", "........", "........", "........",
             ".####...", "........", "........", "........")
g("\u2014", "........", "........", "........", "........",
             "######..", "........", "........", "........")
g("\u2026", "........", "........", "........", "........",
             "........", "........", "#.#.#...", "........")
g("\u2032", "..#.....", ".#......", "........", "........",
             "........", "........", "........", "........")
g("\u2076", ".##.....", "#.......", "###.....", ".##.....",
             "........", "........", "........", "........")
g("\u2081", "........", "........", "........", "........",
             ".#......", "##......", ".#......", "###.....")
g("\u2082", "........", "........", "........", "........",
             "##......", "..#.....", ".#......", "###.....")
g("\u2099", "........", "........", "........", "........",
             "........", "###.....", "#.#.....", "#.#.....")
g("\u2113", "..##....", ".#..#...", ".#..#...", ".##.....",
             ".#......", ".#......", "..##....", "........")
g("\u2192", "........", "........", "........", "....#...",
             "######..", "....#...", "........", "........")
g("\u2208", "........", "........", "..####..", ".#......",
             ".####...", ".#......", "..####..", "........")
g("\u2212", "........", "........", "........", "........",
             "######..", "........", "........", "........")
g("\u222a", "........", "........", "#....#..", "#....#..",
             "#....#..", "#....#..", ".####...", "........")
g("\u2261", "........", "........", "######..", "........",
             "######..", "........", "######..", "........")
g("\u2264", "........", "...##...", ".##.....", "#.......",
             ".##.....", "...##...", "######..", "........")
g("\u2265", "........", "##......", "..##....", "....##..",
             "..##....", "##......", "######..", "........")
g("\u2295", "........", ".#####..", "#..#..#.", "#.###.#.",
             "#..#..#.", ".#####..", "........", "........")
g("\u22a2", "#.......", "#.......", "#.......", "#####...",
             "#.......", "#.......", "#.......", "........")
g("\u2460", "........", ".#####..", "#..#..#.", "#..#..#.",
             "#..#..#.", ".#####..", "........", "........")
g("\u2500", "........", "........", "........", "........",
             "######..", "........", "........", "........")
g("\u2514", "..#.....", "..#.....", "..#.....", "..#.....",
             "..####..", "........", "........", "........")
g("\u2588", "######..", "######..", "######..", "######..",
             "######..", "######..", "######..", "######..")
g("\u27e8", "....#...", "...#....", "..#.....", ".#......",
             "..#.....", "...#....", "....#...", "........")
g("\u27e9", ".#......", "..#.....", "...#....", "....#...",
             "...#....", "..#.....", ".#......", "........")
g("\u27f6", "........", "........", "........", "....#...",
             "######..", "....#...", "........", "........")


# ── the seam ────────────────────────────────────────────────────────────────


def main() -> int:
    if "--sheet" in sys.argv:
        print(sheet(G))
        return 0

    check = "--check" in sys.argv
    work = [
        (OUT / "nether.woff", woff(build(G)), f"{len(G)} glyphs"),
        (OUT / "gfx" / "specimen.gif", specimen(G), "the specimen"),
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
    print(f"font: {len(G)} glyph(s), {len(work[0][1])} bytes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
