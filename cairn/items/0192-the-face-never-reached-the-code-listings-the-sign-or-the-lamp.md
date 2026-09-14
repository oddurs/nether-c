---
id: 192
title: The face never reached the code listings, the sign, or the lamp
type: bug
status: buried
milestone: necropolis
created: 2026-09-13
updated: 2026-09-13
priority: p0
effort: s
area: site
stratum: '0'
proof: Every element on a built page with a user-agent font-family opts back in
---

```css
body { font-family: "Nether 8", ui-monospace, monospace; }
code { font-family: inherit; }
```

That is the whole of it. `font-family` inherits, but only where nothing else
sets it -- and a user agent sets it on `pre`, `code`, `button` and a handful of
others before the page is read. `code` opted back in. `pre` never did.

There are nine `<pre>` on the front page. Every code listing, every transcript,
and `pre.sign` -- the wordmark at the top of every page -- have been set in
whatever monospace the reader happened to have. On a site made of listings that
is most of what there is to look at.

## Why nine pull requests missed it

The wordmark is block art built out of `U+2588`, and a full block renders as a
full block in any monospace font. So the sign looked *correct* while being set
in something else entirely, and it was offered twice in this repository as
evidence that the face had loaded.

A specimen drawn from the same bitmaps proves nothing either, and neither does
a font file that validates: all three checks looked at the font and none of
them looked at whether the page was using it.

## Acceptance criteria

- [x] Every element with a user-agent `font-family` is handed the site's
- [x] Something fails the build when a new one appears and is not

## 2026-09-13

Found by the owner saying the font was not showing, three times, after being told twice it was a cache or a file:// problem. It was neither. The font was correct, the server was correct, and the stylesheet never reached the elements that make up the page.

## 2026-09-13

The guard reads the built HTML rather than a list of tags somebody maintains, so an element that appears in the markup for the first time is caught the day it appears.
