---
id: 184
title: Furniture of our own, and a way to look at the site
type: feature
status: buried
milestone: necropolis
created: 2026-09-13
updated: 2026-09-13
priority: p1
effort: s
area: site
stratum: '0'
proof: The site serves, and the counter counts something true
---

## You cannot look at this site

`@font-face` loads `nether.woff` relative to the stylesheet, and a browser
treats a `file://` page as an opaque origin and refuses a font from one. So the
face this site is set in — the most visible thing about it — is invisible
exactly where somebody is most likely to open it, and falls back to whatever
monospace they happen to have.

`web/necropolis/` is worse: a module cannot be loaded from `file://` at all, so
the page that buries without a server cannot be opened without one either.

Nothing in the repository serves anything. `scripts/task serve` is not §8.0's
`run` and does not amount to it — §8.0 is about executing a Nether C program
and this executes nothing.

## The counter was a lie

```python
def counter(text: str):
    """A hit counter counts visitors. This one counts nothing; it is a name."""
```

It displayed `Cairn 8F3A1C0E`, which is not a cairn of anything. In a project
whose thesis is that a name is derived from content, a fabricated name in the
footer of every page is the same defect as a fabricated transcript, and the
docstring knew.

A hit counter counted visits. This one counts what is left: the holes the world
still owes, read off the same board the front page reads, counting **down** as
the work is done.

## And one badge was his voice

`80 COLUMNS / AS GOD MEANT` is a quotation, not a statement, and the column is
eighty cells of a face drawn here now.

## Acceptance criteria

- [x] The site can be served, and says how
- [x] Nothing in the furniture states something that was not derived
- [x] No badge speaks in a borrowed voice

## 2026-09-13

Found by the owner opening file:///.../site/index.html and seeing almost no change. The CSS and the page were both current; the font simply never loaded, because a browser will not fetch a font across an opaque origin. Nothing in the repository served anything, which is why nobody had noticed.
