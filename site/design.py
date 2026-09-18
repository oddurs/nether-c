#!/usr/bin/env python3
"""
design — the system every visible decision on this site is made out of.

There is no colour library here. This is the Oklab transform, which is nine
multiplications and a cube root, and having it means every colour on the site
can be *stated* rather than picked: a lightness, a chroma and an angle. Two
colours with the same lightness look equally bright, which is the whole reason
a ramp built this way looks designed and one picked out of a box does not.

    python3 site/design.py            the whole system, with its contrast
    python3 site/design.py --css      write site/tokens.css

Colour is role-based: indigo rooms, rose signals, and a violet-to-pink depth
ramp. The light palette is rose porcelain and lavender paper with plum ink,
not an inversion. Both palettes feed CSS, GIF colour tables and the favicon.
Space follows a twelve-pixel grid; type follows an eight-pixel drawing grid.
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

TOKENS = Path(__file__).resolve().parent / "tokens.css"

#: The hue the whole scheme is built on, in degrees. Indigo.
GROUND_HUE = 285.0

#: Rose light, not the arithmetic complement of the ground.
LAMP_HUE = 340.0


def srgb(lightness: float, chroma: float, hue: float) -> str:
    """One Oklch colour as `#rrggbb`, with the chroma pulled in until it fits.

    Out of gamut is not an error to report, it is a colour to find: the hue and
    the lightness are the meaning and the chroma is how loud it is, so when a
    colour will not fit in sRGB the chroma is what gives way.
    """
    for step in range(64):
        hit = _convert(lightness, chroma * (1 - step / 64), hue)
        if hit is not None:
            return hit
    return _convert(lightness, 0.0, hue) or "#000000"


def _convert(lightness: float, chroma: float, hue: float) -> str | None:
    a = chroma * math.cos(math.radians(hue))
    b = chroma * math.sin(math.radians(hue))

    l_ = lightness + 0.3963377774 * a + 0.2158037573 * b
    m_ = lightness - 0.1055613458 * a - 0.0638541728 * b
    s_ = lightness - 0.0894841775 * a - 1.2914855480 * b
    l, m, s = l_ ** 3, m_ ** 3, s_ ** 3

    linear = (
        +4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    )
    out = []
    for channel in linear:
        if channel < -0.0001 or channel > 1.0001:
            return None
        channel = min(max(channel, 0.0), 1.0)
        encoded = (
            12.92 * channel
            if channel <= 0.0031308
            else 1.055 * channel ** (1 / 2.4) - 0.055
        )
        out.append(round(encoded * 255))
    return "#{:02X}{:02X}{:02X}".format(*out)


def strata(lightness: float = 0.80, chroma: float = 0.12,
           dim: float = 0.07, rise: float = 0.04) -> list[str]:
    """Indigo through violet to rose. Numbers carry depth; colour reinforces it."""
    return [srgb(lightness - dim * n / 8, chroma + rise * n / 8,
                 260 + 90 * n / 8) for n in range(9)]


# Shared roles, separately composed for night and daylight.
NETHER = {
    "sunk": srgb(0.12, 0.035, GROUND_HUE),
    "ground": srgb(0.17, 0.045, GROUND_HUE),
    "raised": srgb(0.23, 0.050, GROUND_HUE),
    "ink": srgb(0.95, 0.025, 310),
    "dim": srgb(0.74, 0.045, 290),
    "dimmer": srgb(0.55, 0.055, 290),
    "accent": srgb(0.80, 0.150, LAMP_HUE),
    "alt": srgb(0.80, 0.100, 280),
    "warn": srgb(0.80, 0.120, 35),
    "good": srgb(0.80, 0.090, 185),
    "code": srgb(0.85, 0.080, 250),
    "quote": srgb(0.80, 0.080, 315),
}

LIT = {
    "sunk": srgb(0.985, 0.010, 310),
    "ground": srgb(0.960, 0.018, 335),
    "raised": srgb(0.925, 0.027, GROUND_HUE),
    "ink": srgb(0.270, 0.055, GROUND_HUE),
    "dim": srgb(0.460, 0.065, GROUND_HUE),
    "dimmer": srgb(0.560, 0.060, GROUND_HUE),
    "accent": srgb(0.470, 0.170, LAMP_HUE),
    "alt": srgb(0.430, 0.140, GROUND_HUE),
    "warn": srgb(0.480, 0.140, 25),
    "good": srgb(0.430, 0.070, 185),
    "code": srgb(0.400, 0.090, 265),
    "quote": srgb(0.460, 0.100, 315),
}

RAMPS = {"nether": strata(), "lit": strata(0.49, 0.12, dim=0.09, rise=0.04)}

# Spacing is independent of the font's seven-pixel advance.
#
# Ten rather than twelve. The type cannot come down with it -- a face drawn on
# an eight-pixel grid has four sizes and nothing between them -- so the only
# way this design gets smaller is by taking the air out, and a finer cell is
# what that means. Every step below keeps its count, so the whole scale moves
# together and no two things that used to line up stop.
CELL = 10
SPACE = {"hair": 1, "tight": 2, "snug": 3, "step": 4, "gap": 6, "room": 10}

#: Type. Four sizes and no others.
TYPE = {"fine": 8, "body": 16, "head": 24, "sign": 32}


def luminance(hex_colour: str) -> float:
    raw = hex_colour.lstrip("#")
    out = []
    for i in (0, 2, 4):
        channel = int(raw[i:i + 2], 16) / 255
        out.append(
            channel / 12.92 if channel <= 0.03928 else ((channel + 0.055) / 1.055) ** 2.4
        )
    return 0.2126 * out[0] + 0.7152 * out[1] + 0.0722 * out[2]


def contrast(one: str, two: str) -> float:
    a, b = luminance(one), luminance(two)
    return (max(a, b) + 0.05) / (min(a, b) + 0.05)


def tokens() -> str:
    """`site/tokens.css`. Generated, and the only place a colour is a number.

    `nether.css` says what things are made of; this says what the materials
    are. Nothing else in the stylesheet may contain a `#`.
    """
    lines = [
        "/* Generated by site/design.py. Do not edit.",
        " *",
        " * Colour is Oklch, so the ramps step evenly to the eye rather than",
        " * evenly in a number nobody perceives. Space is the cell. Type is the",
        " * eight-pixel drawing grid, which is a different grid and does not",
        " * move with it. Every value here is derived; the argument",
        " * for each is in site/design.py.",
        " */",
        "",
        ":root {",
    ]
    for key, value in NETHER.items():
        lines.append(f"  --{key}: {value};")
    for n, colour in enumerate(RAMPS["nether"]):
        lines.append(f"  --d{n}: {colour};")
    lines.append("")
    lines.append(f"  --cell: {CELL}px;")
    for key, count in SPACE.items():
        lines.append(f"  --{key}: calc({count} * var(--cell));")
    for key, size in TYPE.items():
        lines.append(f"  --type-{key}: {size}px;")
    lines += ["}", "", "/* The lamp, lit: rose porcelain, lavender paper and plum ink. */",
              ':root[data-lamp="lit"] {']
    for key, value in LIT.items():
        lines.append(f"  --{key}: {value};")
    for n, colour in enumerate(RAMPS["lit"]):
        lines.append(f"  --d{n}: {colour};")
    lines += ["}", ""]
    return "\n".join(lines)


def icon() -> str:
    """The cairn mark uses the same ground and depth ramp as the page."""
    rows = ['<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" shape-rendering="crispEdges">',
            f'  <rect width="16" height="16" fill="{NETHER["ground"]}"/>']
    for (x, y, width, height), depth in zip(
        ((6, 2, 4, 2), (5, 5, 6, 2), (3, 8, 10, 2), (2, 11, 12, 3)), (0, 3, 5, 8)
    ):
        rows.append(f'  <rect x="{x}" y="{y}" width="{width}" height="{height}" fill="{RAMPS["nether"][depth]}"/>')
    return "\n".join(rows + ["</svg>", ""])


def main() -> int:
    if "--css" in sys.argv:
        stale = []
        for path, made in ((TOKENS, tokens()), (TOKENS.with_name("icon.svg"), icon())):
            if path.exists() and path.read_text(encoding="utf-8") == made:
                continue
            if "--check" in sys.argv:
                stale.append(path.name)
            else:
                path.write_text(made, encoding="utf-8")
                print(f"design: wrote site/{path.name}")
        if stale:
            print("design: stale " + ", ".join(stale) + "; run scripts/task design", file=sys.stderr)
        return int(bool(stale))

    for label, table in (("the nether", NETHER), ("the surface", LIT)):
        print(f"\n{label}")
        ground = table["ground"]
        for key, value in table.items():
            if key in ("sunk", "ground", "raised"):
                print(f"  {key:8} {value}         ground")
                continue
            ratio = contrast(value, ground)
            mark = "text" if ratio >= 4.5 else ("edge" if ratio >= 3 else "FAILS")
            print(f"  {key:8} {value}  {ratio:5.2f}  {mark}")
        print("  strata")
        for n, colour in enumerate(RAMPS["nether" if "nether" in label else "lit"]):
            ratio = contrast(colour, ground)
            mark = "text" if ratio >= 4.5 else ("edge" if ratio >= 3 else "FAILS")
            print(f"    d{n}     {colour}  {ratio:5.2f}  {mark}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
