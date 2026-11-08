---
id: 234
uid: 6636d9ca-1ac1-49b7-9569-285545de2a34
title: Read the face at the sizes it is read at
type: chore
status: buried
milestone: face
depends_on:
- 231
created: 2026-09-14
updated: 2026-11-07
priority: p1
effort: m
area: site/nether.css
stratum: '0'
proof: A written decision on which pixel sizes the face is used at, and why, with a screenshot of each
---

The face is drawn on an 8x8 grid and rendered at whatever `rem` works out to.
A bitmap face has opinions about that: at an integer multiple of its grid every
stem is a whole pixel, and between multiples the rasteriser guesses.

Nobody has checked what size the site actually renders it at, or whether that
is one of the good ones. This is a measurement and a paragraph, not a project.

## Delivery plan — 2026-09-15

### Starting point and scope

The stylesheet already specifies an 8/16/24/32px scale and browser checks exist. Audit their actual rendered coverage rather than claiming nobody has chosen sizes.

### Steps

1. Inventory computed font sizes for prose, code, headings, labels and controls, including the responsive rules.
2. Capture representative desktop/phone text at device scale 1 and 2 and at browser zoom, in both palettes; compare regular and bold after 0231.
3. Record the chosen sizes and observed compromises in spec/90-rationale.md; change only demonstrated readability failures.

### Acceptance and evidence

- [x] Each used size has a labelled screenshot with browser, viewport and device scale. Font loading, clipped counters and column alignment are checked; no claim that CSS integer pixels guarantee every zoom rasterization.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

## 2026-11-07

Inventoried computed `fontSize`, `lineHeight`, `fontWeight` and `letterSpacing` across `/site/`, `/site/spec/00-overview.html`, `/site/specimen.html` and `/web/necropolis/` at `1248px`, `768px`, `390px` and `320px` in headless Chrome (`a858ebc4547dc9416bc1cceb06b841d27c648825`). On `/site/**`, `html` sets `14px` solely as the `rem` spacing base while `body` sets `16px` (`22px` leading via `1.375`), `pre.sign`, `figcaption`, `.plaque > h3`, `footer`, `.badge`, `.counter` and specimen grids/meta use `8px` (`8px` or `11px` leading), `h1`/`h2`/`.covenant strong` use `24px` (`33px` leading), and `.s32` uses `32px`. Two off-grid rules were found and fixed in `site/nether.css`: `.necropolis h1` set `font-size: 48px; line-height: 1.2` (`57.6px` line box at desktop, `38.4px` at `<=760px`) and now uses `font-size: 32px; line-height: 1.25` (`40px`), and eight `letter-spacing` declarations used decimal `em` fractions (`.1em`, `.12em`, `.08em`, `.06em`, shifting characters by `0.64px`–`2.4px`) and now use `.125em` (`1/8 em` = `1/2/3/4px` at `8/16/24/32px`, bringing tracked character pitch from `7/8 em` to `1em`). Added `font-size: inherit` to the user-agent element reset, enforced the eight-pixel `font-size`, `line-height` and `letter-spacing` grid in `tests/css/run` and `tests/necropolis/browser.mjs` (`checkTypeGrid`), verified open counters across all 16 closed-loop glyphs (`08BDOPQRabdegopq`) at `8/16/24/32px` in both weights at `dpr=1` and `dpr=2`, captured labelled Chrome screenshots for each size (`face-{8,16,24,32}px-chrome-1248w-dpr1-dark.png`) and across `1248w`/`390w`, `dpr1`/`dpr2`, `zoom125`/`zoom150`, and `dark`/`lit` palettes via `NETHER_SCREENSHOTS`, and recorded the decision and limits in `spec/90-rationale.md` §90.6 (`The four sizes it is read at, and where the grid gives way`). Limits: `8px` at `dpr=1` has 1px counters in bold `B/8/0/a/e/g/o` (kept to chrome/captions/sign, never prose or code); `glyf` outline rasterisation still applies greyscale coverage antialiasing on scanlines adjacent to crossbars; non-integer zoom (`1.25x`/`1.5x`) only lands on whole physical pixels where `size * zoom` is a multiple of 8 (`32px` at `1.25x`; `16px` and `32px` at `1.5x`).
