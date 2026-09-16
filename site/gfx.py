#!/usr/bin/env python3
"""
gfx — an animated GIF89a encoder, and the graphics for the site.

There is no image library here. There is no image library anywhere in this
repository. This file contains a GIF89a writer, an LZW compressor, and a 5x7
bitmap font, none of which is imported. The drawing and encoding are small
enough to keep together and inspect.

    python3 site/gfx.py            write site/gfx/*.gif
    python3 site/gfx.py --check    fail if what is committed is stale

Everything it emits is committed. Nothing here downloads anything.
"""

from __future__ import annotations

import binascii
import math
import struct
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
OUT = ROOT / "site" / "gfx"

# ── the palette ─────────────────────────────────────────────────────────────
#
# Sixteen slots, because that is what a GIF colour table is here, filled from
# site/design.py so that a drawing and the page it sits on are made of the same
# colours. The names are the roles they were always playing; what each one
# resolves to now comes out of Oklch rather than out of a 1980s text mode.

from design import NETHER, RAMPS  # noqa: E402 -- a sibling, not a dependency


def _rgb(value: str) -> tuple[int, int, int]:
    raw = value.lstrip("#")
    return tuple(int(raw[i:i + 2], 16) for i in (0, 2, 4))


_RAMP = RAMPS["nether"]
PALETTE = [
    _rgb(NETHER["ground"]),   #  0 VOID    -- the page's own ground
    _rgb(NETHER["ink"]),      #  1 BONE
    _rgb(NETHER["accent"]),   #  2 SULPHUR -- the lamp
    _rgb(NETHER["alt"]),      #  3 LILAC
    _rgb(NETHER["warn"]),     #  4 SALMON
    _rgb(_RAMP[4]),           #  5 ICE
    _rgb(NETHER["good"]),     #  6 LIME
    _rgb(NETHER["dimmer"]),   #  7 ASH
    _rgb(NETHER["dim"]),      #  8 SMOKE
    _rgb(NETHER["quote"]),    #  9 PLUM
    _rgb(_RAMP[0]),           # 10 BLOOD
    _rgb(_RAMP[2]),           # 11 MOSS
    _rgb(NETHER["raised"]),   # 12 DEEP    -- one step up from the ground
    _rgb(NETHER["code"]),     # 13 TEAL
    _rgb(_RAMP[6]),           # 14 ROT
    _rgb(_RAMP[1]),           # 15 BILE
    # Sixteen names were never nine strata, so `DEPTH` used to be six colours
    # with three of them repeated. The table is thirty-two entries now and the
    # nine have their own, which is the difference between a picture of the
    # ramp and an approximation of it.
    *[_rgb(c) for c in _RAMP],  # 16..24 the strata, in order
]
DEPTH = list(range(16, 25))

# The header declares a table of thirty-two, so thirty-two is what has to be
# written. A short table is not a smaller table: everything after it shifts by
# three bytes an entry and the file stops being a GIF.
PALETTE += [PALETTE[0]] * (32 - len(PALETTE))

VOID, BONE, SULPHUR, LILAC, SALMON, ICE, LIME, ASH = range(8)
SMOKE, PLUM, BLOOD, MOSS, DEEP, TEAL, ROT, BILE = range(8, 16)

# The depth ramp: stratum 0 is sulphur, stratum 8 is lilac.

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


# ── the cairn ───────────────────────────────────────────────────────────────


#: Five stones, base first, drawn small and blown up threefold like everything
#: else here. `+` is the face turned towards the lamp, `#` is stone, `-` is
#: the face turned away, and `.` is not stone at all.
#:
#: Written out one row at a time rather than generated. A stone with the same
#: profile on both sides is a disc, and five discs are a stack of plates -- so
#: none of these is symmetric, none is the same shape, and the pile leans.
STONES: list[tuple[int, list[str]]] = [
    (0, [
        "...++++#######----..",
        "..++##############--",
        ".+#################-",
        "+##################-",
        "-###############---.",
        "..---#########----..",
    ]),
    (1, [
        "..+++#######---..",
        ".++############--",
        "+##############--",
        "+#############---",
        "-###########----.",
        "..--#######---...",
    ]),
    (-2, [
        "..++######---.",
        ".+##########--",
        "+###########--",
        "-##########---",
        "..--######---.",
    ]),
    (2, [
        ".++#####--",
        "+#######--",
        "+#######--",
        "-..-####--",
    ]),
    (-1, [
        ".++###-",
        "+#####-",
        "-.--##-",
    ]),
]

