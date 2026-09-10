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
    ",": [".....", ".....", ".....", ".....", ".##..", ".##..", "#....."[:5]],
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


def rule_bar() -> tuple[list[Frame], list[int]]:
    """A divider that descends. Nine bands, scrolling."""
    w, h, n = 396, 9, 12
    frames = []
    for step in range(n):
        f = Frame(w, h, VOID)
        for x in range(w):
            band = ((x + step * 3) // 4) % 9
            f.rect(x, 1, 1, h - 2, DEPTH[band])
        f.rect(0, 0, w, 1, ASH)
        f.rect(0, h - 1, w, 1, ASH)
        frames.append(f)
    return frames, [8] * n


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


def descent() -> tuple[list[Frame], list[int]]:
    """Nine strata, and something going down through them."""
    w, h, band = 52, 9 * 16 + 2, 16
    frames = []
    for step in range(9):
        f = Frame(w, h, VOID)
        for d in range(9):
            y = 1 + d * band
            f.rect(1, y, w - 2, band - 1, DEPTH[d])
            for x in range(1, w - 1, 3):
                f.set(x + (d % 3), y + band - 2, VOID)
            f.text(4, y + 4, str(d), VOID)
            if d == step:
                f.rect(w - 12, y + 5, 8, 5, VOID)
                f.rect(w - 11, y + 6, 6, 3, BONE)
        f.frame_box(0, 0, w, h, ASH, SMOKE)
        frames.append(f)
    return frames, [42] * 9


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
    "rule.gif": rule_bar,
    "lamp.gif": lamp,
    "excavation.gif": excavation,
    "descent.gif": descent,
    "cairn.gif": lambda: counter("8F3A1C0E"),
    "badge-unlit.gif": lambda: badge("BEST VIEWED", "UNLIT", SULPHUR, BONE),
    "badge-handmade.gif": lambda: badge("MADE BY HAND", "NO LIBRARIES", LIME, BONE),
    "badge-public.gif": lambda: badge("PUBLIC DOMAIN", "TAKE IT", ICE, BONE),
    "badge-norun.gif": lambda: badge("THERE IS NO", "RUN", SALMON, LILAC),
    "badge-80col.gif": lambda: badge("80 COLUMNS", "AS GOD MEANT", BILE, BONE),
    "badge-decay.gif": lambda: badge("THE CORE ONLY", "DECAYS", ROT, BONE),
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
