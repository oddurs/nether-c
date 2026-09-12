#!/usr/bin/env python3
"""
gfx — hand-written animated GIF89a encoder, and the graphics for the site.

There is no image library here. There is no image library anywhere in this
repository. This file contains a GIF89a writer, an LZW compressor, and a 5x7
bitmap font, all of which are typed out rather than imported, because a project
that inverts TempleOS should not be assembled out of other people's parts.

    python3 site/gfx.py            write site/gfx/*.gif
    python3 site/gfx.py --check    fail if what is committed is stale

Everything it emits is committed. Nothing here downloads anything.
"""

from __future__ import annotations

import binascii
import struct
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "site" / "gfx"

# ── the palette ─────────────────────────────────────────────────────────────
# VGA, complemented. Fourteen of the sixteen are their own opposites.
# BROWN and LTBLUE are not, so ROT and BILE were invented for them.

PALETTE = [
    (0x00, 0x00, 0x00),  #  0 VOID
    (0xFF, 0xFF, 0xFF),  #  1 BONE
    (0xFF, 0xFF, 0x55),  #  2 SULPHUR
    (0xFF, 0x55, 0xFF),  #  3 LILAC
    (0xFF, 0x55, 0x55),  #  4 SALMON
    (0x55, 0xFF, 0xFF),  #  5 ICE
    (0x55, 0xFF, 0x55),  #  6 LIME
    (0x55, 0x55, 0x55),  #  7 ASH
    (0xAA, 0xAA, 0xAA),  #  8 SMOKE
    (0xAA, 0x00, 0xAA),  #  9 PLUM
    (0xAA, 0x00, 0x00),  # 10 BLOOD
    (0x00, 0xAA, 0x00),  # 11 MOSS
    (0x00, 0x00, 0xAA),  # 12 DEEP
    (0x00, 0xAA, 0xAA),  # 13 TEAL
    (0x55, 0xAA, 0xFF),  # 14 ROT   (no opposite)
    (0xAA, 0xAA, 0x00),  # 15 BILE  (no opposite)
]

VOID, BONE, SULPHUR, LILAC, SALMON, ICE, LIME, ASH = range(8)
SMOKE, PLUM, BLOOD, MOSS, DEEP, TEAL, ROT, BILE = range(8, 16)

# The depth ramp: stratum 0 is sulphur, stratum 8 is lilac.
DEPTH = [SULPHUR, SULPHUR, BILE, ROT, SALMON, SALMON, PLUM, LILAC, LILAC]

# ── LZW, as the GIF specification describes it ──────────────────────────────


class BitPacker:
    """GIF packs codes least-significant-bit first across byte boundaries."""

    def __init__(self) -> None:
        self.out = bytearray()
        self.acc = 0
        self.nbits = 0

    def write(self, code: int, width: int) -> None:
        self.acc |= code << self.nbits
        self.nbits += width
        while self.nbits >= 8:
            self.out.append(self.acc & 0xFF)
            self.acc >>= 8
            self.nbits -= 8

    def flush(self) -> bytes:
        if self.nbits:
            self.out.append(self.acc & 0xFF)
            self.acc = 0
            self.nbits = 0
        return bytes(self.out)


def lzw(indices: bytes, min_code_size: int) -> bytes:
    clear = 1 << min_code_size
    end = clear + 1
    table: dict[tuple[int, ...], int] = {(i,): i for i in range(clear)}
    next_code = end + 1
    width = min_code_size + 1

    packer = BitPacker()
    packer.write(clear, width)

    prefix: tuple[int, ...] = ()
    for byte in indices:
        candidate = prefix + (byte,)
        if candidate in table:
            prefix = candidate
            continue
        packer.write(table[prefix], width)
        table[candidate] = next_code
        next_code += 1
        if next_code > (1 << width):
            width += 1
        if next_code >= 4096:
            packer.write(clear, width)
            table = {(i,): i for i in range(clear)}
            next_code = end + 1
            width = min_code_size + 1
        prefix = (byte,)

    if prefix:
        packer.write(table[prefix], width)
    packer.write(end, width)
    return packer.flush()