#: Enough of a seam that you can count them.
COURSE = 1

#: What each mark in a stone is made of. The lamp is the only thing in the
#: language that carries light downwards, so the lit faces take sulphur.
QUARRY = {"+": BONE, "#": SMOKE, "-": ASH, ".": None}


def cairn() -> tuple[list[Frame], list[int]]:
    """The mark somebody leaves so that the next person can find the way down.

    A temple is built upwards. This is the other thing you can build out of
    stone: something small, at the mouth of a descent, that says a person was
    here and this is the way.
    """
    scale = 3
    small = Frame(32, 40, VOID)

    # The ground, and the dark it opens into. The rim catches the light and
    # the inside does not, which is the whole of what a shaft is.
    floor = 30
    small.rect(1, floor, 30, 1, ASH)
    small.rect(4, floor + 1, 24, 1, SMOKE)
    for n in range(2, 10):
        inset = 4 + n
        if inset * 2 < 32:
            # The rim catches what the lamp gives it and nothing below does.
            # A colour down there would be something to see, and the whole
            # point of a shaft is that there is not.
            small.rect(inset, floor + n, 32 - inset * 2, 1, ASH if n == 2 else VOID)

    y = floor
    for lean, rows in STONES:
        y -= len(rows) + COURSE
        wide = len(rows[0])
        if any(len(r) != wide for r in rows):
            raise SystemExit("gfx: a stone with rows of different widths")
        left = (32 - wide) // 2 + lean
        for n, row in enumerate(rows):
            for x, mark in enumerate(row):
                shade = QUARRY[mark]
                if shade is not None:
                    small.set(left + x, y + n, shade)

    frame = Frame(32 * scale, 40 * scale, VOID)
    for yy in range(40):
        for x in range(32):
            c = small.px[yy * 32 + x]
            if c != VOID:
                frame.rect(x * scale, yy * scale, scale, scale, c)
    return [frame], [0]


def magnify(source: Frame, scale: int) -> Frame:
    """Integer enlargement is the only enlargement a pixel drawing gets."""
    out = Frame(source.w * scale, source.h * scale, VOID)
    for y in range(source.h):
        for x in range(source.w):
            colour = source.px[y * source.w + x]
            if colour != VOID:
                out.rect(x * scale, y * scale, scale, scale, colour)
    return out


def block(f: Frame, x: int, y: int, w: int, h: int, d: int,
          top: int, face: int, side: int) -> None:
    """A small oblique block: top, face, and the side the light does not get."""
    for row in range(d):
        f.rect(x - row, y + row, w, 1, top)
    f.rect(x - d + 1, y + d, w, h, face)
    for row in range(h):
        f.rect(x - d + w, y + d + row, d, 1, side)


