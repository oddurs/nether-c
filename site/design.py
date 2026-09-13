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

Three scales and nothing else.

**Colour** is Oklch: a lightness, a chroma and an angle. Two colours with the
same lightness look equally bright, which is the whole reason a ramp built this
way looks designed and one picked out of a box does not. The scheme is one
axis -- the ground is indigo, the complement of indigo is amber, and the nine
strata sweep between them at constant lightness, so depth reads as *hue* and
never as "harder to see". Nothing is at full chroma. Full chroma is a 1980s
text mode and this is not one.

**Space** is the cell. Every measure on the page is a whole number of them and
nothing lands on a half.

**Type** is the cell, doubled and trebled. There are four sizes, because a face
drawn on an eight-pixel grid has exactly four at which a pixel of it is a whole
number of the screen's.

The scheme is one axis. The ground is indigo; the complement of indigo is
amber; and the nine strata sweep from the amber end to the violet end at a
constant lightness, so depth reads as *hue* and never as "harder to see".
Nothing is at full chroma. Full chroma is a 1980s text mode, and this is not.
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

TOKENS = Path(__file__).resolve().parent / "tokens.css"

#: The hue the whole scheme is built on, in degrees. Indigo.
GROUND_HUE = 272.0

#: Its complement, which is where the lamp is.
LAMP_HUE = (GROUND_HUE + 180.0) % 360.0


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


def strata(lightness: float = 0.80, chroma: float = 0.115) -> list[str]:
    """The nine. One lightness, one chroma, and the hue does the talking.

    From the lamp's hue to a little past the ground's, the long way round, so
    that a stratum is told apart by *which* colour it is and never by how hard
    it is to see. §1.1 makes the strata a total order; a ramp that dimmed as it
    went would make the deep ones a total order and a legibility problem.
    """
    first, last = LAMP_HUE, GROUND_HUE + 30.0
    return [
        srgb(lightness, chroma, first + (last - first) * n / 8) for n in range(9)
    ]


#: Everything the stylesheet names, and what each one is for.
NETHER = {
    # the dark, three deep
    "sunk": srgb(0.13, 0.030, GROUND_HUE),
    "ground": srgb(0.17, 0.035, GROUND_HUE),
    "raised": srgb(0.23, 0.040, GROUND_HUE),
    # what is written on it
    "ink": srgb(0.945, 0.012, GROUND_HUE),
    "dim": srgb(0.730, 0.028, GROUND_HUE),
    "dimmer": srgb(0.520, 0.032, GROUND_HUE),
    # the lamp, and the one thing opposite it
    "accent": srgb(0.840, 0.120, LAMP_HUE),
    "alt": srgb(0.780, 0.105, GROUND_HUE + 20),
    # the three that mean something
    "warn": srgb(0.740, 0.135, 25.0),
    "good": srgb(0.820, 0.105, 150.0),
    "code": srgb(0.850, 0.075, 175.0),
    "quote": srgb(0.760, 0.090, GROUND_HUE - 40),
}

#: The surface. The same hues with the sun on them: the lightnesses invert
#: about the middle and the chroma comes down, because a colour that reads as
#: quiet on a dark ground shouts on a light one.
LIT = {
    "sunk": srgb(0.985, 0.008, GROUND_HUE),
    "ground": srgb(0.965, 0.012, GROUND_HUE),
    "raised": srgb(0.915, 0.020, GROUND_HUE),
    "ink": srgb(0.220, 0.035, GROUND_HUE),
    "dim": srgb(0.430, 0.045, GROUND_HUE),
    "dimmer": srgb(0.620, 0.045, GROUND_HUE),
    "accent": srgb(0.480, 0.110, LAMP_HUE - 10),
    "alt": srgb(0.420, 0.150, GROUND_HUE + 20),
    "warn": srgb(0.470, 0.170, 25.0),
    "good": srgb(0.460, 0.105, 150.0),
    "code": srgb(0.440, 0.085, 175.0),
    "quote": srgb(0.450, 0.120, GROUND_HUE - 40),
}


#: A ramp for each ground. One is bright and one is not, because the job is
#: the same on both: nine hues at a single lightness, all legible, none of them
#: louder than its neighbour.
RAMPS = {"nether": strata(), "lit": strata(0.470, 0.130)}

#: Space, in cells. The face advances six of its eight pixels, and at a body
#: size of sixteen that is twelve -- so a cell is 12px and every margin,
#: padding and rule on the site is a count of them.
CELL = 12
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
        " * cell doubled and trebled. Every value here is derived; the argument",
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
    lines += ["}", "", "/* The lamp, lit: the same hues with the sun on them. */",
              ':root[data-lamp="lit"] {']
    for key, value in LIT.items():
        lines.append(f"  --{key}: {value};")
    for n, colour in enumerate(RAMPS["lit"]):
        lines.append(f"  --d{n}: {colour};")
    lines += ["}", ""]
    return "\n".join(lines)


def main() -> int:
    if "--css" in sys.argv:
        out = TOKENS.read_text(encoding="utf-8") if TOKENS.exists() else None
        made = tokens()
        if out == made:
            print(f"design: site/{TOKENS.name} is up to date")
            return 0
        if "--check" in sys.argv:
            print(f"design: site/{TOKENS.name} is stale", file=sys.stderr)
            print("        run scripts/task design and commit the result", file=sys.stderr)
            return 1
        TOKENS.write_text(made, encoding="utf-8")
        print(f"design: wrote site/{TOKENS.name}  {len(made)} bytes")
        return 0

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