def sub_blocks(data: bytes) -> bytes:
    out = bytearray()
    for i in range(0, len(data), 255):
        chunk = data[i : i + 255]
        out.append(len(chunk))
        out += chunk
    out.append(0)
    return bytes(out)


# ── the canvas ──────────────────────────────────────────────────────────────


class Frame:
    def __init__(self, w: int, h: int, fill: int = VOID) -> None:
        self.w, self.h = w, h
        self.px = bytearray([fill]) * (w * h)

    def set(self, x: int, y: int, c: int) -> None:
        if 0 <= x < self.w and 0 <= y < self.h:
            self.px[y * self.w + x] = c

    def rect(self, x: int, y: int, w: int, h: int, c: int) -> None:
        for yy in range(y, y + h):
            for xx in range(x, x + w):
                self.set(xx, yy, c)

    def frame_box(self, x: int, y: int, w: int, h: int, hi: int, lo: int) -> None:
        """A 1990s bevel. Light from the top left, always."""
        self.rect(x, y, w, 1, hi)
        self.rect(x, y, 1, h, hi)
        self.rect(x, y + h - 1, w, 1, lo)
        self.rect(x + w - 1, y, 1, h, lo)

    def text(self, x: int, y: int, s: str, c: int) -> int:
        for ch in s.upper():
            glyph = FONT.get(ch, FONT[" "])
            for row, bits in enumerate(glyph):
                for col, on in enumerate(bits):
                    if on == "#":
                        self.set(x + col, y + row, c)
            x += 6
        return x

    @staticmethod
    def width_of(s: str) -> int:
        return len(s) * 6 - 1


def write_gif(path: Path, frames: list[Frame], delays: list[int], loop: bool = True) -> bytes:
    w, h = frames[0].w, frames[0].h
    out = bytearray(b"GIF89a")
    out += w.to_bytes(2, "little") + h.to_bytes(2, "little")
    out += bytes([0b1111_0011, 0, 0])  # global colour table, 16 entries
    for r, g, b in PALETTE:
        out += bytes((r, g, b))

    if loop:
        out += b"\x21\xff\x0bNETSCAPE2.0\x03\x01\x00\x00\x00"

    for frame, delay in zip(frames, delays):
        out += b"\x21\xf9\x04\x00" + delay.to_bytes(2, "little") + b"\x00\x00"
        out += b"\x2c" + (0).to_bytes(2, "little") + (0).to_bytes(2, "little")
        out += w.to_bytes(2, "little") + h.to_bytes(2, "little") + b"\x00"
        out += bytes([4]) + sub_blocks(lzw(bytes(frame.px), 4))

    out += b"\x3b"
    return bytes(out)


# ── PNG, for the one thing that cannot be a GIF ─────────────────────────────
#
# Link previews want a PNG. PNG is a signature, four chunk types and a zlib
# stream, and zlib is in the standard library, so this is about forty lines and
# not a dependency.


def chunk(kind: bytes, data: bytes) -> bytes:
    return (
        struct.pack(">I", len(data))
        + kind
        + data
        + struct.pack(">I", binascii.crc32(kind + data) & 0xFFFFFFFF)
    )


def write_png(frame: Frame, scale: int = 1) -> bytes:
    w, h = frame.w * scale, frame.h * scale

    raw = bytearray()
    for y in range(frame.h):
        row = bytes(frame.px[y * frame.w : (y + 1) * frame.w])
        if scale > 1:
            row = bytes(b for px in row for _ in range(scale) for b in (px,))
        for _ in range(scale):
            raw.append(0)  # filter: none. The image is flat colour; nothing to predict.
            raw += row

    plte = b"".join(bytes(c) for c in PALETTE)

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 3, 0, 0, 0))
        + chunk(b"PLTE", plte)
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )


def big_text(f: Frame, x: int, y: int, s: str, c: int, scale: int) -> int:
    """The 5x7 font, blown up. Blocky on purpose."""
    for ch in s.upper():
        glyph = FONT.get(ch, FONT[" "])
        for row, bits in enumerate(glyph):
            for col, on in enumerate(bits):
                if on == "#":
                    f.rect(x + col * scale, y + row * scale, scale, scale, c)
        x += 6 * scale
    return x


