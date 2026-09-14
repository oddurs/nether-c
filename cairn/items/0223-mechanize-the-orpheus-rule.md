---
id: 223
title: Mechanize the Orpheus rule
type: chore
status: unmarked
milestone: assay
depends_on:
- 222
created: 2026-09-13
updated: 2026-09-13
priority: p0
effort: l
area: proof/
stratum: '0'
proof: 'look''s premise is proved to be exactly what soundness requires: no weaker, and no stronger than necessary'
---

1.6's rule is one premise: `d <= delta`. 90.2 records two rejected forms and
argues this one is right.

Proving it means showing both halves — that it admits everything sound, so the
language is not needlessly restrictive, and that it admits nothing unsound.
The second is the one people worry about; the first is the one that is usually
wrong.
