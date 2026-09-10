---
id: 92
title: Accessibility and no-JavaScript pass
type: chore
status: buried
milestone: lamp
created: 2026-09-10
updated: 2026-09-10
priority: p2
effort: s
area: site/
proof: Keyboard-only navigation works throughout; every table reads as a table; the site is complete with JS off
---

Focus states, heading order, table semantics for the strata and inversion
tables rather than divs pretending, reduced-motion honoured for the cursor,
and contrast checked on both palettes.

The complemented VGA palette was not designed for contrast and some pairs will
fail. Where they do, the pair changes, not the standard.

## 2026-09-10

Done, and the palette needed real changes. Eight pairs failed WCAG AA across the two grounds: badge text, table headers, listing captions, plaque titles, inline code on white, the blockquote rule on white, structural borders on both, and h4 on white. Every one was fixed by changing the pair, never the standard. Notable outcomes: inline code is LIME on black and PURPLE on white because GREEN on white is 3.11:1; h4 became the colour with no opposite, because ROT clears on black and BROWN clears on white while LILAC and its complement do not; table row hover is now a full inversion at 21:1, which is period-appropriate anyway; --dimmer ended up unused and was deleted. tests/contrast/run holds all 22 pairs plus the 16 palette swatches and runs in scripts/task test. Added a skip link and made <main> focusable. The site has exactly two script blocks, both for the lamp, and is complete without them.
