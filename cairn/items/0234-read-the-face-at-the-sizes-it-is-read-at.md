---
id: 234
title: Read the face at the sizes it is read at
type: chore
status: unmarked
milestone: face
depends_on:
- 231
created: 2026-09-14
updated: 2026-09-15
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

- [ ] Each used size has a labelled screenshot with browser, viewport and device scale. Font loading, clipped counters and column alignment are checked; no claim that CSS integer pixels guarantee every zoom rasterization.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