def trace_cube(f: Frame, x: int, y: int, phase: int) -> None:
    """A wire cairn: the trace is present, but never quite stays put."""
    glow = ICE if phase % 3 else SULPHUR
    for ox, oy in ((0, 0), (-4, -3)):
        f.rect(x + ox, y + oy, 8, 1, glow)
        f.rect(x + ox, y + oy + 7, 8, 1, glow)
        f.rect(x + ox, y + oy, 1, 8, glow)
        f.rect(x + ox + 7, y + oy, 1, 8, glow)
    for ax, ay, bx, by in ((0, 0, -4, -3), (7, 0, 3, -3),
                           (0, 7, -4, 4), (7, 7, 3, 4)):
        steps = max(abs(bx - ax), abs(by - ay))
        for n in range(steps + 1):
            f.set(x + ax + (bx - ax) * n // steps, y + ay + (by - ay) * n // steps, glow)
    f.set(x + 3, y + 3, LILAC)


def hero() -> tuple[list[Frame], list[int]]:
    """A cairn over a shaft; the trace circles, and the unanswered stays open."""
    w, h, frames = 200, 64, []
    for phase in range(12):
        f = Frame(w, h, VOID)
        for n in range(38):
            x = (n * 37 + phase * (1 + n % 3)) % w
            y = 3 + (n * 19) % 43
            if not 77 < x < 124 or y < 24:
                f.set(x, y, DEEP if n % 4 else ASH)
        for x in range(-40, 241, 20):
            for y in range(43, 60):
                xx = 100 + (x - 100) * (y - 39) // 23
                if (y + phase) % 3 == 0:
                    f.set(xx, y, DEEP)
        for y in (45, 50, 56, 62):
            f.rect(20 + (y - 43) * 2, y, 160 - (y - 43) * 4, 1, ASH)
        f.rect(76, 51, 48, 2, ASH)
        f.rect(81, 53, 38, 1, SMOKE)
        f.rect(87, 54, 26, 1, DEEP)
        f.rect(94, 55, 12, 1, SMOKE)
        block(f, 82, 43, 36, 5, 5, ASH, DEEP, VOID)
        block(f, 88, 35, 24, 6, 4, SMOKE, ASH, DEEP)
        block(f, 94, 28, 13, 5, 3, ASH, SMOKE, DEEP)
        f.rect(89, 39, 18, 1, SULPHUR)
        pulse = phase % 6
        f.rect(98, 32, 5, 4, PLUM)
        f.rect(99, 33, 3, 2, VOID)
        if pulse in (0, 1):
            f.rect(97, 31, 7, 1, LILAC)
            f.set(96, 33, LILAC)
            f.set(105, 33, LILAC)
        angle = math.tau * phase / 12
        cx = 100 + round(math.cos(angle) * 33)
        cy = 21 + round(math.sin(angle) * 7)
        for dot in range(18):
            orbit = math.tau * dot / 18
            ox = 100 + round(math.cos(orbit) * 33)
            oy = 21 + round(math.sin(orbit) * 7)
            if (dot + phase) % 3 == 0:
                f.set(ox, oy, TEAL)
        trace_cube(f, cx - 4, cy - 4, phase)
        for n in range(5):
            a = angle + n * math.tau / 5
            f.set(100 + round(math.cos(a) * (17 + n * 3)),
                  21 + round(math.sin(a) * (4 + n % 3)),
                  SULPHUR if n % 2 else ICE)
        frames.append(magnify(f, 3))
    return frames, [7] * len(frames)


def write_gif(path: Path, frames: list[Frame], delays: list[int], loop: bool = True) -> bytes:
    w, h = frames[0].w, frames[0].h
    out = bytearray(b"GIF89a")
    out += w.to_bytes(2, "little") + h.to_bytes(2, "little")
    out += bytes([0b1111_0100, 0, 0])  # global colour table, 32 entries
    for r, g, b in PALETTE:
        out += bytes((r, g, b))

    if loop:
        out += b"\x21\xff\x0bNETSCAPE2.0\x03\x01\x00\x00\x00"

    for frame, delay in zip(frames, delays):
        out += b"\x21\xf9\x04\x00" + delay.to_bytes(2, "little") + b"\x00\x00"
        out += b"\x2c" + (0).to_bytes(2, "little") + (0).to_bytes(2, "little")
        out += w.to_bytes(2, "little") + h.to_bytes(2, "little") + b"\x00"
        out += bytes([5]) + sub_blocks(lzw(bytes(frame.px), 5))

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


# ── a 5x7 font ──────────────────────────────────────────────────────────────

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
    "W": ["#...#", "#...#", "#...#", "#.#.#", "#.#.#", "##.##", ".#.#."],
    "X": ["#...#", ".#.#.", "..#..", "..#..", "..#..", ".#.#.", "#...#"],
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
    """A seamless 32x32 tile. Sediment: the ground, one step up, and no more.

    This was drawn in `ASH`, which was a mid grey when the palette was VGA and
    is `--dimmer` now: 3.46:1 against the ground, which is a legible grid
    rather than a texture. A background you can read is a background competing
    with the text on it.

    `DEEP` is `--raised`, 1.13:1 -- there if you look for it and gone if you do
    not, which is what sediment is.
    """
    f = Frame(32, 32, VOID)
    # Two strata to the tile, broken rather than ruled, because a straight line
    # all the way across a page is a rule and this is not one.
    for y in (5, 21):
        for x in range(0, 32, 3):
            if (x // 3 + y) % 4:
                f.set(x + (y // 16), y, DEEP)
    # And a few grains between them, at no particular spacing.
    for x, y in ((7, 12), (23, 13), (14, 27), (29, 2), (2, 18), (18, 30)):
        f.set(x, y, DEEP)
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


def owed() -> int:
    """How many holes the world still owes, read off the board.

    The same files `site/bake` reads. A counter that counts nothing is a
    decoration; this one counts down as the work is done, which is the only
    direction a roadmap is supposed to move.
    """
    open_states = {"unmarked", "marked", "descending", "starved"}
    n = 0
    for path in sorted((ROOT / "cairn" / "items").glob("*.md")):
        raw = path.read_text(encoding="utf-8")
        if not raw.startswith("---\n"):
            continue
        front = raw[4:raw.index("\n---\n", 3)]
        meta = dict(
            (k.strip(), v.strip().strip("\"'"))
            for k, _, v in (line.partition(":") for line in front.split("\n"))
            if k.strip() and v.strip()
        )
        if meta.get("type") != "milestone" and meta.get("status") in open_states:
            n += 1
    return n


def counter(text: str) -> tuple[list[Frame], list[int]]:
    """An odometer. A hit counter counted visits; this counts what is left."""
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



def descent_animated() -> tuple[list[Frame], list[int]]:
    """A trace falling through a recessed nine-stratum shaft.

    Each shelf stays lit after the trace crosses it. The picture makes the
    monotonicity law visible: descent has a direction, and none of the nine
    shelves ever unlights on the way down.
    """
    w, band = 120, 15
    h = 9 * band + 2
    frames = []
    for step in range(11):
        f = Frame(w, h, VOID)
        here = min(step, 8)

        # The shaft walls narrow the field without turning the diagram into a
        # decorative frame. A few seams make it stone rather than a UI panel.
        f.rect(13, 1, 1, h - 2, DEEP)
        f.rect(w - 14, 1, 1, h - 2, DEEP)
        for y in range(5, h - 4, 11):
            f.rect(10, y, 4, 1, ASH)
            f.rect(w - 14, y + 3, 4, 1, ASH)

        for d in range(9):
            y = 1 + d * band
            lit = d <= here
            colour = DEPTH[d] if lit else ASH
            face = DEEP if not lit else SMOKE
            # A shallow tray for every stratum: illuminated top, dark face,
            # and a one-pixel fall into the next shelf.
            f.rect(17, y + 1, 86, 1, colour)
            f.rect(18, y + 2, 84, 7, face)
            f.rect(18, y + 3, 1, 6, BONE if lit else DEEP)
            f.rect(102, y + 2, 1, 7, VOID)
            f.rect(19, y + 8, 83, 1, ASH)
            f.rect(22, y + 4, 76, 1, colour if lit else DEEP)
            f.text(5, y + 3, str(d), colour)

        # The falling trace is a cube rather than a marker: every depth is a
        # value's recorded history, not a point moving on a chart.
        y = 1 + here * band
        if step <= 8:
            trace_cube(f, 82, y + 2, step)
        else:
            # At the bottom, the trace is permanently marked by the hole the
            # ledger cannot fill: foreign code reaches the unrecorded.
            f.rect(85, y + 5, 9, 5, PLUM)
            f.rect(87, y + 6, 5, 3, VOID)
            f.set(84, y + 7, LILAC)
            f.set(94, y + 7, LILAC)

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
    "counter.gif": lambda: counter(f"{owed():06d}"),
    "cairn.gif": cairn,
    "hero.gif": hero,
    "badge-unlit.gif": lambda: badge("BEST VIEWED", "UNLIT", SULPHUR, BONE),
    "badge-handmade.gif": lambda: badge("NO LIBRARIES", "FROM SCRATCH", LIME, BONE),
    "badge-public.gif": lambda: badge("PUBLIC DOMAIN", "TAKE IT", ICE, BONE),
    "badge-norun.gif": lambda: badge("THERE IS NO", "RUN", SALMON, LILAC),
    "badge-80col.gif": lambda: badge("80 COLUMNS", "AND NO MORE", BILE, BONE),
    "badge-decay.gif": lambda: badge("THE CORE ONLY", "DECAYS", ROT, BONE),
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