# ── a 5x7 font, typed out ───────────────────────────────────────────────────

FONT: dict[str, list[str]] = {
    " ": ["....."] * 7,
    "A": [".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"],
    "B": ["####.", "#...#", "####.", "#...#", "#...#", "#...#", "####."],
    "C": [".###.", "#...#", "#....", "#....", "#....", "#...#", ".###."],
    "D": ["####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####."],
    "E": ["#####", "#....", "####.", "#....", "#....", "#....", "#####"],
    "F": ["#####", "#....", "####.", "#....", "#....", "#....", "#...."],
    "G": [".###.", "#...#", "#....", "#.###", "#...#", "#...#", ".###."],
    "H": ["#...#", "#...#", "#####", "#...#", "#...#", "#...#", "#...#"],
    "I": ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "#####"],
    "J": ["..###", "...#.", "...#.", "...#.", "...#.", "#..#.", ".##.."],
    "K": ["#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#"],
    "L": ["#....", "#....", "#....", "#....", "#....", "#....", "#####"],
    "M": ["#...#", "##.##", "#.#.#", "#...#", "#...#", "#...#", "#...#"],
    "N": ["#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#", "#...#"],
    "O": [".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."],
    "P": ["####.", "#...#", "#...#", "####.", "#....", "#....", "#...."],
    "Q": [".###.", "#...#", "#...#", "#...#", "#.#.#", "#..#.", ".##.#"],
    "R": ["####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#"],
    "S": [".####", "#....", "#....", ".###.", "....#", "....#", "####."],
    "T": ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."],
    "U": ["#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."],
    "V": ["#...#", "#...#", "#...#", "#...#", "#...#", ".#.#.", "..#.."],
    "W": ["#...#", "#...#", "#...#", "#...#", "#.#.#", "##.##", "#...#"],
    "X": ["#...#", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#...#"],
    "Y": ["#...#", "#...#", ".#.#.", "..#..", "..#..", "..#..", "..#.."],
    "Z": ["#####", "....#", "...#.", "..#..", ".#...", "#....", "#####"],
    "0": [".###.", "#...#", "#..##", "#.#.#", "##..#", "#...#", ".###."],
    "1": ["..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###."],
    "2": [".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####"],
    "3": ["####.", "....#", "....#", ".###.", "....#", "....#", "####."],
    "4": ["...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#."],
    "5": ["#####", "#....", "####.", "....#", "....#", "#...#", ".###."],
    "6": [".###.", "#...#", "#....", "####.", "#...#", "#...#", ".###."],
    "7": ["#####", "....#", "...#.", "..#..", ".#...", ".#...", ".#..."],
    "8": [".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###."],
    "9": [".###.", "#...#", "#...#", ".####", "....#", "#...#", ".###."],
    ".": [".....", ".....", ".....", ".....", ".....", ".##..", ".##.."],
    ",": [".....", ".....", ".....", ".....", ".##..", ".##..", ".#..."],
    "-": [".....", ".....", ".....", "#####", ".....", ".....", "....."],
    "!": ["..#..", "..#..", "..#..", "..#..", "..#..", ".....", "..#.."],
    "?": [".###.", "#...#", "....#", "...#.", "..#..", ".....", "..#.."],
    ":": [".....", ".##..", ".##..", ".....", ".##..", ".##..", "....."],
    "/": ["....#", "....#", "...#.", "..#..", ".#...", "#....", "#...."],
    "'": ["..#..", "..#..", ".....", ".....", ".....", ".....", "....."],
    "(": ["...#.", "..#..", ".#...", ".#...", ".#...", "..#..", "...#."],
    ")": [".#...", "..#..", "...#.", "...#.", "...#.", "..#..", ".#..."],
    "+": [".....", "..#..", "..#..", "#####", "..#..", "..#..", "....."],
    "*": [".....", "#.#.#", ".###.", "#####", ".###.", "#.#.#", "....."],
    "=": [".....", ".....", "#####", ".....", "#####", ".....", "....."],
    "#": [".#.#.", "#####", ".#.#.", ".#.#.", "#####", ".#.#.", "....."],
}


