---
id: 191
title: The site claims to be made by hand
type: bug
status: buried
milestone: necropolis
created: 2026-09-13
updated: 2026-09-13
priority: p1
effort: s
area: site
stratum: '0'
proof: No file in the tree claims the work was done by hand
---

A badge in the footer of every page said **MADE BY HAND**. The colophon said
"everything in this repository was written by hand". Thirteen places in all
said some version of it.

It is not a claim this project can stand behind, and this is the wrong project
to be loose about that. `spec/90-rationale.md` exists because a design document
that lists only advantages is marketing.

## The distinction

*No libraries* is a claim about what the code is made of. It is true, it is
short enough to print, and anybody can check it against `Cargo.toml` and the
import lines.

*By hand* is a claim about the manner of making. It is doing rhetorical work
the other claim does better, and it is not checkable by anyone.

So the manner claims go and the substance claims get louder. The badge reads
`NO LIBRARIES / FROM SCRATCH`. The colophon says what is in the tree and adds
one line saying, plainly, that it is a claim about the code and deliberately
not about how it came to be written.

Nothing here attributes the work to anything; `CLAUDE.md` settled that and it
is not what this is about. Overclaiming and misattributing are different
faults, and only one of them was happening.

## Acceptance criteria

- [x] Nothing on the site or in the source claims to be made by hand
- [x] The claim that replaces it is checkable

## 2026-09-13

Raised by the owner looking at the footer. Thirteen instances across the badge, the colophon, two page footers, the stylesheet header, both font sources and CLAUDE.md.
