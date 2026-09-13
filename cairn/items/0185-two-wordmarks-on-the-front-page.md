---
id: 185
title: Two wordmarks on the front page
type: bug
status: buried
milestone: necropolis
created: 2026-09-13
updated: 2026-09-13
priority: p1
effort: s
area: site
stratum: '0'
proof: The front page has one wordmark and it is centred
---

`site/template.html` puts an ASCII block-letter sign at the top of every page.
0182 added the cairn to the front page without noticing, so the front page
carried the sign twice: block letters, then a GIF of the same two words.

The one that stays is the one set in the face this project drew, which is also
the one that proves the face loaded. `sign.gif` and its generator go, because
nothing else pointed at them.

It was also left-aligned in a centred column, because a `pre` is a block and is
as wide as the column whatever is inside it.

## Acceptance criteria

- [x] One wordmark per page
- [x] It is centred
- [x] Nothing orphaned is left behind