# ── the graphics ────────────────────────────────────────────────────────────


def bg_tile() -> tuple[list[Frame], list[int]]:
    """A seamless 32x32 tile. Sediment, barely visible, tiled forever."""
    f = Frame(32, 32, VOID)
    for y in range(32):
        if y % 8 == 0:
            for x in range(0, 32, 2):
                f.set(x + (y // 8 % 2), y, ASH)
        if y % 16 == 7:
            for x in range(0, 32, 5):
                f.set(x, y, ASH)
    return [f], [0]


def lamp() -> tuple[list[Frame], list[int]]:
    """A lamp, guttering. Carry it down or see nothing."""
    frames = []
    shapes = [
        ("..##..", ".####.", "######", ".####.", "..##.."),
        "...#.. ..###. .####. ..###. ...#..".split(),
        "..#... .###.. #####. .###.. ..#...".split(),
        ".. #.. ..##.. .####. ..##.. ...#..".replace(" ", "").split(),
    ]
    palette = [SULPHUR, BILE, ROT, SALMON]
    for i in range(8):
        f = Frame(24, 24, VOID)
        f.rect(9, 16, 6, 6, SMOKE)          # the vessel
        f.rect(10, 17, 4, 4, ASH)
        f.rect(11, 14, 2, 2, BILE)          # the wick
        rows = shapes[i % 4] if i % 4 else shapes[0]
        colour = palette[i % len(palette)]
        for r, row in enumerate(rows):
            for c, on in enumerate(row):
                if on == "#":
                    f.set(9 + c, 6 + r, colour)
        f.set(12, 4, SULPHUR if i % 2 else VOID)
        frames.append(f)
    return frames, [11] * 8


def excavation() -> tuple[list[Frame], list[int]]:
    """The 1990s promised that everything was under construction. It was."""
    w, h, n = 396, 44, 6
    frames = []
    for step in range(n):
        f = Frame(w, h, VOID)
        for x in range(0, w, 8):
            f.rect(x, 0, 4, 3, BILE)
            f.rect(x + 4, 0, 4, 3, VOID)
            f.rect(x + (step * 2) % 8, h - 3, 4, 3, BILE)
        f.frame_box(0, 4, w, h - 8, SMOKE, ASH)
        msg = "UNDER EXCAVATION"
        f.text((w - Frame.width_of(msg)) // 2, 12, msg, SULPHUR)
        sub = "NOTHING IS IMPLEMENTED - THE SPEC COMES FIRST"
        f.text((w - Frame.width_of(sub)) // 2, 24, sub, SMOKE if step % 2 else ASH)
        # a pick, swinging
        px = 22 + (step % 3) * 2
        py = 16 + (step % 3)
        for k in range(7):
            f.set(px + k, py + k, ROT)
        f.rect(px + 5, py + 5, 4, 2, SALMON)
        frames.append(f)
    return frames, [16] * n


def badge(top: str, bottom: str, ink: int, glow: int) -> tuple[list[Frame], list[int]]:
    """88x31. The only standard the web ever really agreed on."""
    frames = []
    for step in range(2):
        f = Frame(88, 31, VOID)
        f.frame_box(0, 0, 88, 31, SMOKE, ASH)
        f.rect(1, 1, 86, 13, VOID)
        f.text((88 - Frame.width_of(top)) // 2, 4, top, ink if step == 0 else glow)
        f.rect(2, 16, 84, 1, ASH)
        f.text((88 - Frame.width_of(bottom)) // 2, 20, bottom, SMOKE)
        frames.append(f)
    return frames, [70, 70]


def counter(text: str) -> tuple[list[Frame], list[int]]:
    """A hit counter counts visitors. This one counts nothing; it is a name."""
    pad, cell = 3, 13
    w = pad * 2 + cell * len(text)
    h = 26
    frames = []
    for step in range(2):
        f = Frame(w, h, VOID)
        f.frame_box(0, 0, w, h, ASH, SMOKE)
        for i, ch in enumerate(text):
            x = pad + i * cell
            f.rect(x, 4, cell - 2, 18, VOID)
            f.frame_box(x, 4, cell - 2, 18, ASH, SMOKE)
            lit = ICE if (i + step) % len(text) != 0 else BONE
            f.text(x + 3, 9, ch, lit)
        frames.append(f)
    return frames, [55, 55]



def sign_over_the_door() -> tuple[list[Frame], list[int]]:
    """NETHER C, in blocks, with the depth ramp bleeding down through it.

    The letters are lit from stratum 0 and stain toward stratum 8 as the
    animation runs, which is the only thing the page is really about.
    """
    w, h, n = 396, 62, 9
    frames = []
    for step in range(n):
        f = Frame(w, h, VOID)

        # sediment behind, so the ground is not flat
        for y in range(0, h, 5):
            for x in range((y // 5) % 4, w, 4):
                f.set(x, y, ASH)

        title = "NETHER C"
        scale = 4
        x0 = (w - len(title) * 6 * scale) // 2
        for i, ch in enumerate(title):
            # Each letter sits one stratum deeper than the one before it, and
            # the whole word descends as the animation runs.
            band = min(8, i + step)
            big_text(f, x0 + i * 6 * scale, 8, ch, DEPTH[band], scale)

        f.rect(0, h - 5, w, 1, ASH)
        for d in range(9):
            f.rect(d * (w // 9), h - 4, (w // 9) + 1, 3, DEPTH[d])
        frames.append(f)
    return frames, [18] * n


def descent_animated() -> tuple[list[Frame], list[int]]:
    """A value falling through the strata, and never coming back.

    It leaves the band it passed stained behind it. That is the monotonicity
    law of spec 1.2 drawn rather than stated: no step lowers a depth, and
    there is no `ascend`.
    """
    w, band = 120, 15
    h = 9 * band + 2
    frames = []
    for step in range(11):
        f = Frame(w, h, VOID)
        here = min(step, 8)
        for d in range(9):
            y = 1 + d * band
            lit = d <= here
            colour = DEPTH[d] if lit else ASH
            f.rect(1, y, w - 2, band - 1, colour)
            # a dotted floor between strata
            for x in range(1, w - 1, 3):
                f.set(x + (d % 3), y + band - 2, VOID)
            f.text(5, y + 4, str(d), VOID if lit else SMOKE)

        # the thing that is falling
        y = 1 + here * band
        if step <= 8:
            f.rect(w - 26, y + 4, 10, 7, VOID)
            f.rect(w - 25, y + 5, 8, 5, BONE)
        else:
            # it stopped, and the trail above it stays lit for good
            f.text(w - 38, y + 4, "HERE", VOID)

        f.frame_box(0, 0, w, h, ASH, SMOKE)
        frames.append(f)
    return frames, [30] * 9 + [110, 110]


def orpheus() -> tuple[list[Frame], list[int]]:
    """seal, shade and look. The rule from spec 1.6, in four beats.

    A value sits at stratum 5. Its name rises freely. A shade of it rises
    opaque. Looking at that shade from depth 0 is refused.
    """
    w, h = 300, 96
    top, bottom = 14, 74

    def stage(beat: int) -> Frame:
        f = Frame(w, h, VOID)
        f.rect(0, top - 1, w, 1, ASH)
        f.rect(0, bottom, w, 1, ASH)
        f.text(4, top - 9, "DEPTH 0", SMOKE)
        f.text(4, bottom + 4, "STRATUM 5", DEPTH[5])

        # the value, down where it lives
        f.rect(20, bottom - 14, 34, 12, DEPTH[5])
        f.text(24, bottom - 11, "JSON", VOID)

        captions = [
            ("A VALUE AT STRATUM 5", SMOKE),
            ("SEAL: THE NAME RISES", SULPHUR),
            ("SHADE: IT RISES OPAQUE", ROT),
            ("LOOK: NOT FROM UP HERE", SALMON),
        ]
        text, colour = captions[beat]
        f.text((w - Frame.width_of(text)) // 2, 3, text, colour)

        if beat >= 1:
            # a cairn: pure, so it goes all the way up
            f.rect(96, top + 2, 52, 10, SULPHUR)
            f.text(100, top + 4, "8F3A1C", VOID)
            for y in range(top + 14, bottom - 14, 6):
                f.set(120, y, SULPHUR)
        if beat >= 2:
            # a shade: opaque, and it also rises
            f.rect(196, top + 2, 44, 10, ROT)
            f.text(202, top + 4, "SHADE", VOID)
            for y in range(top + 14, bottom - 14, 6):
                f.set(216, y, ROT)
        if beat == 3:
            # Looking into it from up here is refused. Strike the shade itself
            # rather than the caption: the shade is what you cannot open.
            for k in range(44):
                f.set(196 + k, top + 2 + k * 10 // 44, SALMON)
                f.set(196 + k, top + 11 - k * 10 // 44, SALMON)
            refusal = "DESCEND FIRST"
            f.text(218 - Frame.width_of(refusal) // 2, top + 18, refusal, SALMON)
        return f

    frames = [stage(b) for b in range(4)]
    return frames, [110, 110, 110, 190]


def hole_filled() -> tuple[list[Frame], list[int]]:
    """Burial leaves a hole; exhumation fills it and renames the trace.

    The cairn changing at the end is the point: answering a hole produces a
    new trace rather than revising the old one (spec 6.6).
    """
    w, h = 300, 58
    cells = [(18 + i * 34, 22) for i in range(7)]
    hole_at = 4

    def stage(beat: int) -> Frame:
        f = Frame(w, h, VOID)
        caption = [
            "BURY: ONE QUESTION LEFT OVER",
            "EXHUME --GRANT DISK",
            "THE WORLD ANSWERS",
            "SEALED, AND RENAMED",
        ][beat]
        f.text(6, 4, caption, [SMOKE, SULPHUR, ROT, LIME][beat])

        for i, (x, y) in enumerate(cells):
            if i == hole_at and beat == 0:
                # a hole: an outline with nothing in it
                f.frame_box(x, y, 26, 18, SALMON, SALMON)
                f.text(x + 9, y + 6, "?", SALMON)
            elif i == hole_at and beat == 1:
                f.frame_box(x, y, 26, 18, SULPHUR, SULPHUR)
                f.text(x + 9, y + 6, "?", SULPHUR)
            elif i == hole_at:
                f.rect(x, y, 26, 18, ROT if beat == 2 else LIME)
                f.text(x + 4, y + 6, "11K", VOID)
            else:
                f.rect(x, y, 26, 18, DEPTH[i % 3])
            if i:
                f.rect(x - 8, y + 8, 8, 1, SMOKE)

        name = "4C02AB7F" if beat < 3 else "77DE9B31"
        f.text(w - Frame.width_of(name) - 6, h - 10, name, ICE if beat == 3 else SMOKE)
        return f

    return [stage(b) for b in range(4)], [150, 110, 110, 190]


def rule_variant(kind: str) -> tuple[list[Frame], list[int]]:
    """Dividers that are not all the same bar.

    Eight identical scrolling rules down one page reads as wallpaper. Each of
    these says something about the section it precedes.
    """
    w, h = 396, 9

    if kind == "strata":
        # Nine solid bands: the lattice itself, still.
        f = Frame(w, h, VOID)
        for d in range(9):
            f.rect(d * (w // 9), 2, (w // 9) + 1, h - 4, DEPTH[d])
        return [f], [0]

    if kind == "descend":
        # Arrowheads, pointing down, marching one way.
        frames = []
        for step in range(8):
            f = Frame(w, h, VOID)
            for x in range((step * 2) % 16, w, 16):
                for k in range(4):
                    f.rect(x + 3 - k, 2 + k, 1 + k * 2, 1, DEPTH[(x // 16) % 9])
            frames.append(f)
        return frames, [90] * 8

    if kind == "cairn":
        # A hash, drawn as a hash: stacked blocks, deterministic from nothing.
        f = Frame(w, h, VOID)
        state = 0x9E37
        for x in range(0, w, 6):
            state = (state * 1103515245 + 12345) & 0xFFFF
            tall = state % 3
            f.rect(x, 3 - tall, 4, 3 + tall * 2, SULPHUR if state % 5 else ICE)
        return [f], [0]

    # "quiet": a hairline with one interruption, for between prose sections.
    f = Frame(w, h, VOID)
    f.rect(0, 4, w, 1, ASH)
    f.rect(w // 2 - 12, 3, 24, 3, VOID)
    f.rect(w // 2 - 8, 4, 16, 1, SULPHUR)
    return [f], [0]


def og_card() -> Frame:
    """1200x630, drawn at 200x105 and scaled six times. Blocky on purpose."""
    W, H = 200, 105
    f = Frame(W, H, VOID)

    f.rect(0, 0, W, 3, SULPHUR)
    for d in range(9):
        f.rect(d * (W // 9), H - 4, (W // 9) + 1, 4, DEPTH[d])

    # sediment, only in the margins, so it never sits behind a letter
    for y in range(8, H - 8, 6):
        for x in ((y // 6) % 3, W - 5 + ((y // 6) % 3)):
            f.set(x, y, ASH)

    big_text(f, 12, 14, "NETHER C", BONE, 3)          # 8 chars * 18 = 144
    big_text(f, 12, 42, "A C DIALECT IN WHICH", SMOKE, 1)
    big_text(f, 12, 52, "NOTHING RUNS", SALMON, 2)    # 12 chars * 12 = 144

    f.rect(12, 74, W - 24, 1, ASH)
    big_text(f, 12, 81, "THE PROGRAM HAS ALREADY RUN.", SMOKE, 1)
    big_text(f, 12, 90, "YOU READ WHAT IS LEFT OF IT.", ASH, 1)
    return f


GRAPHICS = {
    "bg.gif": bg_tile,
    "lamp.gif": lamp,
    "excavation.gif": excavation,
    "cairn.gif": lambda: counter("8F3A1C0E"),
    "badge-unlit.gif": lambda: badge("BEST VIEWED", "UNLIT", SULPHUR, BONE),
    "badge-handmade.gif": lambda: badge("MADE BY HAND", "NO LIBRARIES", LIME, BONE),
    "badge-public.gif": lambda: badge("PUBLIC DOMAIN", "TAKE IT", ICE, BONE),
    "badge-norun.gif": lambda: badge("THERE IS NO", "RUN", SALMON, LILAC),
    "badge-80col.gif": lambda: badge("80 COLUMNS", "AS GOD MEANT", BILE, BONE),
    "badge-decay.gif": lambda: badge("THE CORE ONLY", "DECAYS", ROT, BONE),
    "sign.gif": sign_over_the_door,
    "descending.gif": descent_animated,
    "orpheus.gif": orpheus,
    "hole.gif": hole_filled,
    "rule-strata.gif": lambda: rule_variant("strata"),
    "rule-descend.gif": lambda: rule_variant("descend"),
    "rule-cairn.gif": lambda: rule_variant("cairn"),
    "rule-quiet.gif": lambda: rule_variant("quiet"),
}


def main() -> int:
    check = "--check" in sys.argv
    OUT.mkdir(parents=True, exist_ok=True)
    stale = []

    work: list[tuple[str, bytes, str]] = []
    for name, make in GRAPHICS.items():
        frames, delays = make()
        work.append((name, write_gif(OUT / name, frames, delays), f"{len(frames)} frame(s)"))
    work.append(("card.png", write_png(og_card(), scale=6), "1200x630"))

    for name, data, note in work:
        path = OUT / name
        current = path.read_bytes() if path.exists() else None
        if current == data:
            continue
        if check:
            stale.append(name)
        else:
            path.write_bytes(data)
            print(f"drew    site/gfx/{name}  {len(data):>6} bytes  {note}")

    if check:
        if stale:
            print("gfx: the committed graphics are stale:", file=sys.stderr)
            for n in stale:
                print(f"  site/gfx/{n}", file=sys.stderr)
            print("\n  run python3 site/gfx.py and commit the result", file=sys.stderr)
            return 1
        print(f"gfx: {len(work)} graphic(s) up to date")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
