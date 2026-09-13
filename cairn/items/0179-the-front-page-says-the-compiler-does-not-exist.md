---
id: 179
title: The front page says the compiler does not exist
type: bug
status: buried
milestone: necropolis
assignee: Oddur Sigurdsson
created: 2026-09-13
updated: 2026-09-13
priority: p0
effort: m
area: site
stratum: '0'
proof: The front page's account of the project is generated from the board, and the build fails when they disagree
---

```
$ nether
nether — the Nether C rites
  bury    exhume    lamp    cairn    strata    graft
```

All six are built. The World is buried. The front page says:

> Under excavation. Nothing is implemented; the specification comes first.

> The compiler does not exist. There is one binary, and it refuses correctly.

Both were true when somebody typed them and neither is true now, which is what
happens to a sentence about a moving thing. The banner is a GIF, so it did not
even change colour when it stopped being true.

## What to do instead

Not correct the sentences — **generate them**. A page that states the state of
the project is a trace of the board, and this project's whole argument is that
a trace does not drift from the thing it describes.

`site/bake` already substitutes `{{TOC}}`. It can read `cairn/items/*.md` the
same way it reads `spec/*.md` and emit two blocks in the rites' own idiom: the
descent so far, and what the world still owes. Then `scripts/task site:check`
fails the day the board and the page disagree, which is the only way a claim
about the present stays true.

## Acceptance criteria

- [x] Nothing on the front page states the project's state in prose
- [x] The descent and the open holes are generated from `cairn/items/`
- [x] `site:check` fails when the board moves and the page has not been rebaked
- [x] The stale GIF is gone, or says something that cannot go stale

## 2026-09-13

Generated from cairn/items/ by site/bake, the same way the spec pages are generated from spec/. site:check now fails when the board moves and the page has not been rebaked, which is the only way a claim about the present stays true.

## 2026-09-13

Its first run found the board stale too: The Rites was still open with every rites item buried. Closed.

## 2026-09-13

The first version dressed the blocks as $ nether strata nether-c, which is not a command. The transcript harness did not catch it because it reads site/src/index.html and the blocks are generated into the output -- so the check that exists to stop invented transcripts could not see this one. They are captioned as what they are instead.

## 2026-09-13

Plain numerals, not bury's circled ones. At six by eight the ring eats the digit and only the first reads; the font's coverage check refused the rest, correctly.

## 2026-09-13

Closing an item changes the page, so the last step of finishing site work is to rebake. The gate fired on this very commit, which is the right behaviour and worth knowing about.
